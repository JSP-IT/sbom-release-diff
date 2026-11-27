//! The report model. Its JSON form is versioned: see `schema/report-v1.schema.json`.

use serde::Serialize;

use crate::cyclonedx::{Component, IdentityBasis};
use crate::policy::FailOn;

/// Value of `report_format` in JSON output.
pub const REPORT_FORMAT: &str = "sbom-release-diff-report";
/// Major version of the JSON report. It changes only when an existing field
/// is removed, renamed or changes meaning; new fields may be added within a
/// version.
pub const REPORT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub report_format: &'static str,
    pub report_version: u32,
    pub tool: ToolInfo,
    pub old: DocumentInfo,
    pub new: DocumentInfo,
    pub summary: Summary,
    pub added: Vec<ComponentRef>,
    pub removed: Vec<ComponentRef>,
    pub version_changed: Vec<VersionChange>,
    pub license_changed: Vec<LicenseChange>,
    pub ambiguous: Vec<Ambiguity>,
    pub warnings: Vec<Warning>,
    pub policy: PolicyResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentInfo {
    /// File name only. The directory part of the path is not reported.
    pub file: String,
    pub spec_version: String,
    pub serial_number: Option<String>,
    pub bom_version: Option<u64>,
    pub subject: Option<SubjectInfo>,
    /// Components read from the document, nested ones included, before
    /// duplicates are merged.
    pub component_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubjectInfo {
    pub name: String,
    pub group: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Summary {
    pub added: usize,
    pub removed: usize,
    pub version_changed: usize,
    pub license_changed: usize,
    pub ambiguous: usize,
    pub unchanged: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ComponentRef {
    pub identity: String,
    pub identity_basis: IdentityBasis,
    pub component_type: String,
    pub name: String,
    pub group: Option<String>,
    pub version: Option<String>,
    pub licenses: Vec<String>,
}

impl From<&Component> for ComponentRef {
    fn from(c: &Component) -> Self {
        Self {
            identity: c.identity.key.clone(),
            identity_basis: c.identity.basis,
            component_type: c.component_type.clone(),
            name: c.name.clone(),
            group: c.group.clone(),
            version: c.version.clone(),
            licenses: c.licenses.iter().map(|l| l.display.clone()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct VersionChange {
    pub identity: String,
    pub identity_basis: IdentityBasis,
    pub component_type: String,
    pub name: String,
    pub group: Option<String>,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct LicenseChange {
    pub identity: String,
    pub identity_basis: IdentityBasis,
    pub component_type: String,
    pub name: String,
    pub group: Option<String>,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub old_licenses: Vec<String>,
    pub new_licenses: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AmbiguityReason {
    /// One side lists the same identity and version more than once with
    /// different details, so it is unclear which entry to compare.
    DuplicateVersionDiffers,
    /// An identity has several entries and the remaining old and new
    /// versions cannot be paired one-to-one.
    MultipleCandidates,
    /// Removed and added components share a name, but at least one of them
    /// has no purl, so they may or may not be the same component.
    IdentityBasisDiffers,
}

impl AmbiguityReason {
    pub fn describe(self) -> &'static str {
        match self {
            Self::DuplicateVersionDiffers => {
                "same version listed more than once with different details"
            }
            Self::MultipleCandidates => "several versions on both sides; no unique pairing",
            Self::IdentityBasisDiffers => "same name, but a purl is missing on at least one side",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Ambiguity {
    pub identity: String,
    pub reason: AmbiguityReason,
    pub old: Vec<ComponentRef>,
    pub new: Vec<ComponentRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Side {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WarningKind {
    /// Identical entries were merged into one.
    DuplicateComponent,
    /// The purl version and the `version` field differ; the purl was used.
    PurlVersionMismatch,
    /// A license expression did not parse and was compared as text.
    UnparsedLicenseExpression,
}

impl WarningKind {
    pub fn describe(self) -> &'static str {
        match self {
            Self::DuplicateComponent => "identical entries merged",
            Self::PurlVersionMismatch => "purl version differs from version field; purl used",
            Self::UnparsedLicenseExpression => "license expression not parsed; compared as text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Warning {
    pub kind: WarningKind,
    pub side: Side,
    pub identity: String,
    pub version: Option<String>,
    /// Number of identical entries, for `duplicate-component`.
    pub count: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyResult {
    pub fail_on: Vec<FailOn>,
    pub violated: Vec<FailOn>,
    pub passed: bool,
}
