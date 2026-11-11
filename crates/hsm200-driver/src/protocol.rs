use crate::error::{DriverError, Result};

/// Guillemot / Hercules USB Vendor ID.
pub const VENDOR_ID: u16 = 0x06F8;

/// Hercules Stream 200 XLR (`HSM02`) Runtime USB Product ID.
pub const PRODUCT_ID_RUNTIME: u16 = 0xE054;

/// Hercules Stream 200 XLR (`HSM02`) DFU Bootloader USB Product ID.
pub const PRODUCT_ID_DFU: u16 = 0xE055;

/// Hercules Stream 100 (`HSM01`) Runtime USB Product ID.
pub const PRODUCT_ID_STREAM_100: u16 = 0xE053;

/// USB Vendor Specific Bulk Interface number on the Stream 200 XLR (`bInterfaceNumber = 3`).
pub const VENDOR_INTERFACE: u8 = 3;

/// Alternate setting on Interface 3 that exposes the two Bulk endpoints (`bAlternateSetting = 1`).
pub const VENDOR_ALT_SETTING: u8 = 1;

/// Bulk OUT endpoint address (Host -> Device).
pub const EP_BULK_OUT: u8 = 0x01;

/// Bulk IN endpoint address (Device -> Host).
pub const EP_BULK_IN: u8 = 0x81;

/// Maximum packet size for High-Speed USB Bulk endpoints on Interface 3.
pub const BULK_MAX_PACKET_SIZE: usize = 512;

/// Common 12-byte (`0x0C`) packet header size.
pub const HEADER_SIZE: usize = 12;

/// LCD screen width in pixels.
pub const LCD_WIDTH: u16 = 480;

/// LCD screen height in pixels.
pub const LCD_HEIGHT: u16 = 272;

/// Command IDs for the Stream 200 XLR Bulk protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CommandId {
    /// `0x0101`: Poll hardware inputs (buttons, rotary encoders, mixer state).
    PollStatus = 0x0101,
    /// `0x0301`: Configure AK7735EQ Microphone DSP sub-blocks.
    MicDspConfig = 0x0301,
    /// `0x0401`: Audio mixer routing, dB faders, mutes, 48V phantom power, mic source.
    AudioMixer = 0x0401,
    /// `0x0501`: RGB LED ring/meter colors and real-time VU meter levels.
    RgbMeters = 0x0501,
    /// `0x0601`: Physical button solid/blink LED bitmasks and LCD backlight brightness.
    ButtonLedsAndBacklight = 0x0601,
    /// `0x0701`: Chunked 8-bit paletted UI widget/icon upload.
    GraphicWidgetUpload = 0x0701,
    /// `0x0801`: Full-screen framebuffer upload, screen wake/sleep, and page/layer control.
    ScreenControl = 0x0801,
    /// `0x0901`: Device preset command.
    PresetCommand = 0x0901,
    /// `0x0A01`: Device state query.
    DeviceStateQuery = 0x0A01,
}

/// 12-byte little-endian header prepended to all Bulk OUT commands and Bulk IN responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    /// Total packet size in bytes (`12 + payload_len`).
    pub total_size: u32,
    /// Command identifier (e.g., `0x0101`..`0x0A01`).
    pub cmd_id: u16,
    /// Monotonically incrementing sequence number (`0..=255`).
    pub seq: u8,
    /// Reserved byte (`0x00`).
    pub reserved: u8,
    /// Status code (`0` on outbound commands; `0` on successful responses).
    pub status: u32,
}

impl PacketHeader {
    pub const fn new(cmd_id: CommandId, seq: u8, payload_len: usize) -> Self {
        Self {
            total_size: (HEADER_SIZE + payload_len) as u32,
            cmd_id: cmd_id as u16,
            seq,
            reserved: 0,
            status: 0,
        }
    }

    pub fn to_bytes(self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.total_size.to_le_bytes());
        buf[4..6].copy_from_slice(&self.cmd_id.to_le_bytes());
        buf[6] = self.seq;
        buf[7] = self.reserved;
        buf[8..12].copy_from_slice(&self.status.to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_SIZE {
            return Err(DriverError::ShortResponse {
                expected: HEADER_SIZE,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            total_size: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            cmd_id: u16::from_le_bytes([bytes[4], bytes[5]]),
            seq: bytes[6],
            reserved: bytes[7],
            status: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        })
    }
}
