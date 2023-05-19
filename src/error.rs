use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    DoubleAcquire,
    DoubleRelease,
    OutOfBounds,
    OutOfMemory,
    InsufficientBytes,
    ThreadSync,
    NameOrInodeDuplicate,
    NotFound,
    NullBlock,
    Io(std::io::Error),
    Utf8(std::str::Utf8Error),
    SliceIndexing(std::array::TryFromSliceError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            DoubleAcquire => write!(f, "duble acquire"),
            DoubleRelease => write!(f, "double release"),
            OutOfBounds => write!(f, "out of bounds"),
            OutOfMemory => write!(f, "out of memory"),
            InsufficientBytes => write!(f, "insufficient bytes"),
            ThreadSync => write!(f, "thread synchronization"),
            NameOrInodeDuplicate => write!(f, "name or inode duplicate"),
            NotFound => write!(f, "not found"),
            NullBlock => write!(f, "null block"),
            Io(e) => write!(f, "{e}"),
            Utf8(e) => write!(f, "{e}"),
            SliceIndexing(e) => write!(f, "{e}"),
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
        use libc::*;
        use Error::*;
        match value {
            DoubleAcquire => EIO,
            DoubleRelease => EBADF,
            OutOfBounds => ESPIPE,
            OutOfMemory => ENOSPC,
            InsufficientBytes => ENOBUFS,
            ThreadSync => EDEADLOCK,
            NameOrInodeDuplicate => EEXIST,
            NotFound => ENOENT,
            NullBlock => ESPIPE,
            Io(_) => EIO,
            Utf8(_) => EBADMSG,
            SliceIndexing(_) => ENOBUFS,
        }
    }
}
