pub mod display;
pub mod dsp;
pub mod error;
pub mod leds;
pub mod mixer;
pub mod protocol;
pub mod state;
pub mod transport;

pub use display::{
    ScreenSubCommand, make_solid_paletted_background, make_solid_rgb565_frame,
    make_test_pattern_rgb565_frame, rgb888_to_rgb565,
};
pub use dsp::{
    CompressorConfig, DspPreset, DspSubBlock, EqBand, EqFilterType, LimiterConfig, MicDspConfig,
    NoiseGateConfig,
};
pub use error::{DriverError, Result};
pub use leds::{BacklightState, ChannelLedColors, LedMode, RgbColor, RgbMeterState};
pub use mixer::{MicSource, MixBus, MixerState};
pub use protocol::{CommandId, LCD_HEIGHT, LCD_WIDTH, PacketHeader};
pub use state::{Button, HardwareEvent, InputReport};
pub use transport::{DeviceInfo, UsbTransport};

/// High-level Linux driver handle for the Hercules Stream 200 XLR (`06f8:e054`).
pub struct Stream200Xlr {
    transport: UsbTransport,
    pub mixer: MixerState,
    pub dsp: MicDspConfig,
    pub rgb_meters: RgbMeterState,
    pub backlights: BacklightState,
    last_report: Option<InputReport>,
}

impl Stream200Xlr {
    /// Opens the connected Hercules Stream 200 XLR (`06f8:e054`) on the USB bus.
    pub fn open() -> Result<Self> {
        let transport = UsbTransport::open()?;
        Ok(Self {
            transport,
            mixer: MixerState::default(),
            dsp: MicDspConfig::default(),
            rgb_meters: RgbMeterState::default(),
            backlights: BacklightState::default(),
            last_report: None,
        })
    }

    /// Returns static USB device information (bus, address, serial number, firmware version, buffer sizes).
    pub fn device_info(&self) -> &DeviceInfo {
        self.transport.device_info()
    }

    /// Mutable access to the underlying `UsbTransport`.
    pub fn transport_mut(&mut self) -> &mut UsbTransport {
        &mut self.transport
    }

    /// Polls the hardware via Command `0x0101` (`PollStatus`) and returns the current `InputReport`.
    pub fn poll_status(&mut self) -> Result<InputReport> {
        let (_hdr, payload) = self.transport.transact(CommandId::PollStatus, &[])?;
        InputReport::from_payload(payload)
    }

    /// Polls the hardware via Command `0x0101` and returns both the new `InputReport` and
    /// any `HardwareEvent`s (button presses/releases, rotary encoder turns) since the previous call.
    pub fn poll_events(&mut self) -> Result<(InputReport, Vec<HardwareEvent>)> {
        let report = self.poll_status()?;
        let events = match &self.last_report {
            Some(prev) => report.diff(prev),
            None => Vec::new(),
        };
        self.last_report = Some(report.clone());
        Ok((report, events))
    }

    /// Queries the device status payload via Command `0x0A01` (`DeviceStateQuery`).
    pub fn query_device_state(&mut self) -> Result<Vec<u8>> {
        let (_hdr, payload) = self.transport.transact(CommandId::DeviceStateQuery, &[])?;
        Ok(payload.to_vec())
    }

    /// Sends all channel volumes, mutes, +48V Phantom Power, and Mic Source of `self.mixer` via `0x0401`.
    pub fn sync_mixer(&mut self) -> Result<()> {
        for ch in 0..4u8 {
            let creator_pkt = MixerState::encode_channel_update(
                MixBus::Creator,
                ch,
                Some(self.mixer.creator_db[ch as usize]),
                Some(self.mixer.creator_mute[ch as usize]),
            );
            self.transport.transact(CommandId::AudioMixer, &creator_pkt)?;

            let audience_pkt = MixerState::encode_channel_update(
                MixBus::Audience,
                ch,
                Some(self.mixer.audience_db[ch as usize]),
                Some(self.mixer.audience_mute[ch as usize]),
            );
            self.transport.transact(CommandId::AudioMixer, &audience_pkt)?;
        }
        Ok(())
    }

    /// Enables or disables +48V Phantom Power and immediately sends Command `0x0401` (`flags = 0x08`).
    pub fn set_phantom_power(&mut self, enabled: bool) -> Result<()> {
        self.mixer.phantom_power_48v = enabled;
        let payload = MixerState::encode_phantom_power(enabled);
        self.transport.transact(CommandId::AudioMixer, &payload)?;
        Ok(())
    }

    /// Selects the physical microphone input source (`Xlr` or `Headset`) and sends Command `0x0401` (`flags = 0x10`).
    pub fn set_mic_source(&mut self, source: MicSource) -> Result<()> {
        self.mixer.mic_source = source;
        let payload = MixerState::encode_mic_source(source);
        self.transport.transact(CommandId::AudioMixer, &payload)?;
        Ok(())
    }

    /// Updates a specific channel's fader volume in dB (`-120.0..=+10.0`) and/or mute state on `bus`.
    pub fn set_channel_fader(
        &mut self,
        bus: MixBus,
        channel_idx: u8,
        gain_db: Option<f32>,
        muted: Option<bool>,
    ) -> Result<()> {
        let idx = (channel_idx as usize) & 7;
        match bus {
            MixBus::Creator => {
                if let Some(db) = gain_db {
                    self.mixer.creator_db[idx] = db;
                }
                if let Some(m) = muted {
                    self.mixer.creator_mute[idx] = m;
                }
            }
            MixBus::Audience => {
                if let Some(db) = gain_db {
                    self.mixer.audience_db[idx] = db;
                }
                if let Some(m) = muted {
                    self.mixer.audience_mute[idx] = m;
                }
            }
            _ => {}
        }
        let payload = MixerState::encode_channel_update(bus, channel_idx, gain_db, muted);
        self.transport.transact(CommandId::AudioMixer, &payload)?;
        Ok(())
    }

    /// Sends all 12 sub-blocks of `self.dsp` to the AK7735EQ DSP via Command `0x0301` (`MicDspConfig`).
    pub fn sync_dsp(&mut self) -> Result<()> {
        let blocks = [
            DspSubBlock::Preamp,
            DspSubBlock::NoiseGate,
            DspSubBlock::HighPass,
            DspSubBlock::Equalizer,
            DspSubBlock::Compressor,
            DspSubBlock::DeEsser,
            DspSubBlock::Limiter,
            DspSubBlock::VoiceFx,
            DspSubBlock::Reverb,
            DspSubBlock::Echo,
            DspSubBlock::PitchShift,
            DspSubBlock::OutputTrim,
        ];
        for block in blocks {
            let payload = self.dsp.encode_subblock(block);
            self.transport.transact(CommandId::MicDspConfig, &payload)?;
        }
        Ok(())
    }

    /// Applies one of the 8 official DSP presets and syncs all DSP blocks (`0x0301`).
    pub fn apply_dsp_preset(&mut self, preset: DspPreset) -> Result<()> {
        self.dsp = MicDspConfig::from_preset(preset);
        self.sync_dsp()
    }

    /// Sends the current `self.rgb_meters` palettes and VU levels via Command `0x0501` (`RgbMeters`).
    pub fn sync_rgb_meters(&mut self) -> Result<()> {
        let style_pkt = self.rgb_meters.encode_meter_styles();
        self.transport.transact(CommandId::RgbMeters, &style_pkt)?;

        for ch in 0..4u8 {
            for side in 1..=2u8 {
                let pal_pkt = self.rgb_meters.encode_channel_palette(ch, side);
                self.transport.transact(CommandId::RgbMeters, &pal_pkt)?;

                let slot = (ch as usize) * 2 + (side as usize - 1);
                let level_10000 = ((self.rgb_meters.vu_levels[slot] as u32) * 10000) / 255;
                let vu_pkt = RgbMeterState::encode_vu_level(ch, side, level_10000);
                self.transport.transact(CommandId::RgbMeters, &vu_pkt)?;
            }
        }
        Ok(())
    }

    /// Sends both button LEDs (`sub_cmd = 1`) and LCD brightness (`sub_cmd = 2`) via Command `0x0601`.
    pub fn sync_backlights(&mut self) -> Result<()> {
        let led_pkt = self.backlights.encode_button_leds(0x0000_3FFF);
        self.transport
            .transact(CommandId::ButtonLedsAndBacklight, &led_pkt)?;

        let lcd_pkt = self.backlights.encode_lcd_brightness();
        self.transport
            .transact(CommandId::ButtonLedsAndBacklight, &lcd_pkt)?;
        Ok(())
    }

    /// Sets LCD backlight brightness (`0..=100%`) and sends Command `0x0601` (`sub_cmd = 2`).
    pub fn set_lcd_brightness(&mut self, percent: u8) -> Result<()> {
        self.backlights.lcd_brightness = percent.min(100);
        let lcd_pkt = self.backlights.encode_lcd_brightness();
        self.transport
            .transact(CommandId::ButtonLedsAndBacklight, &lcd_pkt)?;
        Ok(())
    }

    /// Wakes or puts the LCD screen to sleep (`0x0801` sub-command `0x0001` or `0x0003`).
    pub fn set_screen_power(&mut self, awake: bool) -> Result<()> {
        let cmd = if awake {
            ScreenSubCommand::Wake
        } else {
            ScreenSubCommand::Sleep
        };
        display::send_screen_command(&mut self.transport, cmd, 0)
    }

    /// Switches the active LCD mixer page (`1` or `2`).
    pub fn set_screen_page(&mut self, page: u8) -> Result<()> {
        display::set_screen_page(&mut self.transport, page)
    }

    /// Fills the LCD background wallpaper with a solid `RgbColor` (`0x0801` `sub_cmd = 0x09` + commit `0x23`).
    /// `slot`: `6` for active live mixer screen, `0` for standby screensaver screen.
    pub fn fill_screen_background_slot(&mut self, color: RgbColor, slot: u32) -> Result<()> {
        let paletted = display::make_solid_paletted_background(color);
        display::upload_paletted_background(&mut self.transport, &paletted, slot)
    }

    /// Fills the active live LCD background wallpaper (`slot = 6`) with a solid `RgbColor`.
    pub fn fill_screen_background(&mut self, color: RgbColor) -> Result<()> {
        self.fill_screen_background_slot(color, display::LIVE_BACKGROUND_SLOT_DEFAULT)
    }

    /// Uploads a full-screen `480x272` RGB565 image (`261,120` bytes) to the LCD background (`0x0801`).
    /// `slot`: `6` for active live mixer screen, `0` for standby screensaver screen.
    pub fn upload_rgb565_frame_slot(&mut self, rgb565_le: &[u8], slot: u32) -> Result<()> {
        display::upload_rgb565_framebuffer(&mut self.transport, rgb565_le, slot)
    }

    /// Uploads a full-screen `480x272` RGB565 image (`261,120` bytes) to the active live LCD background (`slot = 6`).
    pub fn upload_rgb565_frame(&mut self, rgb565_le: &[u8]) -> Result<()> {
        self.upload_rgb565_frame_slot(rgb565_le, display::LIVE_BACKGROUND_SLOT_DEFAULT)
    }

    /// Uploads a full-screen `480x272` 16-bit RGB565 splash framebuffer directly (`0x0801` `sub_cmd = 0x05`).
    pub fn upload_rgb565_splash(&mut self, rgb565_le: &[u8]) -> Result<()> {
        display::upload_rgb565_splash(&mut self.transport, rgb565_le)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_header_roundtrip() {
        let hdr = PacketHeader::new(CommandId::PollStatus, 0x42, 0);
        let bytes = hdr.to_bytes();
        assert_eq!(
            bytes,
            [0x0C, 0x00, 0x00, 0x00, 0x01, 0x01, 0x42, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
        let decoded = PacketHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, hdr);
    }

    #[test]
    fn test_lower_4_action_button_bitmasks() {
        let mut raw = [0u8; 256];
        // Press Action1 (0001 = 0x01) and Action3 (0100 = 0x04) in byte +0x02
        raw[2] = 0b0101;
        // Press Encoder2Push (0010 = 0x02) in byte +0x01
        raw[1] = 0b0010;
        let report = InputReport::from_payload(&raw).unwrap();
        assert!(report.is_pressed(Button::Action1));
        assert!(!report.is_pressed(Button::Action2));
        assert!(report.is_pressed(Button::Action3));
        assert!(!report.is_pressed(Button::Action4));
        assert!(report.is_pressed(Button::Encoder2Push));
        assert_eq!(report.action_buttons_nibble(), 0b0101);
        assert_eq!(report.encoder_push_nibble(), 0b0010);
    }

    #[test]
    fn test_mixer_db_scaling() {
        assert_eq!(MixerState::db_to_raw(-110.0), 0);
        assert_eq!(MixerState::db_to_raw(0.0), 11000);
        assert_eq!(MixerState::db_to_raw(10.0), 12000);
        assert!((MixerState::raw_to_db(11000) - 0.0).abs() < 1e-4);
    }
}
