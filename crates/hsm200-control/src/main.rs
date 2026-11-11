use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use hsm200_driver::{
    Button, DspPreset, HardwareEvent, LCD_HEIGHT, LCD_WIDTH, LedMode, MicSource, MixBus, RgbColor,
    Stream200Xlr, make_test_pattern_rgb565_frame, rgb888_to_rgb565,
};
use image::imageops::FilterType;

#[derive(Parser)]
#[command(
    name = "hsm200-control",
    version,
    about = "Hercules Stream 200 XLR (06f8:e054) Linux Driver & Control CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Display USB device info, firmware version, and a snapshot of all buttons, encoders, and faders
    Info,

    /// Live event monitor for buttons, rotary encoders, and hardware state changes (0x0101)
    Monitor {
        /// Polling interval in milliseconds (default: 20ms = 50 Hz)
        #[arg(short, long, default_value_t = 20)]
        interval_ms: u64,

        /// Print raw hex button masks on every event
        #[arg(short, long)]
        raw: bool,
    },

    /// Configure Audio Mixer, channel dB faders, mutes, +48V Phantom Power, and Mic Input Source (0x0401)
    Audio {
        /// Enable or disable +48V Phantom Power on the XLR input
        #[arg(long)]
        phantom_48v: Option<OnOffArg>,

        /// Select physical microphone connector (xlr or headset)
        #[arg(long)]
        mic_source: Option<MicSourceArg>,

        /// Target channel index (1..=4) for gain or mute changes
        #[arg(short, long)]
        channel: Option<u8>,

        /// Set Creator Mix gain in dB (-110.0 to +10.0) for --channel (or all 4 channels if --channel omitted)
        #[arg(long, allow_hyphen_values = true)]
        creator_db: Option<f32>,

        /// Set Audience Mix gain in dB (-110.0 to +10.0) for --channel (or all 4 channels if --channel omitted)
        #[arg(long, allow_hyphen_values = true)]
        audience_db: Option<f32>,

        /// Mute or unmute Creator Mix channel(s)
        #[arg(long)]
        creator_mute: Option<OnOffArg>,

        /// Mute or unmute Audience Mix channel(s)
        #[arg(long)]
        audience_mute: Option<OnOffArg>,
    },

    /// Configure AK7735EQ Microphone DSP (Preamp, Noise Gate, EQ, Compressor, Limiter) or load a preset (0x0301)
    Dsp {
        /// Apply one of the 8 official microphone DSP presets
        #[arg(short, long, value_enum, default_value_t = DspPresetArg::Default)]
        preset: DspPresetArg,

        /// Override microphone preamp gain in dB (e.g. 10.0)
        #[arg(long, allow_hyphen_values = true)]
        preamp_db: Option<f32>,

        /// Override noise gate threshold in dB (e.g. -55.0)
        #[arg(long, allow_hyphen_values = true)]
        gate_threshold_db: Option<f32>,

        /// Override compressor threshold in dB (e.g. -20.0)
        #[arg(long, allow_hyphen_values = true)]
        comp_threshold_db: Option<f32>,

        /// Override limiter threshold in dB (e.g. -6.0)
        #[arg(long, allow_hyphen_values = true)]
        limiter_threshold_db: Option<f32>,
    },

    /// Control physical button LEDs, LCD backlight brightness, RGB meter colors, and VU meters (0x0501 / 0x0601)
    Led {
        /// Set LCD screen backlight brightness (0..=100%)
        #[arg(short, long)]
        brightness: Option<u8>,

        /// Target button (`action1`..`action4`, `enc1`..`enc4`, `ctrl-a`..`ctrl-f`, or `all`)
        #[arg(long)]
        button: Option<ButtonTargetArg>,

        /// Button LED illumination mode (`off`, `solid`, `blink`)
        #[arg(long, value_enum)]
        mode: Option<LedModeArg>,

        /// Set top/middle/bottom RGB hex color (`RRGGBB`) for all 4 channel meters
        #[arg(long)]
        meter_color: Option<String>,

        /// Set VU meter test level (0..=255) across all 8 channels
        #[arg(long)]
        vu_level: Option<u8>,
    },

    /// Control the 480x272 LCD display: wake/sleep, switch page, draw test pattern, or upload an image (0x0801)
    Display {
        /// Wake or sleep the LCD display
        #[arg(long)]
        power: Option<OnOffArg>,

        /// Switch active mixer page on the LCD (1 or 2)
        #[arg(short, long)]
        page: Option<u8>,

        /// Fill the 480x272 LCD with a solid hex color (`RRGGBB`, e.g. `002244`)
        #[arg(long)]
        fill: Option<String>,

        /// Upload a 480x272 color-bars test pattern to the LCD
        #[arg(long)]
        test_pattern: bool,

        /// Upload a PNG, JPEG, or BMP image file (automatically resized to 480x272 RGB565)
        #[arg(short, long)]
        image: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OnOffArg {
    On,
    Off,
}

impl OnOffArg {
    fn as_bool(self) -> bool {
        matches!(self, OnOffArg::On)
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum MicSourceArg {
    Xlr,
    Headset,
}

#[derive(Clone, Copy, ValueEnum)]
enum DspPresetArg {
    Default,
    FxDisabled,
    NaturalBright,
    NaturalDark,
    RadioBright,
    RadioDark,
    Hxd700Close,
    Hxd700Table,
}

impl From<DspPresetArg> for DspPreset {
    fn from(arg: DspPresetArg) -> Self {
        match arg {
            DspPresetArg::Default => DspPreset::Default,
            DspPresetArg::FxDisabled => DspPreset::FxDisabled,
            DspPresetArg::NaturalBright => DspPreset::NaturalBright,
            DspPresetArg::NaturalDark => DspPreset::NaturalDark,
            DspPresetArg::RadioBright => DspPreset::RadioBright,
            DspPresetArg::RadioDark => DspPreset::RadioDark,
            DspPresetArg::Hxd700Close => DspPreset::Hxd700Close,
            DspPresetArg::Hxd700Table => DspPreset::Hxd700Table,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum LedModeArg {
    Off,
    Solid,
    Blink,
}

impl From<LedModeArg> for LedMode {
    fn from(arg: LedModeArg) -> Self {
        match arg {
            LedModeArg::Off => LedMode::Off,
            LedModeArg::Solid => LedMode::Solid,
            LedModeArg::Blink => LedMode::Blink,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum ButtonTargetArg {
    All,
    Action1,
    Action2,
    Action3,
    Action4,
    Enc1,
    Enc2,
    Enc3,
    Enc4,
    CtrlA,
    CtrlB,
    CtrlC,
    CtrlD,
    CtrlE,
    CtrlF,
}

impl ButtonTargetArg {
    fn to_button(self) -> Option<Button> {
        match self {
            ButtonTargetArg::All => None,
            ButtonTargetArg::Action1 => Some(Button::Action1),
            ButtonTargetArg::Action2 => Some(Button::Action2),
            ButtonTargetArg::Action3 => Some(Button::Action3),
            ButtonTargetArg::Action4 => Some(Button::Action4),
            ButtonTargetArg::Enc1 => Some(Button::Encoder1Push),
            ButtonTargetArg::Enc2 => Some(Button::Encoder2Push),
            ButtonTargetArg::Enc3 => Some(Button::Encoder3Push),
            ButtonTargetArg::Enc4 => Some(Button::Encoder4Push),
            ButtonTargetArg::CtrlA => Some(Button::ControlA),
            ButtonTargetArg::CtrlB => Some(Button::ControlB),
            ButtonTargetArg::CtrlC => Some(Button::ControlC),
            ButtonTargetArg::CtrlD => Some(Button::ControlD),
            ButtonTargetArg::CtrlE => Some(Button::ControlE),
            ButtonTargetArg::CtrlF => Some(Button::ControlF),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut dev = Stream200Xlr::open()?;

    match cli.command {
        Commands::Info => {
            let info = dev.device_info().clone();
            println!("=== Hercules Stream 200 XLR ===");
            println!(
                "USB Location     : Bus {:03} Device {:03} ({:04x}:{:04x})",
                info.bus_number, info.address, info.vendor_id, info.product_id
            );
            println!("Manufacturer     : {}", info.manufacturer);
            println!("Product          : {}", info.product);
            println!("Serial Number    : {}", info.serial_number);
            println!(
                "Firmware Version : {} (bcdDevice 0x{:04x})",
                info.firmware_version_string(),
                info.firmware_bcd
            );
            println!(
                "USB Buffers (EP0): Proto=0x{:04x}, RX_Max={} B, TX_Max={} B",
                info.protocol_version, info.rx_max_transfer_size, info.tx_max_transfer_size
            );

            if let Ok(state_bytes) = dev.query_device_state() {
                print!("Device State (0x0A01): ");
                for b in &state_bytes {
                    print!("{b:02X} ");
                }
                println!();
            }

            let report = dev.poll_status()?;
            println!("\n--- Input Report Snapshot (0x0101) ---");
            println!(
                "Raw Button Mask  : [{:02X}, {:02X}, {:02X}, {:02X}] (Lower 4 Action bits: {:04b}, Encoder Push bits: {:04b})",
                report.raw_buttons[0],
                report.raw_buttons[1],
                report.raw_buttons[2],
                report.raw_buttons[3],
                report.action_buttons_nibble(),
                report.encoder_push_nibble(),
            );
            let pressed = report.pressed_buttons();
            if pressed.is_empty() {
                println!("Pressed Buttons  : (none)");
            } else {
                let names: Vec<&str> = pressed.iter().map(|b| b.name()).collect();
                println!("Pressed Buttons  : {}", names.join(", "));
            }
            println!(
                "Rotary Encoders  : Enc1={} | Enc2={} | Enc3={} | Enc4={} | Aux={}",
                report.encoders[0] as i16,
                report.encoders[1] as i16,
                report.encoders[2] as i16,
                report.encoders[3] as i16,
                report.aux_encoder
            );
            println!(
                "Hardware Flags   : 0x{:02X} | DSP Status: 0x{:08X}",
                report.hw_flags, report.dsp_status
            );
            println!(
                "Headphone Volume : {:.2}% (raw {:.1})",
                report.headphone_volume / 100.0,
                report.headphone_volume
            );
            let creator_pct: Vec<String> = report.creator_faders[0..4]
                .iter()
                .map(|v| format!("{:.1}%", v / 100.0))
                .collect();
            let audience_pct: Vec<String> = report.audience_faders[0..4]
                .iter()
                .map(|v| format!("{:.1}%", v / 100.0))
                .collect();
            println!("Creator Faders   : [{}]", creator_pct.join(", "));
            println!("Audience Faders  : [{}]", audience_pct.join(", "));
        }

        Commands::Monitor { interval_ms, raw } => {
            let info = dev.device_info();
            println!(
                "Monitoring {} (Serial: {}, FW: {}) at {} ms interval. Press Ctrl+C to stop.",
                info.product,
                info.serial_number,
                info.firmware_version_string(),
                interval_ms
            );

            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
            })?;

            // Prime initial state
            let (initial, _) = dev.poll_events()?;
            println!(
                "[INIT] Buttons=[{:02X} {:02X} {:02X} {:02X}] Encoders={:?} Aux={} Flags=0x{:02X}",
                initial.raw_buttons[0],
                initial.raw_buttons[1],
                initial.raw_buttons[2],
                initial.raw_buttons[3],
                initial.encoders,
                initial.aux_encoder,
                initial.hw_flags
            );

            let sleep_dur = Duration::from_millis(interval_ms.max(2));
            while running.load(Ordering::SeqCst) {
                let (report, events) = dev.poll_events()?;
                for ev in events {
                    match ev {
                        HardwareEvent::ButtonPressed(btn) => {
                            if raw {
                                println!(
                                    "[BUTTON DOWN] {:<28} | raw=[{:02X} {:02X} {:02X} {:02X}] action_bits={:04b}",
                                    btn.name(),
                                    report.raw_buttons[0],
                                    report.raw_buttons[1],
                                    report.raw_buttons[2],
                                    report.raw_buttons[3],
                                    report.action_buttons_nibble()
                                );
                            } else {
                                println!(
                                    "[BUTTON DOWN] {} (lower 4 action bits: {:04b})",
                                    btn.name(),
                                    report.action_buttons_nibble()
                                );
                            }
                        }
                        HardwareEvent::ButtonReleased(btn) => {
                            if raw {
                                println!(
                                    "[BUTTON UP  ] {:<28} | raw=[{:02X} {:02X} {:02X} {:02X}] action_bits={:04b}",
                                    btn.name(),
                                    report.raw_buttons[0],
                                    report.raw_buttons[1],
                                    report.raw_buttons[2],
                                    report.raw_buttons[3],
                                    report.action_buttons_nibble()
                                );
                            } else {
                                println!(
                                    "[BUTTON UP  ] {} (lower 4 action bits: {:04b})",
                                    btn.name(),
                                    report.action_buttons_nibble()
                                );
                            }
                        }
                        HardwareEvent::EncoderTurned {
                            encoder,
                            delta,
                            counter,
                        } => {
                            let dir = if delta > 0 { "CW (+)" } else { "CCW (-)" };
                            println!(
                                "[ENCODER {encoder}  ] {dir:<7} delta={delta:+} (counter={counter})"
                            );
                        }
                        HardwareEvent::AuxEncoderChanged {
                            old_value,
                            new_value,
                        } => {
                            println!("[AUX ENCODER] {old_value} -> {new_value}");
                        }
                        HardwareEvent::HwFlagsChanged {
                            old_flags,
                            new_flags,
                        } => {
                            println!("[HW FLAGS   ] 0x{old_flags:02X} -> 0x{new_flags:02X}");
                        }
                    }
                }
                thread::sleep(sleep_dur);
            }
            println!("\nStopped monitoring.");
        }

        Commands::Audio {
            phantom_48v,
            mic_source,
            channel,
            creator_db,
            audience_db,
            creator_mute,
            audience_mute,
        } => {
            if let Some(p48) = phantom_48v {
                dev.set_phantom_power(p48.as_bool())?;
                println!("Set +48V Phantom Power (0x0401): {}", p48.as_bool());
            }
            if let Some(src) = mic_source {
                let s = match src {
                    MicSourceArg::Xlr => MicSource::Xlr,
                    MicSourceArg::Headset => MicSource::Headset,
                };
                dev.set_mic_source(s)?;
                println!("Set Mic Input Source (0x0401): {s:?}");
            }

            let indices: Vec<u8> = match channel {
                Some(ch) if (1..=4).contains(&ch) => vec![ch - 1],
                Some(ch) => {
                    return Err(format!("--channel must be between 1 and 4 (got {ch})").into());
                }
                None => vec![0, 1, 2, 3],
            };

            if creator_db.is_some() || creator_mute.is_some() {
                for &idx in &indices {
                    dev.set_channel_fader(
                        MixBus::Creator,
                        idx,
                        creator_db,
                        creator_mute.map(|m| m.as_bool()),
                    )?;
                }
                println!(
                    "Updated Creator Mix (0x0401) channels {:?}: dB={:?}, mute={:?}",
                    indices,
                    creator_db,
                    creator_mute.map(|m| m.as_bool())
                );
            }

            if audience_db.is_some() || audience_mute.is_some() {
                for &idx in &indices {
                    dev.set_channel_fader(
                        MixBus::Audience,
                        idx,
                        audience_db,
                        audience_mute.map(|m| m.as_bool()),
                    )?;
                }
                println!(
                    "Updated Audience Mix (0x0401) channels {:?}: dB={:?}, mute={:?}",
                    indices,
                    audience_db,
                    audience_mute.map(|m| m.as_bool())
                );
            }
        }

        Commands::Dsp {
            preset,
            preamp_db,
            gate_threshold_db,
            comp_threshold_db,
            limiter_threshold_db,
        } => {
            dev.dsp = hsm200_driver::MicDspConfig::from_preset(preset.into());
            if let Some(gain) = preamp_db {
                dev.dsp.preamp_gain_db = gain;
            }
            if let Some(thresh) = gate_threshold_db {
                dev.dsp.noise_gate.enabled = true;
                dev.dsp.noise_gate.threshold_db = thresh;
            }
            if let Some(thresh) = comp_threshold_db {
                dev.dsp.compressor.enabled = true;
                dev.dsp.compressor.threshold_db = thresh;
            }
            if let Some(thresh) = limiter_threshold_db {
                dev.dsp.limiter.enabled = true;
                dev.dsp.limiter.threshold_db = thresh;
            }

            dev.sync_dsp()?;
            println!(
                "Synced AK7735EQ Microphone DSP (0x0301): Preamp={:.1} dB, Gate={:.1} dB, Comp={:.1} dB, Limiter={:.1} dB",
                dev.dsp.preamp_gain_db,
                dev.dsp.noise_gate.threshold_db,
                dev.dsp.compressor.threshold_db,
                dev.dsp.limiter.threshold_db
            );
        }

        Commands::Led {
            brightness,
            button,
            mode,
            meter_color,
            vu_level,
        } => {
            let mut update_backlights = false;
            if let Some(b) = brightness {
                dev.backlights.lcd_brightness = b.min(100);
                update_backlights = true;
            }
            if let (Some(btn_arg), Some(mode_arg)) = (button, mode) {
                let led_mode: LedMode = mode_arg.into();
                match btn_arg.to_button() {
                    Some(btn) => dev.backlights.set_button(btn, led_mode),
                    None => dev.backlights.set_all_buttons(led_mode),
                }
                update_backlights = true;
            }
            if update_backlights {
                dev.sync_backlights()?;
                println!(
                    "Synced Button LEDs & LCD Backlight (0x0601): solid=0x{:04X}, blink=0x{:04X}, brightness={}%",
                    dev.backlights.solid_mask,
                    dev.backlights.blink_mask,
                    dev.backlights.lcd_brightness
                );
            }

            let mut update_meters = false;
            if let Some(hex) = meter_color {
                let color = RgbColor::from_hex(&hex)
                    .ok_or_else(|| format!("Invalid hex color '{hex}' (expected RRGGBB)"))?;
                for ch in &mut dev.rgb_meters.channels {
                    ch.top = color;
                    ch.middle = color;
                    ch.bottom = color;
                }
                update_meters = true;
            }
            if let Some(vu) = vu_level {
                dev.rgb_meters.vu_levels = [vu; 8];
                update_meters = true;
            }
            if update_meters {
                dev.sync_rgb_meters()?;
                println!("Synced RGB Meter Colors & VU Levels (0x0501).");
            }
        }

        Commands::Display {
            power,
            page,
            fill,
            test_pattern,
            image,
        } => {
            if let Some(pwr) = power {
                dev.set_screen_power(pwr.as_bool())?;
                println!("Set LCD screen power: {:?}", pwr.as_bool());
            }
            if let Some(pg) = page {
                dev.set_screen_page(pg)?;
                println!("Switched LCD screen to Page {pg}.");
            }
            if let Some(hex) = fill {
                let color = RgbColor::from_hex(&hex)
                    .ok_or_else(|| format!("Invalid hex color '{hex}' (expected RRGGBB)"))?;
                dev.fill_screen_background(color)?;
                println!(
                    "Uploaded 480x272 solid background (#{:02X}{:02X}{:02X}) to LCD.",
                    color.r, color.g, color.b
                );
            }
            if test_pattern {
                let frame = make_test_pattern_rgb565_frame();
                dev.upload_rgb565_frame(&frame)?;
                println!("Uploaded 480x272 RGB565 color-bars test pattern to LCD.");
            }
            if let Some(img_path) = image {
                let img = image::open(&img_path)?;
                let resized =
                    img.resize_exact(LCD_WIDTH as u32, LCD_HEIGHT as u32, FilterType::Lanczos3);
                let rgb8 = resized.to_rgb8();
                let mut rgb565_le =
                    Vec::with_capacity((LCD_WIDTH as usize) * (LCD_HEIGHT as usize) * 2);
                for pixel in rgb8.pixels() {
                    let p565 = rgb888_to_rgb565(pixel[0], pixel[1], pixel[2]);
                    rgb565_le.extend_from_slice(&p565.to_le_bytes());
                }
                dev.upload_rgb565_frame(&rgb565_le)?;
                println!(
                    "Uploaded '{}' (resized to 480x272 RGB565) to LCD.",
                    img_path.display()
                );
            }
        }
    }

    Ok(())
}
