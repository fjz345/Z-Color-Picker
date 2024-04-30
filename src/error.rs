use std::fmt::Display;

#[derive(Debug)]
pub enum ZError {
    FileError(std::io::Error),
    JsonError(serde_json::Error),
    Message(String),
    Clipboard(arboard::Error),
}

impl Display for ZError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ZError::FileError(ref err) => std::fmt::Display::fmt(&err, f),
            ZError::JsonError(ref err) => std::fmt::Display::fmt(&err, f),
            ZError::Message(ref err) => std::fmt::Display::fmt(&err, f),
            ZError::Clipboard(ref err) => std::fmt::Display::fmt(&err, f),
        }
    }
}

impl From<std::io::Error> for ZError {
    fn from(err: std::io::Error) -> ZError {
        ZError::FileError(err)
    }
}

impl From<serde_json::Error> for ZError {
    fn from(err: serde_json::Error) -> ZError {
        ZError::JsonError(err)
    }
}

impl From<String> for ZError {
    fn from(err: String) -> ZError {
        ZError::Message(err)
    }
}

impl From<arboard::Error> for ZError {
    fn from(err: arboard::Error) -> ZError {
        ZError::Message(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ZError>;
