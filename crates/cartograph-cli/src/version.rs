//! `cartograph version`, and the release identity the rest of the CLI reports.
//!
//! # Why this is its own module
//!
//! Until M09 the milestone was a literal `"M01"` twenty lines into `main.rs`.
//! It stayed there through M02–M08 and told every user of the binary something
//! false. The lesson is not "remember to update the constant": it is that a
//! fact nothing checks will drift. So this module keeps the one value that
//! genuinely cannot be derived — the milestone — in a single named place, and
//! `tests::the_reported_milestone_matches_the_project_ledger` fails the build
//! when it disagrees with `agentos/artifacts/project-state.yaml`.
//!
//! Everything else is derived: the version from Cargo, the commit and target
//! triple from `build.rs`.

use serde::Serialize;

/// The milestone whose scope this binary implements.
///
/// Checked against the project ledger by a unit test rather than trusted.
pub const MILESTONE: &str = "M14";

/// Release identity, in the shape `version --json` promises.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct VersionInfo {
    /// The released version of the binary, from Cargo.
    pub version: &'static str,
    /// The frozen specification version the domain model implements.
    pub spec_version: &'static str,
    /// The milestone whose scope this binary implements.
    pub milestone: &'static str,
    /// Short commit the binary was built from, or `unknown` outside a checkout.
    pub commit: &'static str,
    /// The target triple this binary was compiled for.
    pub target: &'static str,
}

impl VersionInfo {
    /// The identity of the running binary.
    #[must_use]
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            spec_version: cartograph_core::SPEC_VERSION,
            milestone: MILESTONE,
            commit: env!("CARTOGRAPH_COMMIT"),
            target: env!("CARTOGRAPH_TARGET"),
        }
    }
}

/// Prints the release identity.
///
/// The text form is one `key value` pair per line so `cartograph version |
/// grep` and `cut` both work without a parser; the JSON form is for everything
/// else. Both are deterministic for a given binary.
pub fn run(json: bool) -> Result<String, crate::error::CliError> {
    use std::fmt::Write as _;

    let info = VersionInfo::current();
    let mut out = String::new();

    if json {
        let text = serde_json::to_string_pretty(&info).map_err(|error| {
            crate::error::CliError::new(
                crate::error::ErrorCode::AnalysisFailed,
                "the version could not be serialised as JSON",
            )
            .with_hint(format!("underlying cause: {error}"))
        })?;
        out.push_str(&text);
        out.push('\n');
    } else {
        let _ = writeln!(out, "cartograph {}", info.version);
        let _ = writeln!(out, "specification {}", info.spec_version);
        let _ = writeln!(out, "milestone {}", info.milestone);
        let _ = writeln!(out, "commit {}", info.commit);
        let _ = writeln!(out, "target {}", info.target);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_version_is_the_cargo_version() {
        assert_eq!(VersionInfo::current().version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn the_release_version_is_v010() {
        // M09 ships 0.1.0. Not 1.0: the scope is one supported subset, and
        // saying otherwise in the version string would be the same kind of
        // overclaim the summary wording exists to avoid.
        assert_eq!(VersionInfo::current().version, "0.1.0");
    }

    #[test]
    fn the_specification_version_is_reported() {
        assert_eq!(VersionInfo::current().spec_version, "V3");
    }

    #[test]
    fn the_milestone_is_not_the_stale_m01_value() {
        // The precise regression this module exists to prevent.
        assert_ne!(
            VersionInfo::current().milestone,
            "M01",
            "version reported M01 from M01 through M08; it must never do so again"
        );
    }

    #[test]
    fn build_metadata_is_populated_or_explicitly_unknown() {
        let info = VersionInfo::current();
        // `unknown` is a legitimate value outside a git checkout, but empty is
        // never legitimate: it would render as a blank field.
        assert!(!info.commit.is_empty());
        assert!(!info.target.is_empty());
        assert!(
            info.commit.chars().all(|c| c.is_ascii_alphanumeric()),
            "commit must be sanitised to alphanumerics: {}",
            info.commit
        );
    }

    #[test]
    fn the_reported_milestone_matches_the_project_ledger() {
        // The guard that makes staleness impossible to ship quietly. The
        // ledger is the authority for which milestone is current; if it moves
        // and this constant does not, this test fails.
        //
        // Skipped when the ledger is absent, which is the case for a packaged
        // crate built outside the repository — there the constant is simply
        // what it was when the release was cut.
        let ledger = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .map(|root| root.join("agentos/artifacts/project-state.yaml"));

        let Some(path) = ledger.filter(|p| p.exists()) else {
            return;
        };
        let text = std::fs::read_to_string(&path).expect("ledger is readable");
        let current = text
            .lines()
            .find_map(|line| line.strip_prefix("current_milestone:"))
            .map(str::trim)
            .expect("ledger declares current_milestone");

        assert_eq!(
            MILESTONE, current,
            "version reports {MILESTONE} but the project ledger says {current}"
        );
    }
}
