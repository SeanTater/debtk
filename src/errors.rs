use crate::Position;
use std::io;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum CsvError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("Ambiguous parse ({0:?}): {1}")]
    Ambiguity(Position, &'static str),

    #[error("Invalid input ({0:?}): {1}")]
    Invalid(Position, &'static str),
    // Add more custom variants as needed
}

impl From<io::Error> for CsvError {
    fn from(error: io::Error) -> Self {
        CsvError::Io(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CsvError>;
