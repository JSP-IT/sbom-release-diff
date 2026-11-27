//! Deterministic, offline comparison of two CycloneDX JSON SBOMs.
//!
//! The library reads two documents, validates the fields it compares, pairs
//! components by package URL (or a conservative fallback identity) and
//! reports added, removed, version-changed, declared-license-changed and
//! ambiguous components. It performs no network access.

#![forbid(unsafe_code)]

pub mod cyclonedx;
pub mod diff;
pub mod error;
pub mod input;
pub mod license;
pub mod policy;
pub mod purl;
pub mod render;
pub mod report;
pub mod sanitize;

use std::path::Path;

use cyclonedx::Bom;
use error::Error;
use policy::FailOn;
use report::{DocumentInfo, Report, SubjectInfo, Summary, ToolInfo};

pub const TOOL_NAME: &str = env!("CARGO_PKG_NAME");
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A validated document plus the file name it was read from.
#[derive(Debug, Clone)]
pub struct LoadedBom {
    pub file: String,
    pub bom: Bom,
}

/// Parses and validates a CycloneDX JSON document held in memory.
pub fn parse_document(bytes: &[u8]) -> Result<Bom, Error> {
    let value = input::parse_strict_json(bytes)?;
    cyclonedx::parse_bom(&value)
}

/// Reads, size-checks, parses and validates one SBOM file.
pub fn load(path: &Path, max_input_bytes: u64) -> Result<LoadedBom, Error> {
    let bytes = input::read_bounded(path, max_input_bytes)?;
    let bom = parse_document(&bytes)?;
    let file = path.file_name().map_or_else(
        || "(unnamed)".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    Ok(LoadedBom { file, bom })
}

/// Compares two documents and evaluates the failure policies.
pub fn build_report(old: &LoadedBom, new: &LoadedBom, fail_on: &[FailOn]) -> Report {
    let changes = diff::compare(&old.bom, &new.bom);
    let summary = Summary {
        added: changes.added.len(),
        removed: changes.removed.len(),
        version_changed: changes.version_changed.len(),
        license_changed: changes.license_changed.len(),
        ambiguous: changes.ambiguous.len(),
        unchanged: changes.unchanged,
        warnings: changes.warnings.len(),
    };
    let policy = policy::evaluate(fail_on, &summary);
    Report {
        report_format: report::REPORT_FORMAT,
        report_version: report::REPORT_VERSION,
        tool: ToolInfo {
            name: TOOL_NAME,
            version: TOOL_VERSION,
        },
        old: document_info(old),
        new: document_info(new),
        summary,
        added: changes.added,
        removed: changes.removed,
        version_changed: changes.version_changed,
        license_changed: changes.license_changed,
        ambiguous: changes.ambiguous,
        warnings: changes.warnings,
        policy,
    }
}

fn document_info(loaded: &LoadedBom) -> DocumentInfo {
    let bom = &loaded.bom;
    DocumentInfo {
        file: loaded.file.clone(),
        spec_version: bom.spec_version.clone(),
        serial_number: bom.serial_number.clone(),
        bom_version: bom.bom_version,
        subject: bom.subject.as_ref().map(|s| SubjectInfo {
            name: s.name.clone(),
            group: s.group.clone(),
            version: s.version.clone(),
        }),
        component_count: bom.components.len(),
    }
}
