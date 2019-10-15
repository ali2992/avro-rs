use std::{io, fmt, result};

use serde::{de, ser};
use serde_json::Value as JsonValue;

use crate::schema::{Schema, Name};
use crate::types;
use std::rc::Rc;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Error(Box<ErrorKind>);

pub(crate) fn error(kind: ErrorKind) -> Error {
    Error(Box::new(kind))
}

impl Error {
    /// Return the specific type of this error.
    pub fn kind(&self) -> &ErrorKind {
        &self.0
    }

    /// Unwrap this error into its underlying type.
    pub fn into_kind(self) -> ErrorKind {
        *self.0
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        error(ErrorKind::Deserialization(msg.to_string()))
    }
}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        error(ErrorKind::Serialization(msg.to_string()))
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Self {
        error(ErrorKind::Deserialization(e.to_string()))
    }
}

impl From<ResolutionError> for Error {
    fn from(e: ResolutionError) -> Self {
        error(ErrorKind::Resolution(e))
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        error(ErrorKind::Io(err.to_string()))
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(_: std::string::FromUtf8Error) -> Error {
        error(ErrorKind::Resolution(ResolutionError::BytesToStringUtf8Error))
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(std::error::Error + 'static)> {
        match *self.0 {
            ErrorKind::Io(_) => None,//TODO Some(&err),
            ErrorKind::Generic(_) => None,
            ErrorKind::Resolution(_) => None,
            ErrorKind::SchemaParsing(_) => None,
            ErrorKind::Decode(_) => None,
            ErrorKind::Allocation(_) => None,
            ErrorKind::Serialization(_) => None,
            ErrorKind::Validation(_) => None,
            ErrorKind::Deserialization(_) => None
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            ErrorKind::Io(ref err) => write!(f, "IO error: {}", err),
            ErrorKind::Generic(ref err) => write!(f, "General error: {}", err),
            ErrorKind::SchemaParsing(ref err) => write!(f, "Schema parsing error: {}", err),
            ErrorKind::Allocation(ref err) => write!(f, "Allocation error: {}", err),
            ErrorKind::Decode(ref err) => write!(f, "Decode error: {}", err),
            ErrorKind::Validation(ref err) => write!(f, "Validation error: {}", err),
            ErrorKind::Deserialization(ref err) => write!(f, "Deserialization error: {}", err),
            ErrorKind::Serialization(ref err) => write!(f, "Serialization error: {}", err),
            ErrorKind::Resolution(ref err) => err.fmt(f)
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    Io(String), //TODO have this keep the actual io::Error, needs io::Error to be cloneable though..

    Resolution(ResolutionError),

    Deserialization(String),

    Serialization(String),

    Validation(String),

    Generic(String),

    Decode(String),

    SchemaParsing(String),

    Allocation(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionError {
    BytesToStringUtf8Error,

    IncompatibleSchema(Schema, Schema),

    IncompatibleData(types::Value),

    UnexpectedVariant(usize),

    RecordFieldMissing(Name, usize),

    RecordFieldValueMissing(Rc<String>),

    InvalidDefaultValue(JsonValue),

    Generic(String),
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolutionError::BytesToStringUtf8Error => write!(f, "(ResolutionError::BytesToStringUtf8Error)"),
            ResolutionError::IncompatibleSchema(writer, reader) => write!(f, "(ResolutionError::IncompatibleSchema: writer={:?}, reader={:?})", writer, reader),
            ResolutionError::IncompatibleData(v) => write!(f, "(ResolutionError::IncompatibleData, {:?})", v),
            ResolutionError::UnexpectedVariant(v) => write!(f, "(ResolutionError::UnexpectedVariant, variant={})", v),
            ResolutionError::RecordFieldMissing(record, idx) => write!(f, "(ResolutionError::RecordFieldMissing, record={:?}, field={})", record, idx),
            ResolutionError::RecordFieldValueMissing(v) => write!(f, "(ResolutionError::RecordFieldValueMissing, {})", v),
            ResolutionError::InvalidDefaultValue(json) => write!(f, "(ResolutionError::InvalidDefaultValue, {})", json),
            ResolutionError::Generic(msg) => write!(f, "(ResolutionError::Generic, {})", msg),
        }
    }
}