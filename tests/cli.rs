//! End-to-end tests that run the built binary against fixture SBOMs.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_sbom-release-diff");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> PathBuf {
    root().join("tests/fixtures").join(name)
}

fn example(name: &str) -> PathBuf {
    root().join("examples").join(name)
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&std::ffi::OsStr]) -> Run {
    let output = Command::new(BIN).args(args).output().expect("binary runs");
    Run {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

fn diff(old: &Path, new: &Path, extra: &[&str]) -> Run {
    let mut args: Vec<&std::ffi::OsStr> = vec![old.as_os_str(), new.as_os_str()];
    args.extend(extra.iter().map(std::ffi::OsStr::new));
    run(&args)
}

fn diff_json(old: &Path, new: &Path) -> Value {
    let r = diff(old, new, &["--format", "json"]);
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    serde_json::from_str(&r.stdout).expect("valid JSON")
}

fn pair(name: &str) -> (PathBuf, PathBuf) {
    (
        fixture(&format!("{name}-old.json")),
        fixture(&format!("{name}-new.json")),
    )
}

fn identities(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|c| c["identity"].as_str().unwrap().to_owned())
        .collect()
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

// ---------------------------------------------------------------- golden

fn release_pair() -> (PathBuf, PathBuf) {
    (
        example("release-1.0.0.cdx.json"),
        example("release-1.1.0.cdx.json"),
    )
}

fn assert_golden(format: &str, expected_file: &str) {
    let (old, new) = release_pair();
    let r = diff(&old, &new, &["--format", format]);
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    let expected_path = root().join("tests/expected").join(expected_file);
    let expected = std::fs::read_to_string(&expected_path).expect("golden file exists");
    let actual = r
        .stdout
        .replace(env!("CARGO_PKG_VERSION"), "{{TOOL_VERSION}}");
    assert_eq!(
        normalize_newlines(&actual),
        normalize_newlines(&expected),
        "output differs from {}",
        expected_path.display()
    );
}

#[test]
fn golden_text() {
    assert_golden("text", "release-1.0.0-to-1.1.0.txt");
}

#[test]
fn golden_markdown() {
    assert_golden("markdown", "release-1.0.0-to-1.1.0.md");
}

#[test]
fn golden_json() {
    assert_golden("json", "release-1.0.0-to-1.1.0.json");
}

#[test]
fn documented_example_matches_real_output() {
    let doc = normalize_newlines(
        &std::fs::read_to_string(root().join("docs/example-release-comparison.md")).unwrap(),
    );
    let (old, new) = release_pair();
    for format in ["text", "markdown"] {
        let out = normalize_newlines(&diff(&old, &new, &["-f", format]).stdout);
        assert!(
            doc.contains(&out),
            "docs/example-release-comparison.md is out of date for {format} output"
        );
    }
}

#[test]
fn output_is_deterministic() {
    let (old, new) = release_pair();
    for format in ["text", "markdown", "json"] {
        let a = diff(&old, &new, &["-f", format]).stdout;
        let b = diff(&old, &new, &["-f", format]).stdout;
        assert_eq!(a, b, "{format} output changed between runs");
    }
}

// ---------------------------------------------------------------- reordered data

#[test]
fn reordered_document_has_no_differences() {
    let report = diff_json(
        &example("release-1.0.0.cdx.json"),
        &fixture("reordered-release-1.0.0.cdx.json"),
    );
    let s = &report["summary"];
    for key in [
        "added",
        "removed",
        "version_changed",
        "license_changed",
        "ambiguous",
        "warnings",
    ] {
        assert_eq!(s[key], 0, "{key}");
    }
    assert_eq!(s["unchanged"], 9);
}

#[test]
fn reordering_the_old_document_does_not_change_the_report() {
    let (old, new) = release_pair();
    let mut original = diff_json(&old, &new);
    let mut reordered = diff_json(&fixture("reordered-release-1.0.0.cdx.json"), &new);
    original["old"]["file"] = Value::Null;
    reordered["old"]["file"] = Value::Null;
    assert_eq!(original, reordered);
}

#[test]
fn swapping_inputs_swaps_added_and_removed() {
    let (old, new) = release_pair();
    let forward = diff_json(&old, &new);
    let backward = diff_json(&new, &old);
    assert_eq!(
        identities(&forward["added"]),
        identities(&backward["removed"])
    );
    assert_eq!(
        identities(&forward["removed"]),
        identities(&backward["added"])
    );
    assert_eq!(
        forward["summary"]["version_changed"],
        backward["summary"]["version_changed"]
    );
}

// ---------------------------------------------------------------- duplicates

#[test]
fn duplicate_components() {
    let (old, new) = pair("duplicates");
    let report = diff_json(&old, &new);

    // Identical entries are merged and reported as warnings.
    let warnings = report["warnings"].as_array().unwrap();
    let dup: Vec<(&str, &str, u64)> = warnings
        .iter()
        .filter(|w| w["kind"] == "duplicate-component")
        .map(|w| {
            (
                w["side"].as_str().unwrap(),
                w["identity"].as_str().unwrap(),
                w["count"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        dup,
        vec![("old", "pkg:npm/debug", 2), ("new", "pkg:npm/inherits", 2)]
    );

    // Several versions of one package: the one that is gone is removed.
    assert_eq!(identities(&report["removed"]), vec!["pkg:npm/lodash"]);
    assert_eq!(report["removed"][0]["version"], "4.17.20");
    assert_eq!(identities(&report["added"]), vec!["pkg:npm/glob"]);
    assert_eq!(report["summary"]["version_changed"], 0);

    // Leftover versions on both sides, or one version listed twice with
    // different licenses, are reported as ambiguous instead of guessed.
    let ambiguous: Vec<(&str, &str)> = report["ambiguous"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["identity"].as_str().unwrap(),
                a["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        ambiguous,
        vec![
            ("pkg:npm/ms", "multiple-candidates"),
            ("pkg:npm/semver", "duplicate-version-differs"),
        ]
    );
    assert_eq!(report["ambiguous"][0]["old"][0]["version"], "2.0.0");
    assert_eq!(report["ambiguous"][0]["new"][0]["version"], "2.1.3");
    assert_eq!(report["summary"]["unchanged"], 4);
}

// ---------------------------------------------------------------- missing identifiers

#[test]
fn missing_identifiers_use_fallback_identity() {
    let (old, new) = pair("missing-identifiers");
    let report = diff_json(&old, &new);

    let changes: Vec<(&str, Value, Value)> = report["version_changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["name"].as_str().unwrap(),
                c["old_version"].clone(),
                c["new_version"].clone(),
            )
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            ("vendored-parser", "1.0".into(), "1.1".into()),
            ("build-helper", Value::Null, "2.0".into()),
            ("tzdata", "2024a".into(), "2024b".into()),
        ]
    );
    assert_eq!(
        report["version_changed"][1]["identity"],
        "component:library/org.example/build-helper"
    );
    assert_eq!(report["version_changed"][1]["identity_basis"], "fallback");

    assert_eq!(report["removed"][0]["name"], "legacy-tool");
    assert_eq!(report["summary"]["added"], 0);

    // A purl present on one side only, or a changed component type without
    // a purl, is not silently paired.
    let ambiguous = report["ambiguous"].as_array().unwrap();
    assert_eq!(ambiguous.len(), 2);
    assert!(
        ambiguous
            .iter()
            .all(|a| a["reason"] == "identity-basis-differs")
    );
    assert_eq!(ambiguous[0]["identity"], "name:widget");
    assert_eq!(ambiguous[1]["identity"], "name:zlib");
    assert_eq!(ambiguous[1]["old"][0]["identity_basis"], "purl");
    assert_eq!(ambiguous[1]["new"][0]["identity_basis"], "fallback");

    // app-config (no purl, no version) and blank-version ("" == absent).
    assert_eq!(report["summary"]["unchanged"], 2);
}

// ---------------------------------------------------------------- license expressions

#[test]
fn license_expressions() {
    let (old, new) = pair("licenses");
    let report = diff_json(&old, &new);
    let changed: Vec<(&str, Value, Value)> = report["license_changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["name"].as_str().unwrap(),
                c["old_licenses"].clone(),
                c["new_licenses"].clone(),
            )
        })
        .collect();
    assert_eq!(
        changed,
        vec![
            (
                "operator",
                serde_json::json!(["MIT AND Apache-2.0"]),
                serde_json::json!(["MIT OR Apache-2.0"])
            ),
            (
                "exception",
                serde_json::json!(["GPL-2.0-only"]),
                serde_json::json!(["GPL-2.0-only WITH Classpath-exception-2.0"])
            ),
            ("dropped", serde_json::json!(["MIT"]), serde_json::json!([])),
        ]
    );
    // Operand order, identifier case, id-vs-expression, entry order,
    // concluded-only entries, whitespace and nested parentheses do not count.
    assert_eq!(report["summary"]["unchanged"], 8);

    let unparsed: Vec<&str> = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["kind"] == "unparsed-license-expression")
        .map(|w| w["side"].as_str().unwrap())
        .collect();
    assert_eq!(unparsed, vec!["old", "new"]);
}

// ---------------------------------------------------------------- version changes

#[test]
fn version_changes() {
    let (old, new) = pair("versions");
    let report = diff_json(&old, &new);
    let changes: Vec<(&str, &str, &str)> = report["version_changed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["identity"].as_str().unwrap(),
                c["old_version"].as_str().unwrap(),
                c["new_version"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            ("pkg:npm/downgrade", "3.2.0", "3.1.9"),
            ("pkg:npm/mismatch", "1.0.0", "1.0.1"),
            ("pkg:npm/prerelease", "2.0.0-rc.1", "2.0.0"),
            // Versions are compared as exact strings.
            ("pkg:npm/semver-style", "1.0", "1.0.0"),
            // pypi names are normalized as the purl spec requires.
            ("pkg:pypi/django-rest", "3.14", "3.15"),
        ]
    );
    // A location qualifier does not change identity...
    assert_eq!(report["summary"]["unchanged"], 1);
    // ...but a platform qualifier does.
    assert_eq!(
        identities(&report["removed"]),
        vec!["pkg:deb/debian/curl?arch=amd64&distro=debian-12"]
    );
    assert_eq!(
        identities(&report["added"]),
        vec!["pkg:deb/debian/curl?arch=arm64&distro=debian-12"]
    );
    assert_eq!(report["warnings"][0]["kind"], "purl-version-mismatch");
}

// ---------------------------------------------------------------- hostile content

#[test]
fn hostile_content_is_neutralized() {
    let (old, new) = pair("hostile");
    for format in ["text", "markdown", "json"] {
        let r = diff(&old, &new, &["--format", format]);
        assert_eq!(r.code, 0, "{format}: {}", r.stderr);
        let out = &r.stdout;
        assert!(
            !out.contains("SECRET_TOKEN"),
            "{format} output leaks a credential"
        );
        for c in out.chars() {
            let bad = (c.is_control() && c != '\n')
                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
            assert!(!bad, "{format} output contains raw U+{:04X}", c as u32);
        }
        // An injected newline must not create a fake report line.
        assert!(
            !out.lines().any(|l| l.starts_with("Added (99)")),
            "{format}: injected line"
        );
    }

    let md = diff(&old, &new, &["--format", "markdown"]).stdout;
    // Markup only ever appears inside code spans, and pipes are escaped.
    assert!(md.contains("`` \\| [click](https://evil.example.test) @octocat"));
    assert!(!md.contains(" | [click]"), "unescaped pipe");
    assert!(md.contains("` <script>alert(1)</script> \\| x `"));

    // JSON keeps the data intact, with unsafe characters escaped.
    let json: Value = serde_json::from_str(&diff(&old, &new, &["-f", "json"]).stdout).unwrap();
    let names: Vec<&str> = json["added"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"\u{202e}evil-bidi\u{2066}x"));
    assert!(names.iter().any(|n| n.starts_with("\u{1b}[2J")));
    // Long values are truncated in text output only.
    let long = format!("long-{}", "a".repeat(600));
    assert!(names.contains(&long.as_str()));
    assert!(!diff(&old, &new, &[]).stdout.contains(&long));
}

// ---------------------------------------------------------------- invalid input

#[test]
fn invalid_documents_are_rejected() {
    let cases = [
        ("not-json.json", "invalid JSON"),
        ("xml-document.json", "invalid JSON"),
        ("truncated.json", "invalid JSON"),
        ("trailing-data.json", "trailing characters"),
        ("duplicate-keys.json", "repeats the same key"),
        ("empty.json", "the file is empty"),
        ("invalid-utf8.json", "invalid JSON"),
        ("root-array.json", "root must be a JSON object"),
        ("spdx.json", "bomFormat is missing"),
        ("wrong-bom-format.json", "bomFormat must be \"CycloneDX\""),
        ("missing-spec.json", "specVersion is missing"),
        ("spec-not-string.json", "specVersion must be a string"),
        (
            "unsupported-spec-1.3.json",
            "unsupported CycloneDX specVersion",
        ),
        (
            "unsupported-spec-2.0.json",
            "unsupported CycloneDX specVersion",
        ),
        ("components-not-array.json", "components must be an array"),
        (
            "component-not-object.json",
            "components[0] must be an object",
        ),
        ("missing-name.json", "components[0].name is missing"),
        ("missing-type.json", "components[0].type is missing"),
        (
            "version-not-string.json",
            "components[0].version must be a string",
        ),
        ("null-purl.json", "components[0].purl must be a string"),
        (
            "malformed-purl.json",
            "components[0].purl: purl has no scheme",
        ),
        ("license-id-and-name.json", "either id or name, not both"),
        (
            "license-and-expression.json",
            "exactly one of license or expression",
        ),
        ("bad-acknowledgement.json", "acknowledgement must be"),
        (
            "nested-components-not-array.json",
            "components[0].components[0].components must be an array",
        ),
        (
            "bom-version-zero.json",
            "version must be an integer of 1 or more",
        ),
    ];
    let valid = example("release-1.0.0.cdx.json");
    let dir = root().join("tests/fixtures/invalid");
    let mut seen = 0;
    for (file, expected) in cases {
        let path = dir.join(file);
        for (old, new, label) in [(&path, &valid, "old"), (&valid, &path, "new")] {
            let r = diff(old, new, &[]);
            assert_eq!(r.code, 2, "{file} as {label}: {}", r.stderr);
            assert!(r.stdout.is_empty(), "{file}: no report on error");
            assert!(
                r.stderr.contains(&format!("error: {label} SBOM")) && r.stderr.contains(expected),
                "{file} as {label}: expected {expected:?}, got {:?}",
                r.stderr
            );
        }
        seen += 1;
    }
    let on_disk = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(seen, on_disk, "every invalid fixture has a test case");
}

#[test]
fn errors_do_not_echo_document_content() {
    let dir = std::env::temp_dir().join(format!("sbom-release-diff-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("leak.json");
    std::fs::write(
        &path,
        r#"{"bomFormat": "CycloneDX", "specVersion": "SECRET_TOKEN_1", "components": []}"#,
    )
    .unwrap();
    let r = diff(&path, &example("release-1.0.0.cdx.json"), &[]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(r.code, 2);
    assert!(!r.stderr.contains("SECRET_TOKEN"), "{}", r.stderr);
}

#[test]
fn oversized_input_is_rejected() {
    let (old, new) = release_pair();
    let r = diff(&old, &new, &["--max-input-bytes", "512"]);
    assert_eq!(r.code, 2);
    assert!(
        r.stderr
            .contains("larger than the input limit of 512 bytes")
    );
}

#[test]
fn input_limit_is_bounded() {
    let (old, new) = release_pair();
    for bad in ["0", "536870913", "-1", "ten"] {
        let r = diff(&old, &new, &["--max-input-bytes", bad]);
        assert_eq!(r.code, 2, "{bad}");
    }
    let r = diff(&old, &new, &["--max-input-bytes", "536870912"]);
    assert_eq!(r.code, 0);
}

#[test]
fn too_many_components_are_rejected() {
    let limit = sbom_release_diff::cyclonedx::MAX_COMPONENTS;
    let mut doc = String::from(r#"{"bomFormat":"CycloneDX","specVersion":"1.6","components":["#);
    for i in 0..=limit {
        if i > 0 {
            doc.push(',');
        }
        doc.push_str(r#"{"type":"library","name":"c"}"#);
    }
    doc.push_str("]}");
    let err = sbom_release_diff::parse_document(doc.as_bytes()).unwrap_err();
    assert!(err.to_string().contains("more than"), "{err}");
}

#[test]
fn non_files_and_missing_files_are_rejected() {
    let valid = example("release-1.0.0.cdx.json");
    let r = diff(&root().join("examples"), &valid, &[]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("not a regular file"));

    let r = diff(&root().join("does-not-exist.json"), &valid, &[]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("cannot read"));
}

#[test]
fn usage_errors_exit_2() {
    assert_eq!(run(&[]).code, 2);
    let (old, new) = release_pair();
    assert_eq!(diff(&old, &new, &["--fail-on", "bogus"]).code, 2);
    assert_eq!(diff(&old, &new, &["--format", "xml"]).code, 2);
    assert_eq!(run(&["--help".as_ref()]).code, 0);
    assert_eq!(run(&["--version".as_ref()]).code, 0);
}

// ---------------------------------------------------------------- CI policies

#[test]
fn fail_on_policies() {
    let (old, new) = release_pair();
    // added 1, removed 1, version 6, license 1, ambiguous 1, warnings 0
    let cases: &[(&[&str], i32)] = &[
        (&[], 0),
        (&["--fail-on", "added"], 1),
        (&["--fail-on", "removed"], 1),
        (&["--fail-on", "version-changed"], 1),
        (&["--fail-on", "license-changed"], 1),
        (&["--fail-on", "ambiguous"], 1),
        (&["--fail-on", "warning"], 0),
        (&["--fail-on", "any-change"], 1),
        (&["--fail-on", "warning,added"], 1),
        (&["--fail-on", "warning", "--fail-on", "removed"], 1),
    ];
    for (args, expected) in cases {
        let r = diff(&old, &new, args);
        assert_eq!(r.code, *expected, "{args:?}");
        assert!(!r.stdout.is_empty(), "report printed for {args:?}");
    }

    // No changes means any-change passes.
    let same = diff(&old, &old, &["--fail-on", "any-change"]);
    assert_eq!(same.code, 0);
    assert!(same.stdout.contains("No component differences."));

    let r = diff(
        &old,
        &new,
        &["-f", "json", "--fail-on", "license-changed,warning"],
    );
    let json: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(
        json["policy"],
        serde_json::json!({
            "fail_on": ["license-changed", "warning"],
            "violated": ["license-changed"],
            "passed": false
        })
    );
}

// ---------------------------------------------------------------- JSON schema

/// Checks `value` against the subset of JSON Schema used by
/// schema/report-v1.schema.json, and also fails on undocumented fields.
fn check_schema(schema: &Value, defs: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let name = reference.trim_start_matches("#/$defs/");
        return check_schema(&defs[name], defs, value, path);
    }
    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        let matches = options
            .iter()
            .filter(|o| check_schema(o, defs, value, path).is_ok())
            .count();
        return if matches == 1 {
            Ok(())
        } else {
            Err(format!("{path}: matches {matches} oneOf options"))
        };
    }
    if let Some(expected) = schema.get("const") {
        if value != expected {
            return Err(format!("{path}: expected {expected}"));
        }
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        if !options.contains(value) {
            return Err(format!("{path}: {value} not in enum"));
        }
    }
    if let Some(types) = schema.get("type") {
        let types: Vec<&str> = match types {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        let actual = match value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(n) if n.is_u64() || n.is_i64() => "integer",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        if !types.contains(&actual) {
            return Err(format!("{path}: {actual} is not one of {types:?}"));
        }
    }
    if let (Some(items), Some(array)) = (schema.get("items"), value.as_array()) {
        for (i, item) in array.iter().enumerate() {
            check_schema(items, defs, item, &format!("{path}[{i}]"))?;
        }
    }
    if let (Some(properties), Some(object)) = (schema.get("properties"), value.as_object()) {
        let required: Vec<&str> = schema["required"]
            .as_array()
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for key in &required {
            if !object.contains_key(*key) {
                return Err(format!("{path}: missing required {key}"));
            }
        }
        for (key, item) in object {
            let Some(property) = properties.get(key) else {
                return Err(format!("{path}: field {key} is not in the schema"));
            };
            check_schema(property, defs, item, &format!("{path}.{key}"))?;
        }
    }
    Ok(())
}

#[test]
fn json_output_matches_schema() {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("schema/report-v1.schema.json")).unwrap(),
    )
    .unwrap();
    let defs = &schema["$defs"];
    let mut checked = 0;
    for (old, new) in [
        release_pair(),
        pair("duplicates"),
        pair("missing-identifiers"),
        pair("licenses"),
        pair("versions"),
        pair("hostile"),
    ] {
        let report = diff_json(&old, &new);
        if let Err(e) = check_schema(&schema, defs, &report, "$") {
            panic!("{} vs {}: {e}", old.display(), new.display());
        }
        checked += 1;
    }
    assert_eq!(checked, 6);
}
