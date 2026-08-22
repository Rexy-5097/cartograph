//! Build-time version metadata.
//!
//! `cartograph version` reported a hardcoded `M01` for eight milestones because
//! the string lived in a source file nobody had reason to revisit. The fix is
//! not a better constant — it is to derive what can be derived, and to make the
//! rest fail a test when it goes stale (see `version::tests`).
//!
//! Everything here degrades to `unknown` rather than failing the build: a
//! release tarball or a `cargo install` from a registry has no `.git`
//! directory, and refusing to build in that case would break the very
//! distribution path M09 exists to establish.

use std::process::Command;

fn main() {
    // The target triple the binary is being built for. Cargo always sets it,
    // so `version` can state the platform rather than let a user guess which
    // binary they downloaded.
    println!(
        "cargo:rustc-env=CARTOGRAPH_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned())
    );

    println!("cargo:rustc-env=CARTOGRAPH_COMMIT={}", commit());

    // Rebuild when HEAD moves, so the recorded commit cannot lag the checkout.
    // Both paths are probed: `.git/HEAD` covers a normal clone, and the
    // packed-refs file covers a checkout whose branch ref has been packed.
    for path in [".git/HEAD", ".git/packed-refs"] {
        let candidate = std::path::Path::new("..").join("..").join(path);
        if candidate.exists() {
            println!("cargo:rerun-if-changed={}", candidate.display());
        }
    }
    println!("cargo:rerun-if-env-changed=CARTOGRAPH_BUILD_COMMIT");
}

/// The commit this binary was built from.
///
/// A release pipeline that builds from an exported archive sets
/// `CARTOGRAPH_BUILD_COMMIT` explicitly; that takes precedence so a published
/// binary can still name its source even with no repository present.
fn commit() -> String {
    if let Ok(explicit) = std::env::var("CARTOGRAPH_BUILD_COMMIT") {
        if !explicit.trim().is_empty() {
            return sanitise(&explicit);
        }
    }

    let output = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                "unknown".to_owned()
            } else {
                sanitise(trimmed)
            }
        }
        // No git, not a repository, or git failed. Not an error: the binary is
        // still valid, it simply cannot name its origin.
        _ => "unknown".to_owned(),
    }
}

/// Keeps the recorded commit to characters that cannot break a build flag or
/// leak an arbitrary string into `--json` output.
fn sanitise(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(40)
        .collect();
    if cleaned.is_empty() {
        "unknown".to_owned()
    } else {
        cleaned
    }
}
