use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    DoubleAcquire,
    DoubleRelease,
    OutOfBounds,
    OutOfMemory,
    Io(std::io::Error),
    Utf8(std::str::Utf8Error),
    SliceIndexing(std::array::TryFromSliceError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DoubleAcquire => write!(f, "duble acquire"),
            Self::DoubleRelease => write!(f, "double release"),
            Self::OutOfBounds => write!(f, "out of bounds"),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Utf8(e) => write!(f, "{e}"),
            Self::SliceIndexing(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(value: std::str::Utf8Error) -> Self {
        Self::Utf8(value)
    }
}

impl From<std::array::TryFromSliceError> for Error {
    fn from(value: std::array::TryFromSliceError) -> Self {
        Self::SliceIndexing(value)
    }
}
