#![forbid(unsafe_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

use sbom_release_diff::input::{DEFAULT_MAX_INPUT_BYTES, HARD_MAX_INPUT_BYTES};
use sbom_release_diff::policy::FailOn;
use sbom_release_diff::render::{Format, render};
use sbom_release_diff::{build_report, load, sanitize};

/// Exit code when every requested policy passed.
const EXIT_OK: u8 = 0;
/// Exit code when a --fail-on policy was violated. The report is still printed.
const EXIT_POLICY: u8 = 1;
/// Exit code for usage errors, unreadable, invalid or unsupported input,
/// and output errors.
const EXIT_ERROR: u8 = 2;

/// Compare two CycloneDX JSON SBOMs and report component changes.
///
/// Reports added, removed, version-changed and declared-license-changed
/// components, plus matches that are ambiguous. Works offline: nothing is
/// fetched and no vulnerability data is queried.
#[derive(Debug, Parser)]
#[command(name = "sbom-release-diff", version, about, long_about = None)]
#[command(
    after_help = "Exit codes: 0 = no policy violated, 1 = a --fail-on policy was violated, \
2 = usage, input or output error.\n\
Supported CycloneDX specVersion: 1.4, 1.5, 1.6, 1.7 (JSON only)."
)]
struct Cli {
    /// CycloneDX JSON SBOM of the earlier release.
    old: PathBuf,

    /// CycloneDX JSON SBOM of the later release.
    new: PathBuf,

    /// Output format.
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Exit with code 1 when any of these occur. Comma-separated or repeated.
    #[arg(long, value_enum, value_delimiter = ',', value_name = "POLICY")]
    fail_on: Vec<FailOn>,

    /// Reject an input file larger than this many bytes (at most 536870912).
    #[arg(long, value_name = "BYTES", default_value_t = DEFAULT_MAX_INPUT_BYTES, value_parser = parse_limit)]
    max_input_bytes: u64,
}

fn parse_limit(value: &str) -> Result<u64, String> {
    let limit: u64 = value
        .parse()
        .map_err(|_| "must be a whole number of bytes".to_owned())?;
    if limit == 0 || limit > HARD_MAX_INPUT_BYTES {
        return Err(format!("must be between 1 and {HARD_MAX_INPUT_BYTES}"));
    }
    Ok(limit)
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let rendered = sanitize::message(&err.render().to_string());
            if err.use_stderr() {
                eprintln!("{rendered}");
                return ExitCode::from(EXIT_ERROR);
            }
            println!("{rendered}");
            return ExitCode::from(EXIT_OK);
        }
    };

    let old = match load(&cli.old, cli.max_input_bytes) {
        Ok(doc) => doc,
        Err(err) => return input_error("old", &cli.old, &err),
    };
    let new = match load(&cli.new, cli.max_input_bytes) {
        Ok(doc) => doc,
        Err(err) => return input_error("new", &cli.new, &err),
    };

    let report = build_report(&old, &new, &cli.fail_on);
    let output = render(&report, cli.format);

    let mut stdout = std::io::stdout().lock();
    if let Err(err) = stdout
        .write_all(output.as_bytes())
        .and_then(|()| stdout.flush())
    {
        eprintln!(
            "error: cannot write report: {}",
            sanitize::message(&err.to_string())
        );
        return ExitCode::from(EXIT_ERROR);
    }

    if report.policy.passed {
        ExitCode::from(EXIT_OK)
    } else {
        ExitCode::from(EXIT_POLICY)
    }
}

fn input_error(label: &str, path: &Path, err: &sbom_release_diff::error::Error) -> ExitCode {
    eprintln!(
        "error: {label} SBOM '{}': {}",
        sanitize::display(&path.display().to_string()),
        sanitize::message(&err.to_string())
    );
    ExitCode::from(EXIT_ERROR)
}
