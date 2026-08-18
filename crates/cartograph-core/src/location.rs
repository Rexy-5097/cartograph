//! Where in the source a claim was observed.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// A position in a repository file.
///
/// # Why paths are repository-relative
///
/// Two invariants meet here. A graph must be portable — the same repository
/// analysed on two machines should produce comparable graphs, which absolute
/// paths would prevent. And no local filesystem layout may leak into a
/// committed artefact, a log line or, later, an MCP response. Rejecting
/// absolute paths at construction makes both properties structural rather than
/// a convention someone has to remember.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    file: String,
    line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u32>,
}

impl SourceLocation {
    /// Constructs a location from a repository-relative path and a 1-based line.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Empty`] for an empty path,
    /// [`CoreError::AbsolutePath`] for an absolute path,
    /// [`CoreError::PathEscapesRepository`] for a path containing a `..`
    /// component, and [`CoreError::ZeroIndexed`] for a zero line number.
    pub fn new(file: impl Into<String>, line: u32) -> Result<Self, CoreError> {
        let file = file.into();

        if file.trim().is_empty() {
            return Err(CoreError::Empty { field: "file" });
        }
        // Covers POSIX roots and Windows drive-absolute and UNC paths, since a
        // graph may be produced on one platform and read on another.
        let is_absolute = file.starts_with('/')
            || file.starts_with('\\')
            || file
                .as_bytes()
                .get(1)
                .is_some_and(|&b| b == b':' && file.is_char_boundary(2));
        if is_absolute {
            return Err(CoreError::AbsolutePath { path: file });
        }
        if file.split(['/', '\\']).any(|part| part == "..") {
            return Err(CoreError::PathEscapesRepository { path: file });
        }
        if line == 0 {
            return Err(CoreError::ZeroIndexed { field: "line" });
        }

        Ok(Self {
            file,
            line,
            column: None,
        })
    }

    /// Adds a 1-based column.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ZeroIndexed`] if `column` is zero.
    pub fn with_column(mut self, column: u32) -> Result<Self, CoreError> {
        if column == 0 {
            return Err(CoreError::ZeroIndexed { field: "column" });
        }
        self.column = Some(column);
        Ok(self)
    }

    /// The repository-relative path.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The 1-based line.
    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }

    /// The 1-based column, if known.
    ///
    /// `None` means the column was never determined — it does not mean column
    /// zero, and it must not be rendered as one.
    #[must_use]
    pub fn column(&self) -> Option<u32> {
        self.column
    }
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.column {
            Some(column) => write!(f, "{}:{}:{}", self.file, self.line, column),
            None => write!(f, "{}:{}", self.file, self.line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_way_the_specification_writes_it() {
        let location = SourceLocation::new("src/components/CheckoutButton.tsx", 34).unwrap();
        assert_eq!(location.to_string(), "src/components/CheckoutButton.tsx:34");
        assert_eq!(
            location.with_column(12).unwrap().to_string(),
            "src/components/CheckoutButton.tsx:34:12"
        );
    }

    #[test]
    fn rejects_absolute_paths_on_every_platform() {
        for path in [
            "/Volumes/External/checkout/src/main.rs",
            "/home/dev/app/src/main.rs",
            "C:\\Users\\dev\\app\\src\\main.rs",
            "\\\\server\\share\\main.rs",
        ] {
            assert!(
                matches!(
                    SourceLocation::new(path, 1),
                    Err(CoreError::AbsolutePath { .. })
                ),
                "accepted absolute path `{path}`; a local filesystem layout \
                 would leak into the graph"
            );
        }
    }

    #[test]
    fn rejects_paths_escaping_the_repository() {
        assert!(matches!(
            SourceLocation::new("../secrets/.env", 1),
            Err(CoreError::PathEscapesRepository { .. })
        ));
        assert!(matches!(
            SourceLocation::new("src/../../etc/passwd", 1),
            Err(CoreError::PathEscapesRepository { .. })
        ));
    }

    #[test]
    fn allows_dot_dot_inside_a_filename() {
        // `..` is only a traversal when it is a whole path component.
        assert!(SourceLocation::new("src/weird..name.ts", 1).is_ok());
    }

    #[test]
    fn rejects_zero_based_positions() {
        assert!(matches!(
            SourceLocation::new("src/main.rs", 0),
            Err(CoreError::ZeroIndexed { field: "line" })
        ));
        let location = SourceLocation::new("src/main.rs", 1).unwrap();
        assert!(matches!(
            location.with_column(0),
            Err(CoreError::ZeroIndexed { field: "column" })
        ));
    }

    #[test]
    fn rejects_an_empty_path() {
        assert!(matches!(
            SourceLocation::new("   ", 1),
            Err(CoreError::Empty { field: "file" })
        ));
    }

    #[test]
    fn omits_an_unknown_column_from_json_rather_than_inventing_one() {
        let json = serde_json::to_string(&SourceLocation::new("a.ts", 3).unwrap()).unwrap();
        assert_eq!(json, r#"{"file":"a.ts","line":3}"#);
    }
}
