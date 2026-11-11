use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("Hercules Stream 200 XLR (06f8:e054) not found on USB bus")]
    DeviceNotFound,

    #[error("Device is in DFU bootloader mode (06f8:e055) rather than runtime mode (06f8:e054)")]
    DeviceInDfuMode,

    #[error("USB permission denied opening device (install udev/99-hercules-stream-200xlr.rules or run with sudo): {0}")]
    PermissionDenied(rusb::Error),

    #[error("USB transport error: {0}")]
    Usb(#[from] rusb::Error),

    #[error("Short USB response: expected at least {expected} bytes, got {actual} bytes")]
    ShortResponse { expected: usize, actual: usize },

    #[error("Unexpected response command ID: expected 0x{expected:04X}, got 0x{actual:04X}")]
    CommandMismatch { expected: u16, actual: u16 },

    #[error("Device returned non-zero status code 0x{status:08X} for command 0x{cmd_id:04X}")]
    DeviceStatusError { cmd_id: u16, status: u32 },

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

pub type Result<T> = std::result::Result<T, DriverError>;
