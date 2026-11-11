use crate::state::Button;

/// 24-bit RGB color (`R, G, B`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const RED: Self = Self::new(255, 0, 0);
    pub const GREEN: Self = Self::new(0, 255, 0);
    pub const BLUE: Self = Self::new(0, 0, 255);
    pub const YELLOW: Self = Self::new(255, 200, 0);
    pub const CYAN: Self = Self::new(0, 255, 255);
    pub const MAGENTA: Self = Self::new(255, 0, 255);
    pub const WHITE: Self = Self::new(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses a 6-digit hex string (`"FF8800"` or `"#FF8800"`).
    pub fn from_hex(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self { r, g, b })
    }

    pub const fn to_bytes(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }

    /// Packs `(R, G, B)` as a 24-bit integer `0x00RRGGBB` (matching `hsm_api_core_x64.dll` `0x180013626`).
    pub const fn to_u32_rgb24(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

/// RGB gradient & background colors for a single mixer channel strip (`0..3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelLedColors {
    pub top: RgbColor,
    pub middle: RgbColor,
    pub bottom: RgbColor,
    pub mute: RgbColor,
}

impl Default for ChannelLedColors {
    fn default() -> Self {
        Self {
            top: RgbColor::RED,
            middle: RgbColor::YELLOW,
            bottom: RgbColor::GREEN,
            mute: RgbColor::BLACK,
        }
    }
}

/// State for Command `0x0501` (`RgbMeters` — 60-byte payload, 72 bytes = `0x48` total with header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbMeterState {
    pub channels: [ChannelLedColors; 4],
    /// VU meter bar levels (`0..=255`, scaled to `0..=10000` on wire) for 8 slots (`Ch0..3` Left/Right).
    pub vu_levels: [u8; 8],
    /// Meter style per slot (`0..=3`, default `1`).
    pub meter_styles: [u8; 8],
}

impl Default for RgbMeterState {
    fn default() -> Self {
        Self {
            channels: [ChannelLedColors::default(); 4],
            vu_levels: [0; 8],
            meter_styles: [1; 8],
        }
    }
}

impl RgbMeterState {
    /// Encodes a `sub_cmd = 0x02` (Global Meter Style/Enable) 60-byte payload for Command `0x0501`
    /// (`0x180013537` in `hsm_api_core_x64.dll`).
    pub fn encode_meter_styles(&self) -> [u8; 60] {
        let mut buf = [0u8; 60];
        buf[0x00..0x04].copy_from_slice(&0x02u32.to_le_bytes()); // +0x0C: sub_cmd = 0x02
        buf[0x08..0x10].copy_from_slice(&self.meter_styles); // +0x14..+0x1B
        buf
    }

    /// Encodes a `sub_cmd = 0x0C` (Channel Meter Color Palette) 60-byte payload for Command `0x0501`
    /// (`0x180013713` in `hsm_api_core_x64.dll`).
    /// `channel_idx` is `0..=3`, `side` is `1` (Left / Creator) or `2` (Right / Audience).
    pub fn encode_channel_palette(&self, channel_idx: u8, side: u8) -> [u8; 60] {
        let ch = self.channels[(channel_idx as usize) & 3];
        let mut buf = [0u8; 60];
        buf[0x00..0x04].copy_from_slice(&0x0Cu32.to_le_bytes()); // +0x0C: sub_cmd = 0x0C
        buf[0x04..0x08].copy_from_slice(&(channel_idx as u32).to_le_bytes()); // +0x10: channel 0..3
        buf[0x18..0x1C].copy_from_slice(&1u32.to_le_bytes()); // +0x24: style mode
        let target_flags = ((side as u32).clamp(1, 2)) | 0x0100;
        buf[0x1C..0x20].copy_from_slice(&target_flags.to_le_bytes()); // +0x28: 0x101 or 0x102
        buf[0x20..0x24].copy_from_slice(&ch.mute.to_u32_rgb24().to_le_bytes()); // +0x2C: background RGB24
        buf[0x24..0x28].copy_from_slice(&ch.bottom.to_u32_rgb24().to_le_bytes()); // +0x30: low RGB24
        buf[0x28..0x2C].copy_from_slice(&ch.middle.to_u32_rgb24().to_le_bytes()); // +0x34: mid RGB24
        buf[0x2C..0x30].copy_from_slice(&ch.top.to_u32_rgb24().to_le_bytes()); // +0x38: high RGB24
        buf
    }

    /// Encodes a `sub_cmd = 0x40` (Live VU Meter Level Update) 60-byte payload for Command `0x0501`
    /// (`0x180013c60` in `hsm_api_core_x64.dll`).
    /// `channel_idx` is `0..=3`, `side` is `1` (Left) or `2` (Right), `level_10000` is `0..=10000`.
    pub fn encode_vu_level(channel_idx: u8, side: u8, level_10000: u32) -> [u8; 60] {
        let mut buf = [0u8; 60];
        buf[0x00..0x04].copy_from_slice(&0x40u32.to_le_bytes()); // +0x0C: sub_cmd = 0x40
        buf[0x04..0x08].copy_from_slice(&(channel_idx as u32).to_le_bytes()); // +0x10: channel 0..3
        buf[0x1C..0x20].copy_from_slice(&(side.clamp(1, 2) as u32).to_le_bytes()); // +0x28: side 1 or 2
        buf[0x38..0x3C].copy_from_slice(&level_10000.min(10000).to_le_bytes()); // +0x44: level 0..10000
        buf
    }

    /// Encodes a default 60-byte payload (`sub_cmd = 0x0C`, Channel 0 Left) for legacy callers.
    pub fn to_payload(&self) -> [u8; 60] {
        self.encode_channel_palette(0, 1)
    }
}

/// Illumination mode for a physical button LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedMode {
    Off,
    Solid,
    Blink,
}

/// State for Command `0x0601` (`ButtonLedsAndBacklight` — 20-byte payload, 32 bytes = `0x20` total).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklightState {
    /// Bitmask of buttons with LED enabled (`1 << button.led_index()`).
    pub solid_mask: u32,
    /// Bitmask of buttons with blinking enabled (`1 << button.led_index()`).
    pub blink_mask: u32,
    /// LCD screen backlight brightness (`0..=100`).
    pub lcd_brightness: u8,
    /// Button LED global brightness (`0..=100`, where `0` = dim, `100` = full).
    pub led_intensity: u8,
}

impl Default for BacklightState {
    fn default() -> Self {
        Self {
            solid_mask: 0x0000_3FFF, // All 14 button LEDs on by default
            blink_mask: 0,
            lcd_brightness: 80,
            led_intensity: 100,
        }
    }
}

impl BacklightState {
    /// Sets the `LedMode` for a specific `Button`.
    pub fn set_button(&mut self, button: Button, mode: LedMode) {
        let bit = 1u32 << button.led_index();
        match mode {
            LedMode::Off => {
                self.solid_mask &= !bit;
                self.blink_mask &= !bit;
            }
            LedMode::Solid => {
                self.solid_mask |= bit;
                self.blink_mask &= !bit;
            }
            LedMode::Blink => {
                self.solid_mask |= bit;
                self.blink_mask |= bit;
            }
        }
    }

    /// Sets the `LedMode` for all 14 buttons at once.
    pub fn set_all_buttons(&mut self, mode: LedMode) {
        for button in Button::ALL {
            self.set_button(button, mode);
        }
    }

    /// Encodes the 20-byte (`0x14`) payload for Command `0x0601` (`sub_cmd = 1`: Button LEDs,
    /// matching `0x18001201f` in `hsm_api_core_x64.dll`).
    pub fn encode_button_leds(&self, update_mask: u32) -> [u8; 20] {
        let mut buf = [0u8; 20];
        let steady_bits = self.solid_mask & !self.blink_mask;
        let blink_bits = self.solid_mask & self.blink_mask;
        buf[0x00..0x04].copy_from_slice(&1u32.to_le_bytes()); // +0x0C: sub_cmd = 1 (Button LEDs)
        buf[0x04..0x08].copy_from_slice(&update_mask.to_le_bytes()); // +0x10: mask of LEDs to update
        buf[0x08..0x0C].copy_from_slice(&steady_bits.to_le_bytes()); // +0x14: steady-on bitmask
        buf[0x0C..0x10].copy_from_slice(&blink_bits.to_le_bytes()); // +0x18: blink bitmask
        buf
    }

    /// Encodes the 20-byte (`0x14`) payload for Command `0x0601` (`sub_cmd = 2`: LCD Backlight Brightness,
    /// matching `0x18001224c` in `hsm_api_core_x64.dll`).
    pub fn encode_lcd_brightness(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0x00..0x04].copy_from_slice(&2u32.to_le_bytes()); // +0x0C: sub_cmd = 2 (LCD Brightness)
        let brightness = (self.lcd_brightness.min(100)) as u32;
        buf[0x10..0x14].copy_from_slice(&brightness.to_le_bytes()); // +0x1C: brightness 0..100
        buf
    }

    /// Encodes a combined 20-byte (`0x14`) payload with `sub_cmd = 3` (`1 | 2`).
    pub fn to_payload(&self) -> [u8; 20] {
        let mut buf = self.encode_button_leds(0x0000_3FFF);
        buf[0x00..0x04].copy_from_slice(&3u32.to_le_bytes()); // +0x0C: sub_cmd = 1 | 2
        let brightness = (self.lcd_brightness.min(100)) as u32;
        buf[0x10..0x14].copy_from_slice(&brightness.to_le_bytes()); // +0x1C: brightness 0..100
        buf
    }
}
