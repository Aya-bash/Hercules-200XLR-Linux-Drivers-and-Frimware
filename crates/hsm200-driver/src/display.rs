use crate::error::{DriverError, Result};
use crate::leds::RgbColor;
use crate::protocol::{CommandId, HEADER_SIZE, LCD_HEIGHT, LCD_WIDTH};
use crate::transport::UsbTransport;

/// Size of the `0x0801` / `0x0701` extended packet header (`40` bytes = `0x28` total, `28` bytes payload header).
pub const SCREEN_HEADER_PAYLOAD_SIZE: usize = 28;

/// Total size of a `480x272` 8-bit paletted background buffer (`1024`-byte BGRA palette + `130,560` indices = `0x20200`).
pub const PALETTED_BG_TOTAL_BYTES: usize = 0x20200;

/// Total size of a `480x272` 16-bit RGB565 splash framebuffer (`480 * 272 * 2 = 0x3FC00` = `261,120` bytes).
pub const RGB565_FRAME_TOTAL_BYTES: usize = 0x3FC00;

/// Sub-command IDs for Command `0x0801` (`ScreenControl` — `payload[0x00..0x04]` / packet `+0x0C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ScreenSubCommand {
    /// Wake / turn on LCD screen (`0x0001`, `0x180012305`).
    Wake = 0x0001,
    /// Sleep / turn off LCD screen (`0x0003`, `0x180012305`).
    Sleep = 0x0003,
    /// Full-screen `480x272` RGB565 (`0x3FC00` = `261,120` bytes) chunked splash upload (`0x0005`, `0x180011610`).
    Rgb565SplashChunk = 0x0005,
    /// Full-screen `480x272` 8-bit paletted (`0x20200` = `131,584` bytes) background chunk (`0x0009`, `0x180010900`).
    PalettedBackgroundChunk = 0x0009,
    /// Switch active mixer page (`0x0010`, `0x180011960`, with `page_index` `0` or `1` at packet `+0x24`).
    SelectPage = 0x0010,
    /// Commit uploaded `0x20200` paletted background (`0x0023`, `0x180010b33`).
    CommitBackground = 0x0023,
    /// Refresh Page 1 UI layer (`0x0080`, `0x1800117f0`).
    RefreshPage1 = 0x0080,
    /// Refresh Page 2 UI layer (`0x0100`, `0x1800117f0`).
    RefreshPage2 = 0x0100,
    /// Switch display layout (`0x0200`, `0x180011960`, with `(layout - 1) << 8` at packet `+0x24`).
    SelectLayout = 0x0200,
}

/// Converts an 8-bit `(R, G, B)` color into a 16-bit little-endian RGB565 pixel (`u16`).
#[inline]
pub const fn rgb888_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
    let r5 = ((r as u16) >> 3) & 0x1F;
    let g6 = ((g as u16) >> 2) & 0x3F;
    let b5 = ((b as u16) >> 3) & 0x1F;
    (r5 << 11) | (g6 << 5) | b5
}

/// Sends a 40-byte (`0x28` total = 12-byte USB header + 28-byte payload) `0x0801` screen control packet.
pub fn send_screen_command(
    transport: &mut UsbTransport,
    sub_cmd: ScreenSubCommand,
    param_24: u32,
) -> Result<()> {
    let mut payload = [0u8; SCREEN_HEADER_PAYLOAD_SIZE];
    payload[0x00..0x04].copy_from_slice(&(sub_cmd as u32).to_le_bytes()); // packet +0x0C
    payload[0x18..0x1C].copy_from_slice(&param_24.to_le_bytes()); // packet +0x24
    transport.transact(CommandId::ScreenControl, &payload)?;
    Ok(())
}

/// Switches the active LCD mixer page (`1` or `2`) and refreshes the page UI (`0x180011960` + `0x1800117f0`).
pub fn set_screen_page(transport: &mut UsbTransport, page: u8) -> Result<()> {
    if !(1..=2).contains(&page) {
        return Err(DriverError::InvalidParameter(
            "Screen page must be 1 or 2".to_string(),
        ));
    }
    let page_idx = (page - 1) as u32;
    send_screen_command(transport, ScreenSubCommand::SelectPage, page_idx)?;
    let refresh_cmd = if page == 1 {
        ScreenSubCommand::RefreshPage1
    } else {
        ScreenSubCommand::RefreshPage2
    };
    send_screen_command(transport, refresh_cmd, 0)
}

/// Switches the LCD layout (`1..=4`, `0x180011960`).
pub fn set_screen_layout(transport: &mut UsbTransport, layout: u8) -> Result<()> {
    if layout == 0 {
        return Err(DriverError::InvalidParameter(
            "Screen layout must be >= 1".to_string(),
        ));
    }
    let param_24 = ((layout as u32) - 1) << 8;
    send_screen_command(transport, ScreenSubCommand::SelectLayout, param_24)
}

/// Uploads a `0x20200` (`131,584`-byte) 8-bit paletted background wallpaper to the LCD
/// using `0x0801` `sub_cmd = 0x09` followed by `sub_cmd = 0x23` (`0x180010900` in `hsm_api_core_x64.dll`).
///
/// Format of `paletted_buf` (`0x20200` bytes):
/// - `0x0000..0x0400` (`1024` bytes): 256-entry `BGRA` palette (`[B, G, R, 0xFF]`).
/// - `0x0400..0x20200` (`130,560` bytes): `480 * 272` 8-bit palette indices.
pub fn upload_paletted_background(
    transport: &mut UsbTransport,
    paletted_buf: &[u8],
    commit_slot: u32,
) -> Result<()> {
    if paletted_buf.len() != PALETTED_BG_TOTAL_BYTES {
        return Err(DriverError::InvalidParameter(format!(
            "Paletted background must be {PALETTED_BG_TOTAL_BYTES} bytes (0x20200), got {}",
            paletted_buf.len()
        )));
    }

    let max_chunk = transport
        .tx_max_transfer_size()
        .saturating_sub(HEADER_SIZE + SCREEN_HEADER_PAYLOAD_SIZE)
        .max(472);

    let total_len = PALETTED_BG_TOTAL_BYTES;
    let mut offset = 0usize;

    while offset < total_len {
        let chunk_len = (total_len - offset).min(max_chunk);
        let remaining_after = total_len - offset - chunk_len;

        let mut payload = Vec::with_capacity(SCREEN_HEADER_PAYLOAD_SIZE + chunk_len);
        payload.resize(SCREEN_HEADER_PAYLOAD_SIZE, 0);
        // +0x0C: sub_cmd = 0x09
        payload[0x00..0x04]
            .copy_from_slice(&(ScreenSubCommand::PalettedBackgroundChunk as u32).to_le_bytes());
        // +0x14: offset
        payload[0x08..0x0C].copy_from_slice(&(offset as u32).to_le_bytes());
        // +0x18: remaining bytes after this chunk
        payload[0x0C..0x10].copy_from_slice(&(remaining_after as u32).to_le_bytes());
        // +0x1C: chunk length
        payload[0x10..0x14].copy_from_slice(&(chunk_len as u32).to_le_bytes());
        // +0x28..: data bytes
        payload.extend_from_slice(&paletted_buf[offset..offset + chunk_len]);

        transport.transact(CommandId::ScreenControl, &payload)?;
        offset += chunk_len;
    }

    // Send final 40-byte commit packet (sub_cmd = 0x23, matching 0x180010b33)
    let mut commit_payload = [0u8; SCREEN_HEADER_PAYLOAD_SIZE];
    commit_payload[0x00..0x04]
        .copy_from_slice(&(ScreenSubCommand::CommitBackground as u32).to_le_bytes());
    commit_payload[0x08..0x0C].copy_from_slice(&(total_len as u32).to_le_bytes()); // +0x14 = 0x20200
    commit_payload[0x14..0x18].copy_from_slice(&commit_slot.to_le_bytes()); // +0x20 = slot_id (0 = live)
    transport.transact(CommandId::ScreenControl, &commit_payload)?;

    Ok(())
}

/// Builds a `0x20200` (`131,584`-byte) paletted background buffer filled with a solid `RgbColor`
/// (matching `0x180010cc7..0x180010cfd` in `hsm_api_core_x64.dll`: palette[0] = `[B, G, R, 0xFF]`, all indices = `0`).
pub fn make_solid_paletted_background(color: RgbColor) -> Vec<u8> {
    let mut buf = vec![0u8; PALETTED_BG_TOTAL_BYTES];
    buf[0] = color.b;
    buf[1] = color.g;
    buf[2] = color.r;
    buf[3] = 0xFF;
    buf
}

/// Converts a `480x272` RGB565 little-endian buffer (`261,120` bytes) into a `0x20200` (`131,584`-byte)
/// 8-bit 256-color paletted background buffer (`RGB332` palette in `BGRA` format + `480x272` indices).
pub fn rgb565_to_paletted_background(rgb565_le: &[u8]) -> Result<Vec<u8>> {
    if rgb565_le.len() != RGB565_FRAME_TOTAL_BYTES {
        return Err(DriverError::InvalidParameter(format!(
            "RGB565 buffer must be {RGB565_FRAME_TOTAL_BYTES} bytes, got {}",
            rgb565_le.len()
        )));
    }

    let mut buf = vec![0u8; PALETTED_BG_TOTAL_BYTES];

    // Populate 256-entry RGB332 palette in BGRA order at 0x0000..0x0400
    for idx in 0..256usize {
        let r3 = ((idx >> 5) & 0x07) as u8;
        let g3 = ((idx >> 2) & 0x07) as u8;
        let b2 = (idx & 0x03) as u8;
        let r8 = (r3 << 5) | (r3 << 2) | (r3 >> 1);
        let g8 = (g3 << 5) | (g3 << 2) | (g3 >> 1);
        let b8 = (b2 << 6) | (b2 << 4) | (b2 << 2) | b2;
        buf[idx * 4] = b8;
        buf[idx * 4 + 1] = g8;
        buf[idx * 4 + 2] = r8;
        buf[idx * 4 + 3] = 0xFF;
    }

    // Populate 480 * 272 8-bit indices at 0x0400..0x20200
    let total_pixels = (LCD_WIDTH as usize) * (LCD_HEIGHT as usize);
    for px in 0..total_pixels {
        let raw = u16::from_le_bytes([rgb565_le[px * 2], rgb565_le[px * 2 + 1]]);
        let r3 = ((raw >> 13) & 0x07) as u8;
        let g3 = ((raw >> 8) & 0x07) as u8;
        let b2 = ((raw >> 3) & 0x03) as u8;
        buf[0x0400 + px] = (r3 << 5) | (g3 << 2) | b2;
    }

    Ok(buf)
}

/// Uploads a full-screen `480x272` RGB565 framebuffer (`261,120` bytes) to the live LCD background
/// by converting it to the `0x20200` paletted background format (`sub_cmd = 0x09` + commit `0x23`).
pub fn upload_rgb565_framebuffer(transport: &mut UsbTransport, rgb565_le: &[u8]) -> Result<()> {
    let paletted = rgb565_to_paletted_background(rgb565_le)?;
    upload_paletted_background(transport, &paletted, 0)
}

/// Uploads a raw `480x272` RGB565 splash framebuffer (`0x3FC00` = `261,120` bytes) using `sub_cmd = 0x05`
/// (`0x180011610` in `hsm_api_core_x64.dll`).
pub fn upload_rgb565_splash(transport: &mut UsbTransport, rgb565_le: &[u8]) -> Result<()> {
    if rgb565_le.len() != RGB565_FRAME_TOTAL_BYTES {
        return Err(DriverError::InvalidParameter(format!(
            "RGB565 framebuffer must be exactly {RGB565_FRAME_TOTAL_BYTES} bytes (480x272x2), got {}",
            rgb565_le.len()
        )));
    }

    let max_chunk = transport
        .tx_max_transfer_size()
        .saturating_sub(HEADER_SIZE + SCREEN_HEADER_PAYLOAD_SIZE)
        .max(472);

    let total_len = RGB565_FRAME_TOTAL_BYTES;
    let mut offset = 0usize;

    while offset < total_len {
        let chunk_len = (total_len - offset).min(max_chunk);
        let remaining_after = total_len - offset - chunk_len;

        let mut payload = Vec::with_capacity(SCREEN_HEADER_PAYLOAD_SIZE + chunk_len);
        payload.resize(SCREEN_HEADER_PAYLOAD_SIZE, 0);
        // +0x0C: sub_cmd = 0x05
        payload[0x00..0x04]
            .copy_from_slice(&(ScreenSubCommand::Rgb565SplashChunk as u32).to_le_bytes());
        // +0x14: offset
        payload[0x08..0x0C].copy_from_slice(&(offset as u32).to_le_bytes());
        // +0x18: remaining bytes after this chunk
        payload[0x0C..0x10].copy_from_slice(&(remaining_after as u32).to_le_bytes());
        // +0x1C: chunk length
        payload[0x10..0x14].copy_from_slice(&(chunk_len as u32).to_le_bytes());
        // +0x28..: data bytes
        payload.extend_from_slice(&rgb565_le[offset..offset + chunk_len]);

        transport.transact(CommandId::ScreenControl, &payload)?;
        offset += chunk_len;
    }

    Ok(())
}

/// Generates a full-screen `480x272` RGB565 buffer filled with a solid `RgbColor`.
pub fn make_solid_rgb565_frame(color: RgbColor) -> Vec<u8> {
    let pixel = rgb888_to_rgb565(color.r, color.g, color.b).to_le_bytes();
    let total_pixels = (LCD_WIDTH as usize) * (LCD_HEIGHT as usize);
    let mut buf = Vec::with_capacity(total_pixels * 2);
    for _ in 0..total_pixels {
        buf.extend_from_slice(&pixel);
    }
    buf
}

/// Generates a `480x272` color-bars + gradient test pattern in RGB565 format.
pub fn make_test_pattern_rgb565_frame() -> Vec<u8> {
    let bars = [
        RgbColor::WHITE,
        RgbColor::YELLOW,
        RgbColor::CYAN,
        RgbColor::GREEN,
        RgbColor::MAGENTA,
        RgbColor::RED,
        RgbColor::BLUE,
        RgbColor::BLACK,
    ];
    let width = LCD_WIDTH as usize;
    let height = LCD_HEIGHT as usize;
    let mut buf = Vec::with_capacity(width * height * 2);

    for y in 0..height {
        for x in 0..width {
            let pixel = if y < (height * 3) / 4 {
                let bar_idx = (x * bars.len()) / width;
                let c = bars[bar_idx];
                rgb888_to_rgb565(c.r, c.g, c.b)
            } else {
                let v = ((x * 255) / width) as u8;
                rgb888_to_rgb565(v, v, v)
            };
            buf.extend_from_slice(&pixel.to_le_bytes());
        }
    }
    buf
}
