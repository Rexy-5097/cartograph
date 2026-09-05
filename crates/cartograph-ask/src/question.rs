//! The user's question — the only thing in a provider request that is not
//! already-derived evidence.
//!
//! It is a validated newtype rather than a `String` so that the one free-text
//! field crossing the boundary has a stated shape: non-empty, and bounded. A
//! bound matters because the rest of the request is capped by the bundle, and
//! an unbounded field would make the whole payload unbounded.

use std::fmt;

/// How long a question may be, in bytes.
///
/// Generous for a sentence and far below any model's context. Bytes rather
/// than characters because bytes are what is sent, and the check must not
/// depend on how the text is encoded.
pub const MAX_QUESTION_BYTES: usize = 2048;

/// Why a question was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuestionError {
    /// The question was empty, or nothing but whitespace.
    Empty,
    /// The question was longer than [`MAX_QUESTION_BYTES`].
    ///
    /// Carries the limit, never the question: an error message is a log line.
    TooLong {
        /// The limit that was exceeded, in bytes.
        limit: usize,
    },
}

impl fmt::Display for QuestionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a question must ask something"),
            Self::TooLong { limit } => write!(f, "a question may be at most {limit} bytes"),
        }
    }
}

impl std::error::Error for QuestionError {}

/// One question about an artefact, bounded and non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question(String);

impl Question {
    /// Accepts a question, or says why it cannot be one.
    ///
    /// The text is stored **exactly as given**. Nothing is trimmed, reflowed or
    /// re-encoded: the emptiness check reads the text, it does not rewrite it.
    ///
    /// # Errors
    ///
    /// [`QuestionError::Empty`] for an empty or whitespace-only question, and
    /// [`QuestionError::TooLong`] beyond [`MAX_QUESTION_BYTES`].
    pub fn new(text: impl Into<String>) -> Result<Self, QuestionError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(QuestionError::Empty);
        }
        if text.len() > MAX_QUESTION_BYTES {
            return Err(QuestionError::TooLong {
                limit: MAX_QUESTION_BYTES,
            });
        }
        Ok(Self(text))
    }

    /// The question, byte for byte as it was given.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_is_kept_exactly_as_written() {
        let question = Question::new("  Why does the client reach the users table?  ")
            .expect("a real question");

        assert_eq!(
            question.text(),
            "  Why does the client reach the users table?  ",
            "the question was rewritten on the way in"
        );
    }

    #[test]
    fn an_empty_question_is_refused() {
        assert_eq!(Question::new(""), Err(QuestionError::Empty));
        assert_eq!(Question::new("   \n\t "), Err(QuestionError::Empty));
    }

    #[test]
    fn an_unbounded_question_is_refused() {
        let long = "q".repeat(MAX_QUESTION_BYTES + 1);

        assert_eq!(
            Question::new(long),
            Err(QuestionError::TooLong {
                limit: MAX_QUESTION_BYTES
            })
        );
    }

    #[test]
    fn the_limit_is_inclusive() {
        let exact = "q".repeat(MAX_QUESTION_BYTES);
        assert!(Question::new(exact).is_ok());
    }

    #[test]
    fn a_refusal_never_repeats_the_question() {
        let secretish = "please use my key ghp_notarealtokenatall";
        let long = format!("{secretish}{}", "x".repeat(MAX_QUESTION_BYTES));

        let error = Question::new(long).expect_err("too long");
        let rendered = format!("{error} {error:?}");

        assert!(
            !rendered.contains(secretish),
            "error echoed input: {rendered}"
        );
    }
}
