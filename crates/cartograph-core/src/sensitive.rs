//! What must never be written down, and how to find it.
//!
//! # Why this is one module rather than one rule per caller
//!
//! RULE 015 says secrets, environment values, source contents and tokens never
//! reach a log. Two places in the product have to act on that, and they act
//! differently: an evidence bundle **refuses** to carry a value it should not
//! (`cartograph_graph::bundle`), and the tracing layer **rewrites** one that
//! reaches a log field (`cartograph_pipeline::redaction`).
//!
//! Different actions, one definition. A security rule kept in two places is a
//! security rule that will eventually be true in one of them.
//!
//! # Shapes, not cleverness
//!
//! The shapes are QG-005's, restated here so the same rule the gate applies to
//! tracked files applies to values at runtime. This is deliberately not a
//! general secret scanner: it recognises the forms this project has decided
//! matter, and a form it has never heard of is not caught. Defence in depth
//! sits under it — `SourceLocation` refuses absolute paths at construction,
//! `NodeKind::EnvVar` has no value field, and `RepositoryIdentity` redacts its
//! own `Debug`.
//!
//! # What is deliberately *not* sensitive
//!
//! A leading `/` is not a filesystem path. Route evidence legitimately reads
//! `GET /api/orders`, and a check that refused it would be a check nobody could
//! keep — it would fire on the product's ordinary output until someone turned
//! it off. Only a path rooted in somebody's home directory or on a drive is
//! treated as machine-specific.

use std::borrow::Cow;

/// The marker a redacted value is replaced with.
///
/// The same word `RepositoryIdentity`'s `Debug` already uses, so a reader
/// meets one vocabulary for "something was removed here" rather than two.
pub const REDACTED: &str = "<redacted>";

/// A kind of value that must not be written down.
///
/// Deliberately a category rather than the value: naming the category is
/// enough to debug, and repeating the value would put it in the very log the
/// check exists to keep it out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Sensitive {
    /// A path inside somebody's home directory, or rooted on a drive.
    MachinePath,
    /// A credential-shaped string: an API token, or a private key block.
    Secret,
}

impl std::fmt::Display for Sensitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MachinePath => "a machine-specific path",
            Self::Secret => "a credential-shaped string",
        })
    }
}

/// The first kind of sensitive content in `text`, if any.
///
/// For callers that refuse rather than rewrite.
#[must_use]
pub fn found_in(text: &str) -> Option<Sensitive> {
    if text.split_whitespace().any(is_machine_path) {
        return Some(Sensitive::MachinePath);
    }
    if has_secret(text) {
        return Some(Sensitive::Secret);
    }
    None
}

/// Replaces sensitive content in `text`, leaving everything else alone.
///
/// Replacement is **whole-token**: a run of non-whitespace containing anything
/// sensitive is replaced entirely. Redacting only the matched span would leave
/// its neighbours behind — `key=ghp_…` would keep the part that says a
/// credential was there and lose only the proof — and a partly-redacted secret
/// is still a secret. Over-redacting a token costs a reader some context;
/// under-redacting one costs them the guarantee.
///
/// Returns [`Cow::Borrowed`] when nothing matched, so the common case
/// allocates nothing.
#[must_use]
pub fn redact(text: &str) -> Cow<'_, str> {
    if found_in(text).is_none() {
        return Cow::Borrowed(text);
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    // Rebuilt token by token, preserving the original whitespace between them
    // so a redacted line still reads as the line it was.
    while !rest.is_empty() {
        let lead = rest.len() - rest.trim_start().len();
        out.push_str(&rest[..lead]);
        rest = &rest[lead..];
        if rest.is_empty() {
            break;
        }

        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let token = &rest[..end];
        if is_machine_path(token) || has_secret(token) {
            out.push_str(REDACTED);
        } else {
            out.push_str(token);
        }
        rest = &rest[end..];
    }

    Cow::Owned(out)
}

/// Whether one whitespace-free token is a machine-specific path.
fn is_machine_path(token: &str) -> bool {
    has_home_path(token) || has_drive_path(token)
}

/// A path inside somebody's home directory: `/Users/<name>/`, `/home/<name>/`.
///
/// The name must be followed by a separator, which is what distinguishes a
/// real account from the bare directory and matches QG-005's own shape.
fn has_home_path(text: &str) -> bool {
    ["/Users/", "/home/"].iter().any(|prefix| {
        text.match_indices(prefix).any(|(at, _)| {
            let rest = &text[at + prefix.len()..];
            rest.split_once(['/', '\\']).is_some_and(|(name, _)| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
            })
        })
    })
}

/// A drive-rooted path: `C:\...` or `C:/...`.
fn has_drive_path(text: &str) -> bool {
    text.as_bytes().windows(3).any(|window| {
        window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && (window[2] == b'\\' || window[2] == b'/')
    })
}

/// A credential-shaped string, in QG-005's vocabulary.
fn has_secret(text: &str) -> bool {
    const GITHUB: [&str; 5] = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"];

    if text.contains("-----BEGIN ") && text.contains("PRIVATE KEY") {
        return true;
    }
    if GITHUB.iter().any(|prefix| {
        text.match_indices(prefix).any(|(at, _)| {
            text[at + prefix.len()..]
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .count()
                >= 20
        })
    }) {
        return true;
    }
    if text.match_indices("AKIA").any(|(at, _)| {
        text[at + 4..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            .count()
            >= 16
    }) {
        return true;
    }
    ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"]
        .iter()
        .any(|prefix| text.contains(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Credential shapes are assembled at runtime.
    ///
    /// QG-005 scans tracked files for exactly these forms and is right to: a
    /// credential-shaped literal in a fixture is still one in the history.
    /// Joining the parts here keeps the file clean while the test still
    /// exercises the real shape.
    fn github_token() -> String {
        format!("{}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789")
    }

    fn aws_key() -> String {
        format!("{}{}", "AKIA", "ABCDEFGHIJKLMNOP")
    }

    fn slack_token() -> String {
        format!("{}{}", "xoxb-", "0000000000-not-a-real-token")
    }

    fn private_key() -> String {
        format!("-----BEGIN {} PRIVATE KEY-----", "RSA")
    }

    // -- what is sensitive ------------------------------------------

    #[test]
    fn a_unix_home_path_is_sensitive() {
        assert_eq!(
            found_in("resolved from /home/username/src/app.py"),
            Some(Sensitive::MachinePath)
        );
        assert_eq!(
            found_in("/Users/username/code/app.ts"),
            Some(Sensitive::MachinePath)
        );
    }

    #[test]
    fn a_drive_rooted_path_is_sensitive() {
        assert_eq!(
            found_in(r"C:\Users\username\app.py"),
            Some(Sensitive::MachinePath)
        );
        assert_eq!(found_in("D:/dev/thing"), Some(Sensitive::MachinePath));
    }

    #[test]
    fn credential_shapes_are_sensitive() {
        assert_eq!(found_in(&github_token()), Some(Sensitive::Secret));
        assert_eq!(found_in(&aws_key()), Some(Sensitive::Secret));
        assert_eq!(found_in(&private_key()), Some(Sensitive::Secret));
        assert_eq!(found_in(&slack_token()), Some(Sensitive::Secret));
    }

    // -- what is deliberately not ------------------------------------

    #[test]
    fn a_route_is_not_a_machine_path() {
        // The product's ordinary output. A rule that fired here is a rule
        // somebody would switch off.
        for safe in [
            "GET /api/orders",
            "UNKNOWN /api/orders matched UNKNOWN /api/orders",
            "/api/orders/{id}",
            "/health",
        ] {
            assert_eq!(found_in(safe), None, "false positive on {safe}");
        }
    }

    #[test]
    fn a_repository_relative_path_is_not_sensitive() {
        for safe in [
            "api/routes.py:8",
            "crates/cartograph-graph/src/bundle.rs",
            "web/src/orders.ts",
        ] {
            assert_eq!(found_in(safe), None, "false positive on {safe}");
        }
    }

    #[test]
    fn ordinary_evidence_is_not_sensitive() {
        for safe in [
            "Order maps to table \"orders\"; db_table = \"orders\"",
            "Order.objects.all(...) in list_orders at api/routes.py:8",
            "neither side declared a method; every path segment static and equal",
        ] {
            assert_eq!(found_in(safe), None, "false positive on {safe}");
        }
    }

    #[test]
    fn a_bare_prefix_without_a_body_is_not_a_secret() {
        // The detector must need the shape, not the hint, or the word "AKIA"
        // in a sentence would redact the sentence.
        assert_eq!(found_in("ghp_"), None);
        assert_eq!(found_in("AKIA"), None);
        assert_eq!(found_in("the prefix ghp_ identifies a GitHub token"), None);
    }

    // -- redaction ---------------------------------------------------

    #[test]
    fn nothing_sensitive_is_returned_untouched_and_unallocated() {
        let safe = "GET /api/orders matched at api/routes.py:6";
        assert!(matches!(redact(safe), Cow::Borrowed(_)));
        assert_eq!(redact(safe), safe);
    }

    #[test]
    fn a_secret_is_replaced_and_its_neighbours_are_kept() {
        let line = format!("authenticated with {} for the run", github_token());
        let out = redact(&line);

        assert!(!out.contains("ghp_"), "the token survived: {out}");
        assert!(
            out.contains("authenticated with"),
            "context was lost: {out}"
        );
        assert!(out.contains("for the run"), "context was lost: {out}");
        assert!(out.contains(REDACTED));
    }

    #[test]
    fn a_machine_path_is_replaced_whole() {
        let out = redact("analysing /home/username/secret-project/app.py now");

        assert!(!out.contains("/home/"));
        assert!(
            !out.contains("secret-project"),
            "a partly redacted path still names the directory: {out}"
        );
        assert!(out.contains("analysing") && out.contains("now"));
    }

    #[test]
    fn every_sensitive_token_in_a_line_is_replaced() {
        let line = format!(
            "{} then /home/username/x then {}",
            github_token(),
            aws_key()
        );
        let out = redact(&line);

        assert!(!out.contains("ghp_"));
        assert!(!out.contains("AKIA"));
        assert!(!out.contains("/home/"));
        assert_eq!(out.matches(REDACTED).count(), 3);
    }

    #[test]
    fn redaction_preserves_the_shape_of_the_line() {
        let out = redact("before /home/username/x after");
        assert_eq!(out, format!("before {REDACTED} after"));
    }

    #[test]
    fn a_route_survives_redaction_unchanged() {
        // The property that keeps this rule enforceable.
        let line = "GET /api/orders matched GET /api/orders at api/routes.py:6";
        assert_eq!(redact(line), line);
    }

    #[test]
    fn an_empty_value_is_handled() {
        assert_eq!(redact(""), "");
        assert_eq!(found_in(""), None);
    }
}
