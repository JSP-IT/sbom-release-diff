use std::fmt;

/// An input or usage error.
///
/// Messages are built only from text written in this crate, JSON locations
/// such as `components[3].name`, and the input path the user supplied.
/// Values taken from an SBOM are never copied into an error message, so a
/// rejected document cannot use the error path to print its own content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Adds context in front of the message, for example the input label.
    #[must_use]
    pub fn context(self, context: impl fmt::Display) -> Self {
        Self {
            message: format!("{context}: {}", self.message),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
