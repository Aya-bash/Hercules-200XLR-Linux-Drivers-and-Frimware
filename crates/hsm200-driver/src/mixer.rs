/// Microphone physical input source on the Hercules Stream 200 XLR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MicSource {
    /// Rear XLR / TRS combo jack (`0`).
    Xlr = 0,
    /// Front 3.5mm headset microphone jack (`1`).
    Headset = 1,
}

/// Target mix bus for Command `0x0401` (`packet[+0x10]` / `payload[0x04..0x08]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MixBus {
    /// Creator / Headphone Mix (`1`).
    Creator = 1,
    /// Audience / Stream Mix (`2`).
    Audience = 2,
    /// Hardware Input / Microphone (`4`).
    HardwareInput = 4,
    /// Master Output (`8`).
    MasterOutput = 8,
}

/// Complete Audio Mixer & Hardware Routing state for Command `0x0401` (28-byte payload, 40 bytes total).
#[derive(Debug, Clone, PartialEq)]
pub struct MixerState {
    /// Creator Mix 8 channel volumes in dB (`-120.0` to `+10.0` dB).
    pub creator_db: [f32; 8],
    /// Creator Mix 8 channel mute states.
    pub creator_mute: [bool; 8],
    /// Audience Mix 8 channel volumes in dB (`-120.0` to `+10.0` dB).
    pub audience_db: [f32; 8],
    /// Audience Mix 8 channel mute states.
    pub audience_mute: [bool; 8],
    /// Hardware input pre-gain / monitor levels in dB (`[Mic, LineIn, Aux, Optical]`).
    pub hw_input_db: [f32; 4],
    /// Hardware input mute states (`[Mic, LineIn, Aux, Optical]`).
    pub hw_input_mute: [bool; 4],
    /// Master output volume levels in dB (`[Headphones, LineOut]`).
    pub master_output_db: [f32; 2],
    /// Master output mute states (`[Headphones, LineOut]`).
    pub master_output_mute: [bool; 2],
    /// +48V Phantom Power for the XLR microphone input.
    pub phantom_power_48v: bool,
    /// Active microphone connector (`Xlr` vs `Headset`).
    pub mic_source: MicSource,
}

impl Default for MixerState {
    fn default() -> Self {
        Self {
            creator_db: [0.0; 8],
            creator_mute: [false; 8],
            audience_db: [0.0; 8],
            audience_mute: [false; 8],
            hw_input_db: [0.0; 4],
            hw_input_mute: [false; 4],
            master_output_db: [0.0; 2],
            master_output_mute: [false; 2],
            phantom_power_48v: false,
            mic_source: MicSource::Xlr,
        }
    }
}

impl MixerState {
    /// Converts a percentage (`0.0..=100.0%`) into dB using the exact formula from
    /// `hsm_api_core_x64.dll` (`0x18000fb20`):
    /// `if pct < 0.15 { -120.0 } else { 12.125 * log2(pct / 100.0) }`.
    pub fn percent_to_db(percent: f32) -> f32 {
        if percent < 0.15 {
            -120.0
        } else {
            12.125 * (percent / 100.0).log2()
        }
    }

    /// Legacy helper converting dB (`-110.0..=+10.0`) to `0..=12000`.
    pub fn db_to_raw(db: f32) -> u16 {
        let clamped = db.clamp(-110.0, 10.0);
        ((clamped + 110.0) * 100.0).round() as u16
    }

    /// Legacy helper converting `0..=12000` back to dB (`-110.0..=+10.0`).
    pub fn raw_to_db(raw: u16) -> f32 {
        (raw as f32 / 100.0) - 110.0
    }

    /// Encodes a 28-byte `0x0401` payload updating a single channel's fader volume (`flags |= 0x02`)
    /// and/or mute state (`flags |= 0x04`), matching `0x18000fb20` & `0x18000fbd0` in `hsm_api_core_x64.dll`.
    pub fn encode_channel_update(
        bus: MixBus,
        channel_idx: u8,
        gain_db: Option<f32>,
        muted: Option<bool>,
    ) -> [u8; 28] {
        let mut buf = [0u8; 28];
        let mut flags = 0u32;
        let ch_bit = 1u32 << (channel_idx & 31);

        buf[0x04..0x08].copy_from_slice(&(bus as u32).to_le_bytes()); // packet +0x10: mix_bus

        if let Some(db) = gain_db {
            flags |= 0x02;
            buf[0x0C..0x10].copy_from_slice(&ch_bit.to_le_bytes()); // packet +0x18: gain channel mask
            buf[0x10..0x14].copy_from_slice(&db.clamp(-120.0, 10.0).to_le_bytes()); // packet +0x1C: gain_db (f32)
        }

        if let Some(is_muted) = muted {
            flags |= 0x04;
            buf[0x14..0x18].copy_from_slice(&ch_bit.to_le_bytes()); // packet +0x20: mute mask
            let mute_bits = if is_muted { ch_bit } else { 0 };
            buf[0x18..0x1C].copy_from_slice(&mute_bits.to_le_bytes()); // packet +0x24: mute state
        }

        buf[0x00..0x04].copy_from_slice(&flags.to_le_bytes()); // packet +0x0C: flags
        buf
    }

    /// Encodes a 28-byte `0x0401` payload setting +48V Phantom Power (`flags = 0x08`, `bus = 4`),
    /// matching `0x1800123fa` in `hsm_api_core_x64.dll`.
    pub fn encode_phantom_power(enabled: bool) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0x00..0x04].copy_from_slice(&0x08u32.to_le_bytes()); // +0x0C: flags = 0x08 (48V)
        buf[0x04..0x08].copy_from_slice(&(MixBus::HardwareInput as u32).to_le_bytes()); // +0x10: bus = 4
        buf[0x14..0x18].copy_from_slice(&1u32.to_le_bytes()); // +0x20: mask = 1
        buf[0x18..0x1C].copy_from_slice(&(u32::from(enabled)).to_le_bytes()); // +0x24: 1 = On, 0 = Off
        buf
    }

    /// Encodes a 28-byte `0x0401` payload selecting the Microphone Input Source (`flags = 0x10`, `bus = 4`),
    /// matching `0x180010328` in `hsm_api_core_x64.dll`.
    pub fn encode_mic_source(source: MicSource) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0x00..0x04].copy_from_slice(&0x10u32.to_le_bytes()); // +0x0C: flags = 0x10 (Mic Source)
        buf[0x04..0x08].copy_from_slice(&(MixBus::HardwareInput as u32).to_le_bytes()); // +0x10: bus = 4
        buf[0x14..0x18].copy_from_slice(&(source as u32).to_le_bytes()); // +0x20: 0 = XLR, 1 = Headset
        buf[0x18..0x1C].copy_from_slice(&1u32.to_le_bytes()); // +0x24: 1
        buf
    }

    /// Encodes a 28-byte `0x0401` payload for Channel 0 of Creator Mix.
    pub fn to_payload(&self) -> [u8; 28] {
        Self::encode_channel_update(
            MixBus::Creator,
            0,
            Some(self.creator_db[0]),
            Some(self.creator_mute[0]),
        )
    }
}
