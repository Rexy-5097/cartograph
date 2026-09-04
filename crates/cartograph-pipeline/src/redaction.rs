//! Redaction at the tracing layer, so no call site has to remember it.
//!
//! # Why here, and why now
//!
//! `SECURITY.md` states as an invariant that "Logs never carry secrets…
//! Redaction happens at the tracing layer", and `docs/security/README.md`
//! schedules the mechanical form for "the first LSP/network code". Until M16
//! there was none, so the invariant held by discipline: the CLI's
//! `init_tracing` carries a comment saying nothing may log a file's text, and
//! nothing enforced it.
//!
//! ADR-0021 puts this before the first network request rather than after,
//! because a redaction rule that arrives with the code it protects has already
//! been shipped without it once.
//!
//! # Why in `cartograph-pipeline`
//!
//! The same reasoning ADR-0020 used for the authorization guard: it sits below
//! the transport so it is true for *every* client, rather than for the one that
//! happens to be well behaved. Every client — CLI, MCP, desktop — depends on
//! this crate; nothing depends on `cartograph-cli`, so a layer living there
//! could not protect the other two. The rule this applies is defined once, in
//! [`cartograph_core::sensitive`].
//!
//! # Why a field formatter rather than a filter
//!
//! A `Layer` can observe an event but cannot rewrite what the `fmt` layer
//! prints, because `fmt` formats from the original values. [`Redacting`]
//! replaces the field formatter itself, which is the last point before bytes
//! reach a sink — so it covers event fields, span fields and the `message`
//! field alike, including values written by code that has not been added yet.
//!
//! Redaction is **structural, not textual post-processing**: each field's value
//! is inspected on its own, so a field name is never mistaken for a value and a
//! redaction can never span two fields.
//!
//! # What it does not do
//!
//! It does not stop a caller writing a secret into a *field name*, and it does
//! not reach output written with `println!` past tracing entirely. Those remain
//! review matters. It is defence in depth under the structural guarantees that
//! already exist — `SourceLocation` refusing absolute paths, `EnvVar` holding
//! no value, `RepositoryIdentity` redacting its own `Debug` — not a substitute
//! for them.

use std::fmt;

use cartograph_core::sensitive;
use tracing::field::{Field, Visit};
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::fmt::format::{FormatFields, Writer};

/// A field formatter that redacts sensitive values on their way to a sink.
///
/// Install it on the `fmt` builder with `.fmt_fields(Redacting)`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Redacting;

impl<'writer> FormatFields<'writer> for Redacting {
    fn format_fields<R: RecordFields>(&self, writer: Writer<'writer>, fields: R) -> fmt::Result {
        let mut visitor = RedactingVisitor {
            writer,
            result: Ok(()),
            first: true,
        };
        fields.record(&mut visitor);
        visitor.result
    }
}

/// Writes each field, with its value passed through the redaction rule.
struct RedactingVisitor<'writer> {
    writer: Writer<'writer>,
    result: fmt::Result,
    first: bool,
}

impl RedactingVisitor<'_> {
    /// Writes one field, redacting the value and never the name.
    ///
    /// The name is written as given: a name is chosen in source, not supplied
    /// at runtime, so it cannot carry a value that arrived from outside. The
    /// value is the part that can.
    fn write(&mut self, field: &Field, value: &str) {
        if self.result.is_err() {
            return;
        }

        let separator = if self.first { "" } else { " " };
        self.first = false;
        let redacted = sensitive::redact(value);

        // `message` is the event's own text and is printed bare, matching the
        // default formatter so installing this does not reshape every log line.
        self.result = if field.name() == "message" {
            write!(self.writer, "{separator}{redacted}")
        } else {
            write!(self.writer, "{separator}{}={redacted}", field.name())
        };
    }
}

impl Visit for RedactingVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.write(field, value);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.write(field, &value.to_string());
    }

    /// The catch-all.
    ///
    /// `Visit`'s other methods default to this one, so every integer, boolean,
    /// float and arbitrary `Debug` value lands here and is inspected. A value
    /// type added later is covered without anyone deciding to cover it.
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.write(field, &format!("{value:?}"));
    }
}
