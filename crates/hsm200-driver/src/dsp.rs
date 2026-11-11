/// Sub-block index (`0..=11`) for Command `0x0301` (`MicDspConfig`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DspSubBlock {
    Preamp = 0,
    NoiseGate = 1,
    HighPass = 2,
    Equalizer = 3,
    Compressor = 4,
    DeEsser = 5,
    Limiter = 6,
    VoiceFx = 7,
    Reverb = 8,
    Echo = 9,
    PitchShift = 10,
    OutputTrim = 11,
}

/// Biquad EQ filter type for the 3-band equalizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EqFilterType {
    Bypass = 0,
    HighPass = 1,
    LowPass = 2,
    Peaking = 3,
    LowShelf = 4,
    HighShelf = 5,
}

/// Single band of the 3-band Microphone Equalizer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqBand {
    pub filter_type: EqFilterType,
    pub frequency_hz: u16,
    pub gain_db: f32,
    pub q_factor: f32,
}

impl Default for EqBand {
    fn default() -> Self {
        Self {
            filter_type: EqFilterType::Peaking,
            frequency_hz: 1000,
            gain_db: 0.0,
            q_factor: 1.0,
        }
    }
}

/// Noise Gate parameters for the AK7735EQ DSP.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseGateConfig {
    pub enabled: bool,
    pub threshold_db: f32,
    pub attenuation_percent: u8,
    pub attack_ms: u16,
    pub release_ms: u16,
}

impl Default for NoiseGateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_db: -66.0,
            attenuation_percent: 0,
            attack_ms: 1,
            release_ms: 100,
        }
    }
}

/// Compressor parameters for the AK7735EQ DSP.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressorConfig {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: u8,
    pub attack_ms: u16,
    pub release_ms: u16,
    pub makeup_gain_db: f32,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_db: -12.0,
            ratio: 4,
            attack_ms: 1,
            release_ms: 100,
            makeup_gain_db: 5.0,
        }
    }
}

/// Limiter parameters for the AK7735EQ DSP.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimiterConfig {
    pub enabled: bool,
    pub threshold_db: f32,
    pub attack_ms: u16,
    pub release_ms: u16,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_db: -6.0,
            attack_ms: 1,
            release_ms: 60,
        }
    }
}

/// Built-in Microphone DSP presets extracted from the official Hercules Stream Control XML profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspPreset {
    Default,
    FxDisabled,
    NaturalBright,
    NaturalDark,
    RadioBright,
    RadioDark,
    Hxd700Close,
    Hxd700Table,
}

/// Complete AK7735EQ Microphone DSP configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct MicDspConfig {
    pub preamp_gain_db: f32,
    pub noise_gate: NoiseGateConfig,
    pub eq_enabled: bool,
    pub eq_bands: [EqBand; 3],
    pub compressor: CompressorConfig,
    pub limiter: LimiterConfig,
}

impl Default for MicDspConfig {
    fn default() -> Self {
        Self::from_preset(DspPreset::Default)
    }
}

impl MicDspConfig {
    /// Constructs a `MicDspConfig` from one of the 8 official Hercules DSP presets.
    pub fn from_preset(preset: DspPreset) -> Self {
        match preset {
            DspPreset::Default => Self {
                preamp_gain_db: 10.0,
                noise_gate: NoiseGateConfig {
                    enabled: true,
                    threshold_db: -66.0,
                    attenuation_percent: 0,
                    attack_ms: 1,
                    release_ms: 100,
                },
                eq_enabled: true,
                eq_bands: [
                    EqBand {
                        filter_type: EqFilterType::HighPass,
                        frequency_hz: 30,
                        gain_db: -0.2,
                        q_factor: 0.8,
                    },
                    EqBand {
                        filter_type: EqFilterType::LowPass,
                        frequency_hz: 12988,
                        gain_db: -2.9,
                        q_factor: 0.8,
                    },
                    EqBand::default(),
                ],
                compressor: CompressorConfig {
                    enabled: true,
                    threshold_db: -12.0,
                    ratio: 4,
                    attack_ms: 1,
                    release_ms: 100,
                    makeup_gain_db: 5.0,
                },
                limiter: LimiterConfig {
                    enabled: true,
                    threshold_db: -6.0,
                    attack_ms: 1,
                    release_ms: 60,
                },
            },
            DspPreset::FxDisabled => Self {
                preamp_gain_db: 10.0,
                noise_gate: NoiseGateConfig {
                    enabled: false,
                    ..NoiseGateConfig::default()
                },
                eq_enabled: false,
                eq_bands: [EqBand::default(); 3],
                compressor: CompressorConfig {
                    enabled: false,
                    ..CompressorConfig::default()
                },
                limiter: LimiterConfig {
                    enabled: false,
                    ..LimiterConfig::default()
                },
            },
            DspPreset::NaturalBright => Self {
                preamp_gain_db: 10.0,
                noise_gate: NoiseGateConfig {
                    enabled: true,
                    threshold_db: -58.0,
                    attenuation_percent: 0,
                    attack_ms: 2,
                    release_ms: 100,
                },
                eq_enabled: true,
                eq_bands: [
                    EqBand {
                        filter_type: EqFilterType::LowShelf,
                        frequency_hz: 200,
                        gain_db: -1.5,
                        q_factor: 0.8,
                    },
                    EqBand {
                        filter_type: EqFilterType::HighShelf,
                        frequency_hz: 5500,
                        gain_db: 2.5,
                        q_factor: 0.7,
                    },
                    EqBand {
                        filter_type: EqFilterType::Peaking,
                        frequency_hz: 2500,
                        gain_db: 1.2,
                        q_factor: 1.0,
                    },
                ],
                compressor: CompressorConfig {
                    enabled: true,
                    threshold_db: -20.0,
                    ratio: 3,
                    attack_ms: 5,
                    release_ms: 100,
                    makeup_gain_db: 6.0,
                },
                limiter: LimiterConfig {
                    enabled: true,
                    threshold_db: -6.0,
                    attack_ms: 1,
                    release_ms: 60,
                },
            },
            DspPreset::NaturalDark => Self {
                preamp_gain_db: 10.0,
                noise_gate: NoiseGateConfig {
                    enabled: true,
                    threshold_db: -58.0,
                    attenuation_percent: 0,
                    attack_ms: 2,
                    release_ms: 100,
                },
                eq_enabled: true,
                eq_bands: [
                    EqBand {
                        filter_type: EqFilterType::LowShelf,
                        frequency_hz: 250,
                        gain_db: 2.0,
                        q_factor: 0.8,
                    },
                    EqBand {
                        filter_type: EqFilterType::HighShelf,
                        frequency_hz: 6000,
                        gain_db: -2.0,
                        q_factor: 0.7,
                    },
                    EqBand {
                        filter_type: EqFilterType::Peaking,
                        frequency_hz: 1500,
                        gain_db: 0.8,
                        q_factor: 1.0,
                    },
                ],
                compressor: CompressorConfig {
                    enabled: true,
                    threshold_db: -20.0,
                    ratio: 3,
                    attack_ms: 5,
                    release_ms: 100,
                    makeup_gain_db: 6.0,
                },
                limiter: LimiterConfig {
                    enabled: true,
                    threshold_db: -6.0,
                    attack_ms: 1,
                    release_ms: 60,
                },
            },
            DspPreset::RadioBright => Self {
                preamp_gain_db: 10.0,
                noise_gate: NoiseGateConfig {
                    enabled: true,
                    threshold_db: -52.0,
                    attenuation_percent: 0,
                    attack_ms: 3,
                    release_ms: 100,
                },
                eq_enabled: true,
                eq_bands: [
                    EqBand {
                        filter_type: EqFilterType::LowShelf,
                        frequency_hz: 280,
                        gain_db: 1.8,
                        q_factor: 1.0,
                    },
                    EqBand {
                        filter_type: EqFilterType::HighShelf,
                        frequency_hz: 6000,
                        gain_db: 3.0,
                        q_factor: 0.7,
                    },
                    EqBand {
                        filter_type: EqFilterType::Peaking,
                        frequency_hz: 2200,
                        gain_db: 1.5,
                        q_factor: 1.0,
                    },
                ],
                compressor: CompressorConfig {
                    enabled: true,
                    threshold_db: -32.0,
                    ratio: 7,
                    attack_ms: 5,
                    release_ms: 100,
                    makeup_gain_db: 14.0,
                },
                limiter: LimiterConfig {
                    enabled: true,
                    threshold_db: -15.0,
                    attack_ms: 1,
                    release_ms: 60,
                },
            },
            DspPreset::RadioDark => Self {
                preamp_gain_db: 10.0,
                noise_gate: NoiseGateConfig {
                    enabled: true,
                    threshold_db: -52.0,
                    attenuation_percent: 0,
                    attack_ms: 3,
                    release_ms: 100,
                },
                eq_enabled: true,
                eq_bands: [
                    EqBand {
                        filter_type: EqFilterType::LowShelf,
                        frequency_hz: 317,
                        gain_db: 2.2,
                        q_factor: 1.0,
                    },
                    EqBand {
                        filter_type: EqFilterType::HighShelf,
                        frequency_hz: 6509,
                        gain_db: -1.0,
                        q_factor: 0.1,
                    },
                    EqBand {
                        filter_type: EqFilterType::Peaking,
                        frequency_hz: 1744,
                        gain_db: 1.4,
                        q_factor: 1.0,
                    },
                ],
                compressor: CompressorConfig {
                    enabled: true,
                    threshold_db: -32.0,
                    ratio: 7,
                    attack_ms: 5,
                    release_ms: 100,
                    makeup_gain_db: 14.0,
                },
                limiter: LimiterConfig {
                    enabled: true,
                    threshold_db: -15.0,
                    attack_ms: 1,
                    release_ms: 60,
                },
            },
            DspPreset::Hxd700Close => Self {
                preamp_gain_db: 12.0,
                noise_gate: NoiseGateConfig {
                    enabled: true,
                    threshold_db: -54.0,
                    attenuation_percent: 0,
                    attack_ms: 2,
                    release_ms: 100,
                },
                eq_enabled: true,
                eq_bands: [
                    EqBand {
                        filter_type: EqFilterType::HighPass,
                        frequency_hz: 80,
                        gain_db: 0.0,
                        q_factor: 0.7,
                    },
                    EqBand {
                        filter_type: EqFilterType::Peaking,
                        frequency_hz: 3500,
                        gain_db: 2.0,
                        q_factor: 1.0,
                    },
                    EqBand {
                        filter_type: EqFilterType::HighShelf,
                        frequency_hz: 8000,
                        gain_db: 1.5,
                        q_factor: 0.7,
                    },
                ],
                compressor: CompressorConfig {
                    enabled: true,
                    threshold_db: -24.0,
                    ratio: 4,
                    attack_ms: 3,
                    release_ms: 100,
                    makeup_gain_db: 8.0,
                },
                limiter: LimiterConfig {
                    enabled: true,
                    threshold_db: -6.0,
                    attack_ms: 1,
                    release_ms: 60,
                },
            },
            DspPreset::Hxd700Table => Self {
                preamp_gain_db: 18.0,
                noise_gate: NoiseGateConfig {
                    enabled: true,
                    threshold_db: -48.0,
                    attenuation_percent: 0,
                    attack_ms: 3,
                    release_ms: 120,
                },
                eq_enabled: true,
                eq_bands: [
                    EqBand {
                        filter_type: EqFilterType::HighPass,
                        frequency_hz: 110,
                        gain_db: 0.0,
                        q_factor: 0.7,
                    },
                    EqBand {
                        filter_type: EqFilterType::Peaking,
                        frequency_hz: 3000,
                        gain_db: 2.5,
                        q_factor: 1.0,
                    },
                    EqBand {
                        filter_type: EqFilterType::HighShelf,
                        frequency_hz: 7500,
                        gain_db: 2.0,
                        q_factor: 0.7,
                    },
                ],
                compressor: CompressorConfig {
                    enabled: true,
                    threshold_db: -28.0,
                    ratio: 5,
                    attack_ms: 3,
                    release_ms: 100,
                    makeup_gain_db: 10.0,
                },
                limiter: LimiterConfig {
                    enabled: true,
                    threshold_db: -6.0,
                    attack_ms: 1,
                    release_ms: 60,
                },
            },
        }
    }

    /// Encodes a single 76-byte (`0x4C`) payload for Command `0x0301` (`88` bytes = `0x58` total with header),
    /// matching `0x180010020` in `hsm_api_core_x64.dll` for iteration `esi = block as usize` (`0..=11`).
    pub fn encode_subblock(&self, block: DspSubBlock) -> [u8; 76] {
        const DEFAULT_EQ_FREQS: [f32; 12] = [
            30.0, 75.0, 150.0, 300.0, 600.0, 1000.0, 2000.0, 4000.0, 8000.0, 12000.0, 14000.0,
            16000.0,
        ];

        let esi = (block as usize).min(11);
        let mut buf = [0u8; 76];

        // +0x0C (payload +0x00): flags (0x7FFF on iteration 0, 0x0400 on iterations 1..11)
        let flags: u32 = if esi == 0 { 0x7FFF } else { 0x0400 };
        buf[0x00..0x04].copy_from_slice(&flags.to_le_bytes());

        // +0x10 (payload +0x04): preamp gain in integer dB
        let preamp_int = self.preamp_gain_db.round() as i32;
        buf[0x04..0x08].copy_from_slice(&preamp_int.to_le_bytes());

        // +0x14..+0x1F (payload +0x08..+0x13): Noise Gate (threshold_db, attack_ms, release_ms)
        let (gate_thresh, gate_attack, gate_release) = if self.noise_gate.enabled {
            (
                self.noise_gate.threshold_db,
                self.noise_gate.attack_ms as f32,
                self.noise_gate.release_ms as f32,
            )
        } else {
            (-100.0f32, 1.0f32, 10.0f32)
        };
        buf[0x08..0x0C].copy_from_slice(&gate_thresh.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&gate_attack.to_le_bytes());
        buf[0x10..0x14].copy_from_slice(&gate_release.to_le_bytes());

        // +0x20..+0x37 (payload +0x14..+0x2B): Compressor
        let (comp_thresh, comp_ratio, comp_attack, comp_release, comp_makeup) =
            if self.compressor.enabled {
                (
                    self.compressor.threshold_db,
                    self.compressor.ratio as f32,
                    self.compressor.attack_ms as f32,
                    self.compressor.release_ms as f32,
                    self.compressor.makeup_gain_db,
                )
            } else {
                (0.0f32, 1.0f32, 2.0f32, 12.0f32, 4.0f32)
            };
        if esi == 0 {
            buf[0x14..0x18].copy_from_slice(&comp_thresh.to_le_bytes());
            buf[0x18..0x1C].copy_from_slice(&comp_ratio.to_le_bytes());
            buf[0x1C..0x1E].copy_from_slice(&1u16.to_le_bytes());
        }
        buf[0x1E..0x20].copy_from_slice(&0u16.to_le_bytes());
        buf[0x20..0x24].copy_from_slice(&comp_attack.to_le_bytes());
        buf[0x24..0x28].copy_from_slice(&comp_release.to_le_bytes());
        buf[0x28..0x2C].copy_from_slice(&comp_makeup.to_le_bytes());

        // +0x38..+0x47 (payload +0x2C..+0x3B): Biquad Equalizer Band `esi`
        buf[0x2C..0x2E].copy_from_slice(&(esi as u16).to_le_bytes());
        let (eq_type, eq_freq, eq_q, eq_gain) = if self.eq_enabled && esi < self.eq_bands.len() {
            let b = self.eq_bands[esi];
            (
                b.filter_type as u16,
                b.frequency_hz as f32,
                b.q_factor,
                b.gain_db,
            )
        } else {
            (0u16, DEFAULT_EQ_FREQS[esi], std::f32::consts::SQRT_2, 0.0f32)
        };
        buf[0x2E..0x30].copy_from_slice(&eq_type.to_le_bytes());
        buf[0x30..0x34].copy_from_slice(&eq_freq.to_le_bytes());
        buf[0x34..0x38].copy_from_slice(&eq_q.to_le_bytes());
        buf[0x38..0x3C].copy_from_slice(&eq_gain.to_le_bytes());

        // +0x48..+0x4B (payload +0x3C..+0x3F): Mic Source (0 = XLR, 1 = Headset)
        buf[0x3C..0x40].copy_from_slice(&0u32.to_le_bytes());

        // +0x4C..+0x57 (payload +0x40..+0x4B): Limiter
        let (lim_thresh, lim_attack, lim_release) = if self.limiter.enabled {
            (
                self.limiter.threshold_db,
                self.limiter.attack_ms as f32,
                self.limiter.release_ms as f32,
            )
        } else {
            (-6.0f32, 1.0f32, 60.0f32)
        };
        buf[0x40..0x44].copy_from_slice(&lim_thresh.to_le_bytes());
        buf[0x44..0x48].copy_from_slice(&lim_attack.to_le_bytes());
        buf[0x48..0x4C].copy_from_slice(&lim_release.to_le_bytes());

        buf
    }
}
