//! Writing results to stdout.
//!
//! # Why output is rendered to a string first
//!
//! Two reasons, both learned from things that break real tools.
//!
//! `cartograph . | head -5` closes the pipe after five lines. Rust ignores
//! `SIGPIPE`, so the next `println!` returns `EPIPE` and panics — the user
//! sees a Rust panic message for doing something completely ordinary. Writing
//! once, through a function that treats a broken pipe as a normal end, makes
//! the composition work.
//!
//! And a command whose output is a value rather than a sequence of side
//! effects can be compared byte-for-byte in a test, which is what the golden
//! fixtures in `tests/golden/` rely on.

use std::io::Write;

use crate::error::ExitCode;

/// Writes `text` to stdout.
///
/// A broken pipe is success: the reader stopped listening, which is what
/// `head` does by design and not a failure of this program.
pub fn emit(text: &str) -> ExitCode {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    match handle
        .write_all(text.as_bytes())
        .and_then(|()| handle.flush())
    {
        Ok(()) => ExitCode::Success,
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::Success,
        Err(_) => ExitCode::Analysis,
    }
}

/// Writes a human-facing message to stderr, ignoring a broken pipe.
pub fn emit_error(text: &str) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = writeln!(handle, "{text}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_to_a_live_stdout_succeeds() {
        assert_eq!(emit(""), ExitCode::Success);
    }
}
