use std::fmt;

#[derive(Debug, Clone)]
pub struct ParsedSelection {
    pub pages: Vec<u32>,
    pub groups: Vec<Vec<u32>>,
}

#[derive(Debug, Clone)]
pub struct PdfToolError(pub(crate) String);

impl PdfToolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PdfToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PdfToolError {}

pub type Result<T> = std::result::Result<T, PdfToolError>;
