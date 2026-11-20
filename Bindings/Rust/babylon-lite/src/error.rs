use crate::sys;
use std::fmt;

/// Failure status reported by Babylon Lite (`bl_Status` other than `BL_OK`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Status {
    InvalidArgument,
    InvalidHandle,
    Disposed,
    WrongRuntime,
    WrongEngine,
    WrongThread,
    OutOfMemory,
    Unsupported,
    NotReady,
    Busy,
    Cancelled,
    Stopped,
    DeviceLost,
    ShaderError,
    HostError,
    AudioUnavailable,
    AudioSuspended,
    /// A status this projection does not know yet.
    Other(i32),
}

impl Status {
    pub(crate) fn from_raw(status: sys::bl_Status) -> Self {
        match status {
            sys::bl_Status::BL_INVALID_ARGUMENT => Self::InvalidArgument,
            sys::bl_Status::BL_INVALID_HANDLE => Self::InvalidHandle,
            sys::bl_Status::BL_DISPOSED => Self::Disposed,
            sys::bl_Status::BL_WRONG_RUNTIME => Self::WrongRuntime,
            sys::bl_Status::BL_WRONG_ENGINE => Self::WrongEngine,
            sys::bl_Status::BL_WRONG_THREAD => Self::WrongThread,
            sys::bl_Status::BL_OUT_OF_MEMORY => Self::OutOfMemory,
            sys::bl_Status::BL_UNSUPPORTED => Self::Unsupported,
            sys::bl_Status::BL_NOT_READY => Self::NotReady,
            sys::bl_Status::BL_BUSY => Self::Busy,
            sys::bl_Status::BL_CANCELLED => Self::Cancelled,
            sys::bl_Status::BL_STOPPED => Self::Stopped,
            sys::bl_Status::BL_DEVICE_LOST => Self::DeviceLost,
            sys::bl_Status::BL_SHADER_ERROR => Self::ShaderError,
            sys::bl_Status::BL_HOST_ERROR => Self::HostError,
            sys::bl_Status::BL_AUDIO_UNAVAILABLE => Self::AudioUnavailable,
            sys::bl_Status::BL_AUDIO_SUSPENDED => Self::AudioSuspended,
            other => Self::Other(other.0),
        }
    }
}

/// An error from a Babylon Lite call.
///
/// `operation` and `message` are present when the runtime reported the failure
/// through its error callback during the failing call; many argument and handle
/// checks fail with a status only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub status: Status,
    /// Original Babylon Lite error number, or 0.
    pub source_error_code: u32,
    pub operation: String,
    pub message: String,
}

impl Error {
    pub(crate) fn from_status(status: sys::bl_Status) -> Self {
        Self {
            status: Status::from_raw(status),
            source_error_code: 0,
            operation: String::new(),
            message: String::new(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.status)?;
        if !self.operation.is_empty() {
            write!(f, " in {}", self.operation)?;
        }
        if !self.message.is_empty() {
            write!(f, ": {}", self.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
