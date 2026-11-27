//! CI failure policies.

use serde::Serialize;

use crate::report::{PolicyResult, Summary};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum FailOn {
    /// Any component was added.
    Added,
    /// Any component was removed.
    Removed,
    /// Any paired component changed version.
    VersionChanged,
    /// Any paired component changed its declared licenses.
    LicenseChanged,
    /// Any match was ambiguous.
    Ambiguous,
    /// Any warning was reported.
    Warning,
    /// Any of added, removed, version-changed, license-changed or ambiguous.
    AnyChange,
}

impl FailOn {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::VersionChanged => "version-changed",
            Self::LicenseChanged => "license-changed",
            Self::Ambiguous => "ambiguous",
            Self::Warning => "warning",
            Self::AnyChange => "any-change",
        }
    }

    fn is_violated(self, s: &Summary) -> bool {
        match self {
            Self::Added => s.added > 0,
            Self::Removed => s.removed > 0,
            Self::VersionChanged => s.version_changed > 0,
            Self::LicenseChanged => s.license_changed > 0,
            Self::Ambiguous => s.ambiguous > 0,
            Self::Warning => s.warnings > 0,
            Self::AnyChange => {
                s.added + s.removed + s.version_changed + s.license_changed + s.ambiguous > 0
            }
        }
    }
}

/// Evaluates the requested policies. Order and repeats in `fail_on` do not
/// affect the result.
pub fn evaluate(fail_on: &[FailOn], summary: &Summary) -> PolicyResult {
    let mut fail_on = fail_on.to_vec();
    fail_on.sort();
    fail_on.dedup();
    let violated: Vec<FailOn> = fail_on
        .iter()
        .copied()
        .filter(|p| p.is_violated(summary))
        .collect();
    PolicyResult {
        passed: violated.is_empty(),
        fail_on,
        violated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_policy_always_passes() {
        let summary = Summary {
            added: 3,
            ..Summary::default()
        };
        assert!(evaluate(&[], &summary).passed);
    }

    #[test]
    fn reports_only_violated_policies() {
        let summary = Summary {
            removed: 1,
            ..Summary::default()
        };
        let result = evaluate(&[FailOn::Added, FailOn::Removed, FailOn::Removed], &summary);
        assert!(!result.passed);
        assert_eq!(result.fail_on, vec![FailOn::Added, FailOn::Removed]);
        assert_eq!(result.violated, vec![FailOn::Removed]);
    }

    #[test]
    fn any_change_ignores_warnings() {
        let summary = Summary {
            warnings: 2,
            unchanged: 5,
            ..Summary::default()
        };
        assert!(evaluate(&[FailOn::AnyChange], &summary).passed);
        assert!(!evaluate(&[FailOn::Warning], &summary).passed);
    }
}
