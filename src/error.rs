use std::fmt;

#[derive(Debug)]
pub enum ParserError {
    Io(std::io::Error),
    Parse(String),
    Validation(String),
    UnsupportedFormat,
    Conversion(String),
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParserError::Io(e) => write!(f, "IO error: {}", e),
            ParserError::Parse(msg) => write!(f, "Parse error: {}", msg),
            ParserError::Validation(msg) => write!(f, "Validation error: {}", msg),
            ParserError::UnsupportedFormat => write!(f, "Unsupported format"),
            ParserError::Conversion(msg) => write!(f, "Conversion error: {}", msg),
        }
    }
}

impl std::error::Error for ParserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParserError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ParserError {
    fn from(error: std::io::Error) -> Self {
        ParserError::Io(error)
    }
}
