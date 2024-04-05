use std::error::Error;

#[derive(Debug)]
pub enum ZError {
    FileError(std::io::Error),
    JsonError(serde_json::Error),
    Message(String),
}

impl Error for ZError {
    fn description(&self) -> &str {
        match *self {
            ZError::FileError(ref err) => err.description(),
            ZError::JsonError(ref err) => err.description(),
            ZError::Message(ref s) => &s,
        }
    }

    // Not sure what these do
    // fn source(&self) -> Option<&(dyn Error + 'static)> {
    //     None
    // }

    // fn cause(&self) -> Option<&dyn Error> {
    //     self.source()
    // }
}

impl std::fmt::Display for ZError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ZError::FileError(ref err) => err.fmt(f),
            ZError::JsonError(ref err) => err.fmt(f),
            ZError::Message(ref err) => err.fmt(f),
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

pub type Result<T> = std::result::Result<T, ZError>;
