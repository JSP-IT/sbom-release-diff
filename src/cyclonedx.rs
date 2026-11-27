//! Validation and extraction of the CycloneDX fields this tool compares.
//!
//! Only the fields needed for the comparison are read. Everything else in
//! the document (services, dependencies, vulnerabilities, properties,
//! hashes, external references and so on) is ignored and never reported.

use serde_json::{Map, Value};

use crate::error::Error;
use crate::license::LicenseEntry;
use crate::purl::{Purl, percent_encode};

/// CycloneDX specification versions accepted in `specVersion`.
pub const SUPPORTED_SPEC_VERSIONS: &[&str] = &["1.4", "1.5", "1.6", "1.7"];

/// Maximum number of components (including nested ones) in one document.
pub const MAX_COMPONENTS: usize = 250_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdentityBasis {
    /// Identified by package URL.
    Purl,
    /// No purl: identified by component type, group and name.
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// Version-independent key used to pair components across documents.
    pub key: String,
    pub basis: IdentityBasis,
    /// Lowercased group and name, used only to detect components whose
    /// identity basis differs between the two documents.
    pub loose: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub component_type: String,
    pub group: Option<String>,
    pub name: String,
    /// Version from the purl if present, otherwise from `version`.
    pub version: Option<String>,
    pub identity: Identity,
    /// Declared licenses, sorted and deduplicated by canonical form.
    pub licenses: Vec<LicenseEntry>,
}

impl Component {
    pub fn license_keys(&self) -> Vec<&str> {
        self.licenses.iter().map(|l| l.canonical.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    pub name: String,
    pub group: Option<String>,
    pub version: Option<String>,
}

/// Something worth telling the user that does not make the input invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNote {
    pub kind: NoteKind,
    pub identity: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NoteKind {
    /// The purl version and the `version` field disagree; the purl wins.
    PurlVersionMismatch,
    /// A license expression did not parse and is compared as text.
    UnparsedLicenseExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bom {
    pub spec_version: String,
    pub serial_number: Option<String>,
    pub bom_version: Option<u64>,
    pub subject: Option<Subject>,
    pub components: Vec<Component>,
    pub notes: Vec<ParseNote>,
}

/// Validates a parsed JSON document as a supported CycloneDX BOM.
pub fn parse_bom(root: &Value) -> Result<Bom, Error> {
    let root = root
        .as_object()
        .ok_or_else(|| invalid("the document root must be a JSON object"))?;

    match root.get("bomFormat") {
        Some(Value::String(s)) if s == "CycloneDX" => {}
        Some(Value::String(_)) => return Err(invalid("bomFormat must be \"CycloneDX\"")),
        Some(_) => return Err(invalid("bomFormat must be a string")),
        None => {
            return Err(invalid(
                "bomFormat is missing (is this a CycloneDX JSON document?)",
            ));
        }
    }

    let spec_version = match root.get("specVersion") {
        Some(Value::String(s)) if SUPPORTED_SPEC_VERSIONS.contains(&s.as_str()) => s.clone(),
        Some(Value::String(_)) => {
            return Err(Error::new(format!(
                "unsupported CycloneDX specVersion; supported versions are {}",
                SUPPORTED_SPEC_VERSIONS.join(", ")
            )));
        }
        Some(_) => return Err(invalid("specVersion must be a string")),
        None => return Err(invalid("specVersion is missing")),
    };

    let serial_number = optional_string(root, "serialNumber", "serialNumber")?;
    let bom_version = match root.get("version") {
        None => None,
        Some(Value::Number(n)) => Some(
            n.as_u64()
                .filter(|v| *v >= 1)
                .ok_or_else(|| invalid("version must be an integer of 1 or more"))?,
        ),
        Some(_) => return Err(invalid("version must be an integer")),
    };

    let mut context = Context {
        components: Vec::new(),
        notes: Vec::new(),
    };

    let subject = match root.get("metadata") {
        None => None,
        Some(Value::Object(metadata)) => match metadata.get("component") {
            None => None,
            Some(value) => {
                let component = context.component(value, "metadata.component", false)?;
                Some(Subject {
                    name: component.name,
                    group: component.group,
                    version: component.version,
                })
            }
        },
        Some(_) => return Err(invalid("metadata must be an object")),
    };
    // The subject is described, not compared, so drop notes about it.
    context.notes.clear();

    match root.get("components") {
        None => {}
        Some(Value::Array(items)) => context.components_array(items, "components")?,
        Some(_) => return Err(invalid("components must be an array")),
    }

    Ok(Bom {
        spec_version,
        serial_number,
        bom_version,
        subject,
        components: context.components,
        notes: context.notes,
    })
}

struct Context {
    components: Vec<Component>,
    notes: Vec<ParseNote>,
}

impl Context {
    fn components_array(&mut self, items: &[Value], location: &str) -> Result<(), Error> {
        for (index, item) in items.iter().enumerate() {
            let location = format!("{location}[{index}]");
            let component = self.component(item, &location, true)?;
            if self.components.len() >= MAX_COMPONENTS {
                return Err(Error::new(format!(
                    "the document has more than {MAX_COMPONENTS} components"
                )));
            }
            self.components.push(component);
        }
        Ok(())
    }

    fn component(
        &mut self,
        value: &Value,
        location: &str,
        include_children: bool,
    ) -> Result<Component, Error> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid(format!("{location} must be an object")))?;

        let component_type = required_string(object, "type", location)?;
        let name = required_string(object, "name", location)?;
        let group = optional_string(object, "group", &format!("{location}.group"))?
            .filter(|g| !g.is_empty());
        let declared_version = optional_string(object, "version", &format!("{location}.version"))?
            .filter(|v| !v.is_empty());

        let purl = optional_string(object, "purl", &format!("{location}.purl"))?
            .filter(|p| !p.is_empty())
            .map(|p| {
                Purl::parse(&p).map_err(|reason| invalid(format!("{location}.purl: {reason}")))
            })
            .transpose()?;

        let loose = match &group {
            Some(group) => format!("{}/{}", group.to_lowercase(), name.to_lowercase()),
            None => name.to_lowercase(),
        };
        let (identity, version) = match &purl {
            Some(purl) => {
                let key = purl.identity_key();
                let version = match (&purl.version, &declared_version) {
                    (Some(p), Some(d)) if p != d => {
                        self.notes.push(ParseNote {
                            kind: NoteKind::PurlVersionMismatch,
                            identity: key.clone(),
                            version: Some(p.clone()),
                        });
                        Some(p.clone())
                    }
                    (Some(p), _) => Some(p.clone()),
                    (None, d) => d.clone(),
                };
                let identity = Identity {
                    key,
                    basis: IdentityBasis::Purl,
                    loose,
                };
                (identity, version)
            }
            None => {
                let identity = Identity {
                    key: fallback_key(&component_type, group.as_deref(), &name),
                    basis: IdentityBasis::Fallback,
                    loose,
                };
                (identity, declared_version)
            }
        };

        let mut licenses = match object.get("licenses") {
            None => Vec::new(),
            Some(Value::Array(items)) => parse_licenses(items, location)?,
            Some(_) => return Err(invalid(format!("{location}.licenses must be an array"))),
        };
        licenses.sort();
        licenses.dedup_by(|a, b| a.canonical == b.canonical);
        if licenses.iter().any(|l| !l.parsed) {
            self.notes.push(ParseNote {
                kind: NoteKind::UnparsedLicenseExpression,
                identity: identity.key.clone(),
                version: version.clone(),
            });
        }

        if include_children {
            match object.get("components") {
                None => {}
                Some(Value::Array(items)) => {
                    self.components_array(items, &format!("{location}.components"))?;
                }
                Some(_) => {
                    return Err(invalid(format!("{location}.components must be an array")));
                }
            }
        }

        Ok(Component {
            component_type,
            group,
            name,
            version,
            identity,
            licenses,
        })
    }
}

/// Identity for components without a purl. `bom-ref` is not used: it is
/// only unique inside one document and is often a random UUID. Each part is
/// percent-encoded, so a '/' inside a group or name cannot collide with the
/// separator.
fn fallback_key(component_type: &str, group: Option<&str>, name: &str) -> String {
    format!(
        "component:{}/{}/{}",
        percent_encode(component_type),
        percent_encode(group.unwrap_or("")),
        percent_encode(name)
    )
}

fn parse_licenses(items: &[Value], location: &str) -> Result<Vec<LicenseEntry>, Error> {
    let mut entries = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let location = format!("{location}.licenses[{index}]");
        let object = item
            .as_object()
            .ok_or_else(|| invalid(format!("{location} must be an object")))?;
        match (object.get("license"), object.get("expression")) {
            (Some(license), None) => {
                let license = license
                    .as_object()
                    .ok_or_else(|| invalid(format!("{location}.license must be an object")))?;
                if !is_declared(license, &format!("{location}.license"))? {
                    continue;
                }
                let id = optional_string(license, "id", &format!("{location}.license.id"))?;
                let name = optional_string(license, "name", &format!("{location}.license.name"))?;
                let entry = match (id, name) {
                    (Some(id), None) if !id.trim().is_empty() => LicenseEntry::from_id(&id),
                    (None, Some(name)) if !name.trim().is_empty() => LicenseEntry::from_name(&name),
                    (Some(_), Some(_)) => {
                        return Err(invalid(format!(
                            "{location}.license must have either id or name, not both"
                        )));
                    }
                    _ => {
                        return Err(invalid(format!(
                            "{location}.license needs a non-empty id or name"
                        )));
                    }
                };
                entries.push(entry);
            }
            (None, Some(expression)) => {
                if !is_declared(object, &location)? {
                    continue;
                }
                let expression = expression
                    .as_str()
                    .filter(|e| !e.trim().is_empty())
                    .ok_or_else(|| {
                        invalid(format!("{location}.expression must be a non-empty string"))
                    })?;
                entries.push(LicenseEntry::from_expression(expression));
            }
            _ => {
                return Err(invalid(format!(
                    "{location} must contain exactly one of license or expression"
                )));
            }
        }
    }
    Ok(entries)
}

/// CycloneDX 1.6 added `acknowledgement`. Only declared licenses (or
/// entries without an acknowledgement) are compared; concluded ones are
/// skipped.
fn is_declared(object: &Map<String, Value>, location: &str) -> Result<bool, Error> {
    match object.get("acknowledgement") {
        None => Ok(true),
        Some(Value::String(s)) if s == "declared" => Ok(true),
        Some(Value::String(s)) if s == "concluded" => Ok(false),
        Some(_) => Err(invalid(format!(
            "{location}.acknowledgement must be \"declared\" or \"concluded\""
        ))),
    }
}

fn required_string(
    object: &Map<String, Value>,
    key: &str,
    location: &str,
) -> Result<String, Error> {
    match object.get(key) {
        Some(Value::String(s)) if !s.is_empty() => Ok(s.clone()),
        Some(Value::String(_)) => Err(invalid(format!("{location}.{key} must not be empty"))),
        Some(_) => Err(invalid(format!("{location}.{key} must be a string"))),
        None => Err(invalid(format!("{location}.{key} is missing"))),
    }
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    location: &str,
) -> Result<Option<String>, Error> {
    match object.get(key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{location} must be a string"))),
    }
}

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::new(format!("invalid CycloneDX document: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bom(components: Value) -> Value {
        json!({"bomFormat": "CycloneDX", "specVersion": "1.6", "components": components})
    }

    #[test]
    fn nested_components_are_included() {
        let doc = bom(json!([
            {"type": "library", "name": "outer", "components": [
                {"type": "library", "name": "inner"}
            ]}
        ]));
        let parsed = parse_bom(&doc).unwrap();
        assert_eq!(parsed.components.len(), 2);
    }

    #[test]
    fn purl_version_wins_and_is_noted() {
        let doc = bom(json!([
            {"type": "library", "name": "x", "version": "1.0", "purl": "pkg:npm/x@1.1"}
        ]));
        let parsed = parse_bom(&doc).unwrap();
        assert_eq!(parsed.components[0].version.as_deref(), Some("1.1"));
        assert_eq!(parsed.notes[0].kind, NoteKind::PurlVersionMismatch);
    }

    #[test]
    fn concluded_licenses_are_ignored() {
        let doc = bom(json!([
            {"type": "library", "name": "x", "licenses": [
                {"license": {"id": "MIT", "acknowledgement": "declared"}},
                {"license": {"id": "GPL-3.0-only", "acknowledgement": "concluded"}},
                {"expression": "BSD-3-Clause", "acknowledgement": "concluded"}
            ]}
        ]));
        let parsed = parse_bom(&doc).unwrap();
        assert_eq!(parsed.components[0].license_keys(), vec!["expr:mit"]);
    }

    #[test]
    fn fallback_keys_do_not_collide() {
        assert_ne!(
            fallback_key("library", Some("a/b"), "c"),
            fallback_key("library", Some("a"), "b/c")
        );
    }

    #[test]
    fn rejects_unsupported_spec_version() {
        let doc = json!({"bomFormat": "CycloneDX", "specVersion": "1.3"});
        let err = parse_bom(&doc).unwrap_err().to_string();
        assert!(err.contains("unsupported"), "{err}");
    }
}
