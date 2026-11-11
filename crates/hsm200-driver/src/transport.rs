use std::time::Duration;

use rusb::{Context, DeviceHandle, UsbContext};

use crate::error::{DriverError, Result};
use crate::protocol::{
    BULK_MAX_PACKET_SIZE, CommandId, EP_BULK_IN, EP_BULK_OUT, HEADER_SIZE, PacketHeader,
    PRODUCT_ID_DFU, PRODUCT_ID_RUNTIME, VENDOR_ALT_SETTING, VENDOR_ID, VENDOR_INTERFACE,
};

/// Information about a connected Hercules Stream 200 XLR device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub bus_number: u8,
    pub address: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub firmware_bcd: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial_number: String,
    /// Protocol readiness byte queried via EP0 Vendor Request `0x10`.
    pub protocol_version: u16,
    /// Maximum Bulk IN packet buffer size reported by firmware via EP0 Vendor Request `0x01` (`wValue=0`).
    pub rx_max_transfer_size: u32,
    /// Maximum Bulk OUT packet buffer size reported by firmware via EP0 Vendor Request `0x01` (`wValue=1`).
    pub tx_max_transfer_size: u32,
}

impl DeviceInfo {
    /// Formats `bcdDevice` (e.g., `0x0239`) as a human-readable version string (`"2.39"`).
    pub fn firmware_version_string(&self) -> String {
        let major = (self.firmware_bcd >> 8) & 0xFF;
        let minor_tens = (self.firmware_bcd >> 4) & 0x0F;
        let minor_ones = self.firmware_bcd & 0x0F;
        format!("{major}.{minor_tens}{minor_ones}")
    }
}

/// Synchronous USB Bulk transport for the Hercules Stream 200 XLR (`06f8:e054`).
pub struct UsbTransport {
    handle: DeviceHandle<Context>,
    info: DeviceInfo,
    seq: u8,
    timeout: Duration,
    rx_buf: Vec<u8>,
}

impl UsbTransport {
    /// Opens the first available Hercules Stream 200 XLR (`06f8:e054`) on the USB bus,
    /// claims Interface 3, queries EP0 vendor buffer parameters, and selects Alternate Setting 1
    /// (`EP 0x01 OUT`, `EP 0x81 IN`).
    pub fn open() -> Result<Self> {
        let context = Context::new()?;
        let devices = context.devices()?;

        let mut found_dfu = false;

        for device in devices.iter() {
            let desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };

            if desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID_DFU {
                found_dfu = true;
            }

            if desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID_RUNTIME {
                let handle = device.open().map_err(|e| match e {
                    rusb::Error::Access => DriverError::PermissionDenied(e),
                    other => DriverError::Usb(other),
                })?;

                let timeout = Duration::from_millis(5000);
                let manufacturer = handle
                    .read_manufacturer_string_ascii(&desc)
                    .unwrap_or_else(|_| "Hercules".to_string());
                let product = handle
                    .read_product_string_ascii(&desc)
                    .unwrap_or_else(|_| "Hercules Stream 200 XLR".to_string());
                let serial_number = handle
                    .read_serial_number_string_ascii(&desc)
                    .unwrap_or_default();

                let v = desc.device_version();
                let firmware_bcd =
                    ((v.major() as u16) << 8) | ((v.minor() as u16) << 4) | (v.sub_minor() as u16);

                // Detach kernel driver on Interface 3 if attached, then claim Interface 3.
                if handle.kernel_driver_active(VENDOR_INTERFACE).unwrap_or(false) {
                    let _ = handle.detach_kernel_driver(VENDOR_INTERFACE);
                }
                handle
                    .claim_interface(VENDOR_INTERFACE)
                    .map_err(|e| match e {
                        rusb::Error::Access => DriverError::PermissionDenied(e),
                        other => DriverError::Usb(other),
                    })?;

                // Per hsm_api_core_x64.dll (0x180013fc5..0x18001409f):
                // Query Vendor-to-Interface IN requests (bmRequestType = 0xC1, wIndex = 3)
                // before switching to Alternate Setting 1:
                // 1) bRequest = 0x10, wValue = 0 -> 2-byte protocol status (byte 0 == 1)
                // 2) bRequest = 0x01, wValue = 0 -> 4-byte RX max buffer size
                // 3) bRequest = 0x01, wValue = 1 -> 4-byte TX max buffer size
                let mut proto_buf = [0u8; 2];
                let protocol_version = if handle
                    .read_control(
                        0xC1,
                        0x10,
                        0x0000,
                        VENDOR_INTERFACE as u16,
                        &mut proto_buf,
                        timeout,
                    )
                    .is_ok()
                {
                    u16::from_le_bytes(proto_buf)
                } else {
                    1
                };

                let mut rx_size_buf = [0u8; 4];
                let rx_max_transfer_size = if handle
                    .read_control(
                        0xC1,
                        0x01,
                        0x0000,
                        VENDOR_INTERFACE as u16,
                        &mut rx_size_buf,
                        timeout,
                    )
                    .is_ok()
                {
                    u32::from_le_bytes(rx_size_buf).max(512)
                } else {
                    1024
                };

                let mut tx_size_buf = [0u8; 4];
                let tx_max_transfer_size = if handle
                    .read_control(
                        0xC1,
                        0x01,
                        0x0001,
                        VENDOR_INTERFACE as u16,
                        &mut tx_size_buf,
                        timeout,
                    )
                    .is_ok()
                {
                    u32::from_le_bytes(tx_size_buf).max(512)
                } else {
                    1024
                };

                handle.set_alternate_setting(VENDOR_INTERFACE, VENDOR_ALT_SETTING)?;

                let info = DeviceInfo {
                    bus_number: device.bus_number(),
                    address: device.address(),
                    vendor_id: desc.vendor_id(),
                    product_id: desc.product_id(),
                    firmware_bcd,
                    manufacturer,
                    product,
                    serial_number,
                    protocol_version,
                    rx_max_transfer_size,
                    tx_max_transfer_size,
                };

                let rx_alloc = (rx_max_transfer_size as usize).max(4096);
                return Ok(Self {
                    handle,
                    info,
                    seq: 1,
                    timeout,
                    rx_buf: vec![0u8; rx_alloc],
                });
            }
        }

        if found_dfu {
            return Err(DriverError::DeviceInDfuMode);
        }
        Err(DriverError::DeviceNotFound)
    }

    pub fn device_info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn tx_max_transfer_size(&self) -> usize {
        self.info.tx_max_transfer_size as usize
    }

    pub fn rx_max_transfer_size(&self) -> usize {
        self.info.rx_max_transfer_size as usize
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Sends a Bulk OUT command packet (`12-byte header + payload`) and reads the
    /// corresponding Bulk IN response packet, returning `(response_header, response_payload)`.
    pub fn transact(&mut self, cmd_id: CommandId, payload: &[u8]) -> Result<(PacketHeader, &[u8])> {
        let seq = self.seq;
        self.seq = if self.seq == 0xFF { 1 } else { self.seq + 1 };

        let header = PacketHeader::new(cmd_id, seq, payload.len());
        let mut tx_buf = Vec::with_capacity(HEADER_SIZE + payload.len());
        tx_buf.extend_from_slice(&header.to_bytes());
        tx_buf.extend_from_slice(payload);

        self.handle
            .write_bulk(EP_BULK_OUT, &tx_buf, self.timeout)?;

        // Per hsm_api_core_x64.dll: if packet size is an exact multiple of wMaxPacketSize (512)
        // and smaller than tx_max_transfer_size, send a Zero-Length Packet (ZLP).
        let max_tx = self.tx_max_transfer_size();
        if !tx_buf.is_empty()
            && tx_buf.len() % BULK_MAX_PACKET_SIZE == 0
            && tx_buf.len() < max_tx
        {
            let _ = self.handle.write_bulk(EP_BULK_OUT, &[], self.timeout);
        }

        let n = self
            .handle
            .read_bulk(EP_BULK_IN, &mut self.rx_buf, self.timeout)?;
        let resp_hdr = PacketHeader::from_bytes(&self.rx_buf[..n])?;

        // Device responses set bit 15 (0x8000) on cmd_id (e.g. 0x0101 -> 0x8101).
        let expected_cmd = cmd_id as u16;
        if resp_hdr.cmd_id != expected_cmd && resp_hdr.cmd_id != (expected_cmd | 0x8000) {
            return Err(DriverError::CommandMismatch {
                expected: expected_cmd | 0x8000,
                actual: resp_hdr.cmd_id,
            });
        }

        if resp_hdr.status != 0 {
            return Err(DriverError::DeviceStatusError {
                cmd_id: resp_hdr.cmd_id,
                status: resp_hdr.status,
            });
        }

        let payload_end = (resp_hdr.total_size as usize).min(n);
        let resp_payload = if payload_end >= HEADER_SIZE {
            &self.rx_buf[HEADER_SIZE..payload_end]
        } else {
            &[]
        };

        Ok((resp_hdr, resp_payload))
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        let _ = self.handle.set_alternate_setting(VENDOR_INTERFACE, 0);
        let _ = self.handle.release_interface(VENDOR_INTERFACE);
    }
}
