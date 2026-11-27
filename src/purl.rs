//! Package URL (purl) parsing and version-independent identity keys.
//!
//! Only the parts of the purl specification needed to build a stable
//! identity are implemented. Nothing is resolved or fetched.

use std::collections::BTreeMap;

/// Longest purl accepted, in bytes.
pub const MAX_PURL_LEN: usize = 4096;

/// Qualifiers that describe where an artifact came from or how to verify
/// it, rather than which package it is. They are left out of the identity
/// key (and so out of reports), because they often change between releases
/// and can carry credentials, for example in `repository_url`.
const NON_IDENTITY_QUALIFIERS: &[&str] = &[
    "checksum",
    "checksums",
    "download_url",
    "file_name",
    "repository_url",
    "vcs_url",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purl {
    pub package_type: String,
    pub namespace: Vec<String>,
    pub name: String,
    pub version: Option<String>,
    pub qualifiers: BTreeMap<String, String>,
    pub subpath: Vec<String>,
}

impl Purl {
    /// Parses a purl string. Error messages never include the input.
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        if input.len() > MAX_PURL_LEN {
            return Err("purl is longer than 4096 bytes");
        }
        let (scheme, rest) = input.split_once(':').ok_or("purl has no scheme")?;
        if !scheme.eq_ignore_ascii_case("pkg") {
            return Err("purl scheme must be 'pkg'");
        }
        let rest = rest.trim_start_matches('/');

        let (rest, subpath) = match rest.rsplit_once('#') {
            Some((r, s)) => (r, parse_subpath(s)?),
            None => (rest, Vec::new()),
        };
        let (rest, qualifiers) = match rest.rsplit_once('?') {
            Some((r, q)) => (r, parse_qualifiers(q)?),
            None => (rest, BTreeMap::new()),
        };

        let (package_type, rest) = rest.split_once('/').ok_or("purl has no package name")?;
        let package_type = package_type.to_ascii_lowercase();
        validate_type(&package_type)?;

        // A version separator only counts after the last '/', so an
        // unencoded npm scope such as `@angular/core` is not read as one.
        let last_slash = rest.rfind('/');
        let (path, version) = match rest.rfind('@') {
            Some(at) if last_slash.is_none_or(|slash| at > slash) => {
                let version = percent_decode(&rest[at + 1..])?;
                (&rest[..at], (!version.is_empty()).then_some(version))
            }
            _ => (rest, None),
        };

        let mut segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(percent_decode)
            .collect::<Result<_, _>>()?;
        let name = segments.pop().ok_or("purl has no package name")?;
        if name.is_empty() {
            return Err("purl has no package name");
        }
        let mut purl = Self {
            package_type,
            namespace: segments,
            name,
            version,
            qualifiers,
            subpath,
        };
        purl.normalize_for_type();
        Ok(purl)
    }

    /// Applies the case and character rules the purl type definitions
    /// require. Types not listed here are compared exactly as decoded.
    fn normalize_for_type(&mut self) {
        match self.package_type.as_str() {
            "bitbucket" | "github" => {
                for segment in &mut self.namespace {
                    *segment = segment.to_lowercase();
                }
                self.name = self.name.to_lowercase();
            }
            "pypi" => {
                self.name = self.name.to_lowercase().replace('_', "-");
            }
            _ => {}
        }
    }

    /// A version-independent key: type, namespace, name, identity
    /// qualifiers and subpath, in canonical ASCII form.
    pub fn identity_key(&self) -> String {
        let mut key = format!("pkg:{}/", self.package_type);
        for segment in &self.namespace {
            key.push_str(&percent_encode(segment));
            key.push('/');
        }
        key.push_str(&percent_encode(&self.name));
        let identity_qualifiers: Vec<String> = self
            .qualifiers
            .iter()
            .filter(|(k, _)| !NON_IDENTITY_QUALIFIERS.contains(&k.as_str()))
            .map(|(k, v)| format!("{k}={}", percent_encode(&redact_userinfo(v))))
            .collect();
        if !identity_qualifiers.is_empty() {
            key.push('?');
            key.push_str(&identity_qualifiers.join("&"));
        }
        if !self.subpath.is_empty() {
            key.push('#');
            let encoded: Vec<String> = self.subpath.iter().map(|s| percent_encode(s)).collect();
            key.push_str(&encoded.join("/"));
        }
        key
    }
}

fn validate_type(package_type: &str) -> Result<(), &'static str> {
    let mut chars = package_type.chars();
    match chars.next() {
        None => Err("purl has no package type"),
        Some(c) if !c.is_ascii_alphabetic() => Err("purl type must start with a letter"),
        Some(_) if chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-')) => {
            Ok(())
        }
        Some(_) => Err("purl type contains an invalid character"),
    }
}

fn parse_qualifiers(input: &str) -> Result<BTreeMap<String, String>, &'static str> {
    let mut qualifiers = BTreeMap::new();
    for pair in input.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').ok_or("purl qualifier has no '='")?;
        let key = key.to_ascii_lowercase();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            return Err("purl qualifier key is invalid");
        }
        let value = percent_decode(value)?;
        if value.is_empty() {
            continue;
        }
        if qualifiers.insert(key, value).is_some() {
            return Err("purl repeats a qualifier key");
        }
    }
    Ok(qualifiers)
}

fn parse_subpath(input: &str) -> Result<Vec<String>, &'static str> {
    input
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .map(percent_decode)
        .collect()
}

fn percent_decode(input: &str) -> Result<String, &'static str> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes
                .get(i + 1..i + 3)
                .ok_or("purl has an incomplete percent escape")?;
            let hex = std::str::from_utf8(hex).map_err(|_| "purl has an invalid percent escape")?;
            let byte =
                u8::from_str_radix(hex, 16).map_err(|_| "purl has an invalid percent escape")?;
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "purl percent escapes do not form valid UTF-8")
}

/// Percent-encodes everything except unreserved ASCII, giving an ASCII-only
/// key with no separators or control characters inside a segment.
pub fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'~' | b':') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Replaces `user:password@` in a URL-like value with `REDACTED@`.
pub fn redact_userinfo(value: &str) -> String {
    let Some(scheme_end) = value.find("://") else {
        return value.to_owned();
    };
    let authority_start = scheme_end + 3;
    let authority_end = value[authority_start..]
        .find(['/', '?', '#'])
        .map_or(value.len(), |i| authority_start + i);
    match value[authority_start..authority_end].rfind('@') {
        Some(at) => format!(
            "{}REDACTED{}",
            &value[..authority_start],
            &value[authority_start + at..]
        ),
        None => value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> String {
        Purl::parse(s).unwrap().identity_key()
    }

    #[test]
    fn parses_basic_purl() {
        let p = Purl::parse("pkg:maven/org.apache.commons/commons-lang3@3.14.0").unwrap();
        assert_eq!(p.package_type, "maven");
        assert_eq!(p.namespace, vec!["org.apache.commons"]);
        assert_eq!(p.name, "commons-lang3");
        assert_eq!(p.version.as_deref(), Some("3.14.0"));
    }

    #[test]
    fn identity_ignores_version_and_location_qualifiers() {
        assert_eq!(
            key("pkg:npm/left-pad@1.3.0?repository_url=https://u:p@example.test/r"),
            key("pkg:npm/left-pad@1.2.0")
        );
    }

    #[test]
    fn identity_keeps_platform_qualifiers() {
        assert_ne!(
            key("pkg:deb/debian/curl@8.0?arch=amd64"),
            key("pkg:deb/debian/curl@8.0?arch=arm64")
        );
    }

    #[test]
    fn encoded_and_unencoded_npm_scope_match() {
        assert_eq!(
            key("pkg:npm/%40angular/core@17.0.0"),
            key("pkg:npm/@angular/core@17.0.0")
        );
        let p = Purl::parse("pkg:npm/@angular/core").unwrap();
        assert_eq!(p.name, "core");
        assert_eq!(p.version, None);
    }

    #[test]
    fn type_specific_normalization() {
        assert_eq!(key("pkg:pypi/Django_Rest@1"), key("pkg:pypi/django-rest@2"));
        assert_eq!(key("pkg:GitHub/Org/Repo@1"), key("pkg:github/org/repo@1"));
        // Other types are case-sensitive.
        assert_ne!(key("pkg:maven/Org/Lib@1"), key("pkg:maven/org/lib@1"));
    }

    #[test]
    fn qualifier_order_does_not_matter() {
        assert_eq!(
            key("pkg:rpm/fedora/curl@8?arch=x86_64&distro=fedora-40"),
            key("pkg:rpm/fedora/curl@8?distro=fedora-40&arch=x86_64")
        );
    }

    #[test]
    fn rejects_malformed_purls() {
        for bad in [
            "maven/org/lib@1",
            "http://example.test/x",
            "pkg:",
            "pkg:npm",
            "pkg:npm/",
            "pkg:1npm/x",
            "pkg:npm/x?arch",
            "pkg:npm/x?a=1&a=2",
            "pkg:npm/%zz",
            "pkg:npm/%ff",
        ] {
            assert!(Purl::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn identity_key_is_ascii() {
        let k = key("pkg:generic/n\u{e4}me%20x?tag=\u{1b}[31m");
        assert!(k.is_ascii());
        assert!(!k.contains('\u{1b}'));
    }

    #[test]
    fn redacts_userinfo() {
        assert_eq!(
            redact_userinfo("https://user:tok@host.test/path@x"),
            "https://REDACTED@host.test/path@x"
        );
        assert_eq!(
            redact_userinfo("https://host.test/a@b"),
            "https://host.test/a@b"
        );
        assert_eq!(redact_userinfo("plain"), "plain");
    }
}
