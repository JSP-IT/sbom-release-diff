//! Text, Markdown and JSON rendering. Every SBOM-derived value passes
//! through [`crate::sanitize`] on the way out.

use std::fmt::Write as _;

use crate::cyclonedx::IdentityBasis;
use crate::report::{ComponentRef, DocumentInfo, Report, Side};
use crate::sanitize::{display, json_output, markdown_code};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Plain text for terminals and CI logs.
    Text,
    /// GitHub-flavored Markdown, for pull request comments and job summaries.
    Markdown,
    /// Versioned JSON for tooling (see schema/report-v1.schema.json).
    Json,
}

pub fn render(report: &Report, format: Format) -> String {
    match format {
        Format::Text => text(report),
        Format::Markdown => markdown(report),
        Format::Json => json(report),
    }
}

pub fn json(report: &Report) -> String {
    let serialized = serde_json::to_string_pretty(report).expect("report serializes");
    let mut out = json_output(&serialized);
    out.push('\n');
    out
}

const NONE: &str = "(none)";

/// Human-readable name: the purl identity, or group/name for components
/// identified without a purl.
fn label(
    identity: &str,
    basis: IdentityBasis,
    component_type: &str,
    name: &str,
    group: Option<&str>,
) -> String {
    match basis {
        IdentityBasis::Purl => identity.to_owned(),
        IdentityBasis::Fallback => match group {
            Some(group) => format!("{group}/{name} ({component_type}, no purl)"),
            None => format!("{name} ({component_type}, no purl)"),
        },
    }
}

fn component_label(c: &ComponentRef) -> String {
    label(
        &c.identity,
        c.identity_basis,
        &c.component_type,
        &c.name,
        c.group.as_deref(),
    )
}

fn version(v: Option<&str>) -> &str {
    v.unwrap_or(NONE)
}

fn licenses_text(licenses: &[String]) -> String {
    if licenses.is_empty() {
        NONE.to_owned()
    } else {
        licenses
            .iter()
            .map(|l| display(l))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn licenses_md(licenses: &[String]) -> String {
    if licenses.is_empty() {
        NONE.to_owned()
    } else {
        licenses
            .iter()
            .map(|l| markdown_code(l))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn subject_text(doc: &DocumentInfo) -> String {
    match &doc.subject {
        None => NONE.to_owned(),
        Some(s) => {
            let name = match &s.group {
                Some(g) => format!("{g}/{}", s.name),
                None => s.name.clone(),
            };
            match &s.version {
                Some(v) => format!("{name} {v}"),
                None => name,
            }
        }
    }
}

fn summary_line(report: &Report) -> String {
    let s = &report.summary;
    let warnings = if s.warnings == 1 {
        "warning"
    } else {
        "warnings"
    };
    format!(
        "{} added, {} removed, {} version changed, {} license changed, {} ambiguous, {} unchanged, {} {warnings}",
        s.added,
        s.removed,
        s.version_changed,
        s.license_changed,
        s.ambiguous,
        s.unchanged,
        s.warnings
    )
}

fn policy_text(report: &Report) -> String {
    let p = &report.policy;
    let list = |items: &[crate::policy::FailOn]| {
        items
            .iter()
            .map(|i| i.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if p.fail_on.is_empty() {
        "passed (no --fail-on policy set)".to_owned()
    } else if p.passed {
        format!("passed (fail on: {})", list(&p.fail_on))
    } else {
        format!(
            "FAILED: {} (fail on: {})",
            list(&p.violated),
            list(&p.fail_on)
        )
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Old => "old",
        Side::New => "new",
    }
}

fn has_differences(report: &Report) -> bool {
    !(report.added.is_empty()
        && report.removed.is_empty()
        && report.version_changed.is_empty()
        && report.license_changed.is_empty()
        && report.ambiguous.is_empty())
}

pub fn text(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} {} - report v{}",
        report.tool.name, report.tool.version, report.report_version
    );
    for (title, doc) in [("Old", &report.old), ("New", &report.new)] {
        let _ = writeln!(
            out,
            "{title}: {} (CycloneDX {}, {} components, subject: {})",
            display(&doc.file),
            display(&doc.spec_version),
            doc.component_count,
            display(&subject_text(doc))
        );
    }
    let _ = writeln!(out, "\nSummary: {}", summary_line(report));

    if !report.added.is_empty() {
        let _ = writeln!(out, "\nAdded ({}):", report.added.len());
        for c in &report.added {
            let _ = writeln!(
                out,
                "  + {}  {}  [{}]",
                display(&component_label(c)),
                display(version(c.version.as_deref())),
                licenses_text(&c.licenses)
            );
        }
    }
    if !report.removed.is_empty() {
        let _ = writeln!(out, "\nRemoved ({}):", report.removed.len());
        for c in &report.removed {
            let _ = writeln!(
                out,
                "  - {}  {}  [{}]",
                display(&component_label(c)),
                display(version(c.version.as_deref())),
                licenses_text(&c.licenses)
            );
        }
    }
    if !report.version_changed.is_empty() {
        let _ = writeln!(out, "\nVersion changed ({}):", report.version_changed.len());
        for c in &report.version_changed {
            let name = label(
                &c.identity,
                c.identity_basis,
                &c.component_type,
                &c.name,
                c.group.as_deref(),
            );
            let _ = writeln!(
                out,
                "  ~ {}  {} -> {}",
                display(&name),
                display(version(c.old_version.as_deref())),
                display(version(c.new_version.as_deref()))
            );
        }
    }
    if !report.license_changed.is_empty() {
        let _ = writeln!(out, "\nLicense changed ({}):", report.license_changed.len());
        for c in &report.license_changed {
            let name = label(
                &c.identity,
                c.identity_basis,
                &c.component_type,
                &c.name,
                c.group.as_deref(),
            );
            let _ = writeln!(
                out,
                "  ! {}  {}: [{}] -> [{}]",
                display(&name),
                display(version(c.new_version.as_deref())),
                licenses_text(&c.old_licenses),
                licenses_text(&c.new_licenses)
            );
        }
    }
    if !report.ambiguous.is_empty() {
        let _ = writeln!(out, "\nAmbiguous ({}):", report.ambiguous.len());
        for a in &report.ambiguous {
            let _ = writeln!(out, "  ? {}  {}", display(&a.identity), a.reason.describe());
            for (side, entries) in [("old", &a.old), ("new", &a.new)] {
                let listed = entries
                    .iter()
                    .map(|c| {
                        format!(
                            "{} {} [{}]",
                            display(&component_label(c)),
                            display(version(c.version.as_deref())),
                            licenses_text(&c.licenses)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                let _ = writeln!(out, "      {side}: {listed}");
            }
        }
    }
    if !report.warnings.is_empty() {
        let _ = writeln!(out, "\nWarnings ({}):", report.warnings.len());
        for w in &report.warnings {
            let count = w.count.map(|n| format!(" (x{n})")).unwrap_or_default();
            let _ = writeln!(
                out,
                "  * {} {} {}: {}{count}",
                side_name(w.side),
                display(&w.identity),
                display(version(w.version.as_deref())),
                w.kind.describe()
            );
        }
    }
    if !has_differences(report) {
        let _ = writeln!(out, "\nNo component differences.");
    }
    let _ = writeln!(out, "\nPolicy: {}", policy_text(report));
    out
}

pub fn markdown(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "## SBOM release diff\n");
    let _ = writeln!(out, "| | Old | New |");
    let _ = writeln!(out, "|---|---|---|");
    let (o, n) = (&report.old, &report.new);
    let _ = writeln!(
        out,
        "| File | {} | {} |",
        markdown_code(&o.file),
        markdown_code(&n.file)
    );
    let _ = writeln!(
        out,
        "| CycloneDX | {} | {} |",
        markdown_code(&o.spec_version),
        markdown_code(&n.spec_version)
    );
    let _ = writeln!(
        out,
        "| Subject | {} | {} |",
        markdown_code(&subject_text(o)),
        markdown_code(&subject_text(n))
    );
    let _ = writeln!(
        out,
        "| Components | {} | {} |",
        o.component_count, n.component_count
    );
    let _ = writeln!(out, "\n**Summary:** {}", summary_line(report));

    if !report.added.is_empty() || !report.removed.is_empty() {
        for (title, list) in [("Added", &report.added), ("Removed", &report.removed)] {
            if list.is_empty() {
                continue;
            }
            let _ = writeln!(out, "\n### {title} ({})\n", list.len());
            let _ = writeln!(out, "| Component | Version | Declared licenses |");
            let _ = writeln!(out, "|---|---|---|");
            for c in list {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    markdown_code(&component_label(c)),
                    markdown_code(version(c.version.as_deref())),
                    licenses_md(&c.licenses)
                );
            }
        }
    }
    if !report.version_changed.is_empty() {
        let _ = writeln!(
            out,
            "\n### Version changed ({})\n",
            report.version_changed.len()
        );
        let _ = writeln!(out, "| Component | Old version | New version |");
        let _ = writeln!(out, "|---|---|---|");
        for c in &report.version_changed {
            let name = label(
                &c.identity,
                c.identity_basis,
                &c.component_type,
                &c.name,
                c.group.as_deref(),
            );
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                markdown_code(&name),
                markdown_code(version(c.old_version.as_deref())),
                markdown_code(version(c.new_version.as_deref()))
            );
        }
    }
    if !report.license_changed.is_empty() {
        let _ = writeln!(
            out,
            "\n### Declared license changed ({})\n",
            report.license_changed.len()
        );
        let _ = writeln!(out, "| Component | Version | Old licenses | New licenses |");
        let _ = writeln!(out, "|---|---|---|---|");
        for c in &report.license_changed {
            let name = label(
                &c.identity,
                c.identity_basis,
                &c.component_type,
                &c.name,
                c.group.as_deref(),
            );
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                markdown_code(&name),
                markdown_code(version(c.new_version.as_deref())),
                licenses_md(&c.old_licenses),
                licenses_md(&c.new_licenses)
            );
        }
    }
    if !report.ambiguous.is_empty() {
        let _ = writeln!(out, "\n### Ambiguous ({})\n", report.ambiguous.len());
        let _ = writeln!(out, "| Identity | Reason | Old | New |");
        let _ = writeln!(out, "|---|---|---|---|");
        for a in &report.ambiguous {
            let entries = |list: &[ComponentRef]| {
                list.iter()
                    .map(|c| {
                        format!(
                            "{} {}",
                            markdown_code(&component_label(c)),
                            markdown_code(version(c.version.as_deref()))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                markdown_code(&a.identity),
                a.reason.describe(),
                entries(&a.old),
                entries(&a.new)
            );
        }
    }
    if !report.warnings.is_empty() {
        let _ = writeln!(out, "\n### Warnings ({})\n", report.warnings.len());
        let _ = writeln!(out, "| Side | Component | Version | Warning |");
        let _ = writeln!(out, "|---|---|---|---|");
        for w in &report.warnings {
            let count = w.count.map(|n| format!(" (x{n})")).unwrap_or_default();
            let _ = writeln!(
                out,
                "| {} | {} | {} | {}{count} |",
                side_name(w.side),
                markdown_code(&w.identity),
                markdown_code(version(w.version.as_deref())),
                w.kind.describe()
            );
        }
    }
    if !has_differences(report) {
        let _ = writeln!(out, "\nNo component differences.");
    }
    let _ = writeln!(out, "\n**Policy:** {}", policy_text(report));
    let _ = writeln!(
        out,
        "\n_{} {}, report v{}. Declared licenses are compared as written; this is not a license compatibility assessment._",
        report.tool.name, report.tool.version, report.report_version
    );
    out
}
