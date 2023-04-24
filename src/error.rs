use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    DoubleAcquire,
    DoubleRelease,
    OutOfBounds,
    OutOfMemory,
    InsufficientBytes,
    ThreadSync,
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
            Self::InsufficientBytes => write!(f, "insufficient bytes"),
            Self::ThreadSync => write!(f, "thread synchronization"),
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

impl<T> From<std::sync::LockResult<T>> for Error {
    fn from(_: std::sync::LockResult<T>) -> Self {
        Self::ThreadSync
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self::ThreadSync
    }
}

impl From<Error> for libc::c_int {
    fn from(value: Error) -> Self {
        match value {
            Error::DoubleAcquire => 1,
            Error::DoubleRelease => 2,
            Error::OutOfBounds => 3,
            Error::OutOfMemory => 4,
            Error::InsufficientBytes => 5,
            Error::ThreadSync => 6,
            Error::Io(_) => 7,
            Error::Utf8(_) => 8,
            Error::SliceIndexing(_) => 9,
        }
    }
}
