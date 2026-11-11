use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hsm200_driver::{
    display, Button, CommandId, DeviceInfo, DspPreset, HardwareEvent, InputReport, LedMode,
    MicDspConfig, MicSource, MixBus, MixerState, RgbColor, Stream200Xlr,
};

/// Commands sent from the GUI thread to the background USB device worker thread.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GuiCommand {
    /// Update mixer channel creator/audience fader values and mutes.
    SyncMixer,
    /// Set a single channel's fader volume percentage (0..=100%).
    SetChannelVolume {
        channel: u8,
        bus: MixBus,
        volume_pct: f32,
    },
    /// Set a single channel's mute state.
    SetChannelMute {
        channel: u8,
        bus: MixBus,
        muted: bool,
    },
    /// Set +48V Phantom Power for the XLR microphone.
    SetPhantomPower(bool),
    /// Set physical microphone input source (Xlr vs Headset).
    SetMicSource(MicSource),
    /// Set headphone / master monitor volume (0..=100%).
    SetHeadphoneVolume(f32),
    /// Send full DSP configuration.
    SyncDsp(MicDspConfig),
    /// Apply official DSP preset.
    ApplyDspPreset(DspPreset),
    /// Set a physical button LED mode.
    SetButtonLed { button: Button, mode: LedMode },
    /// Set all physical button LEDs.
    SetAllButtonsLed(LedMode),
    /// Set LCD screen backlight intensity (0..=100%).
    SetLcdBrightness(u8),
    /// Set RGB meter colors and VU levels.
    SyncRgbMeters,
    /// Set LCD screen power.
    SetScreenPower(bool),
    /// Switch active LCD screen page (1 or 2).
    SetScreenPage(u8),
    /// Switch LCD screen layout mode.
    SetScreenLayout(u8),
    /// Fill LCD background wallpaper with a solid color.
    FillScreenBackground { color: RgbColor, slot: u32 },
    /// Upload full-screen RGB565 wallpaper or splash image.
    UploadImage {
        rgb565_le: Vec<u8>,
        slot: u32,
        splash: bool,
    },
}

/// Messages emitted from the background USB worker thread to the GUI thread.
#[derive(Debug)]
pub enum DeviceEvent {
    Connected(DeviceInfo),
    Disconnected(String),
    InputUpdate {
        report: InputReport,
        events: Vec<HardwareEvent>,
    },
    CommandSuccess(String),
    CommandError(String),
}

/// Handle to the background USB worker bridge.
pub struct DeviceBridge {
    cmd_tx: Sender<GuiCommand>,
    event_rx: Receiver<DeviceEvent>,
    _worker: JoinHandle<()>,
}

impl DeviceBridge {
    /// Spawns the background USB polling and command execution thread.
    pub fn spawn() -> Self {
        let (cmd_tx, cmd_rx) = channel::<GuiCommand>();
        let (event_tx, event_rx) = channel::<DeviceEvent>();

        let worker = thread::spawn(move || {
            worker_loop(cmd_rx, event_tx);
        });

        Self {
            cmd_tx,
            event_rx,
            _worker: worker,
        }
    }

    /// Sends a non-blocking command to the USB device.
    pub fn send(&self, cmd: GuiCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Drains all pending events from the worker thread.
    pub fn drain_events(&self) -> Vec<DeviceEvent> {
        let mut events = Vec::new();
        while let Ok(ev) = self.event_rx.try_recv() {
            events.push(ev);
        }
        events
    }
}

fn worker_loop(cmd_rx: Receiver<GuiCommand>, event_tx: Sender<DeviceEvent>) {
    let mut dev_opt: Option<Stream200Xlr> = None;
    let mut last_connect_attempt = Instant::now() - Duration::from_secs(5);

    loop {
        // Attempt connection if not connected
        if dev_opt.is_none() {
            if last_connect_attempt.elapsed() >= Duration::from_millis(1500) {
                last_connect_attempt = Instant::now();
                match Stream200Xlr::open() {
                    Ok(dev) => {
                        let info = dev.device_info().clone();
                        let _ = event_tx.send(DeviceEvent::Connected(info));
                        dev_opt = Some(dev);
                    }
                    Err(err) => {
                        let _ = event_tx.send(DeviceEvent::Disconnected(err.to_string()));
                    }
                }
            } else {
                // Drain any incoming GUI commands while disconnected
                while let Ok(cmd) = cmd_rx.try_recv() {
                    let _ = event_tx.send(DeviceEvent::CommandError(format!(
                        "Device not connected (cannot execute {cmd:?})"
                    )));
                }
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        }

        // We have an active device handle
        let mut dev = match dev_opt.take() {
            Some(d) => d,
            None => continue,
        };

        // 1. Process pending GUI commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            let res = match cmd {
                GuiCommand::SyncMixer => dev.sync_mixer().map(|_| "Synced Audio Mixer".to_string()),
                GuiCommand::SetChannelVolume { channel, bus, volume_pct } => {
                    let db = MixerState::percent_to_db(volume_pct);
                    dev.set_channel_fader(bus, channel, Some(db), None)
                        .map(|_| format!("Ch {} {:?}: {:.0}% ({:.1} dB)", channel + 1, bus, volume_pct, db))
                }
                GuiCommand::SetChannelMute { channel, bus, muted } => {
                    dev.set_channel_fader(bus, channel, None, Some(muted))
                        .map(|_| format!("Ch {} {:?} Mute: {}", channel + 1, bus, muted))
                }
                GuiCommand::SetPhantomPower(en) => {
                    dev.set_phantom_power(en)
                        .map(|_| format!("+48V Phantom Power: {}", if en { "ENABLED" } else { "DISABLED" }))
                }
                GuiCommand::SetMicSource(src) => {
                    dev.set_mic_source(src)
                        .map(|_| format!("Microphone Source: {:?}", src))
                }
                GuiCommand::SetHeadphoneVolume(vol) => {
                    let db = MixerState::percent_to_db(vol);
                    dev.mixer.master_output_db[0] = db;
                    let payload = MixerState::encode_channel_update(MixBus::MasterOutput, 0, Some(db), None);
                    let _ = dev.transport_mut().transact(CommandId::AudioMixer, &payload);
                    Ok(format!("Headphone Volume: {:.0}%", vol))
                }
                GuiCommand::SyncDsp(dsp_cfg) => {
                    dev.dsp = dsp_cfg;
                    dev.sync_dsp().map(|_| "Synced AK7735EQ Mic DSP".to_string())
                }
                GuiCommand::ApplyDspPreset(preset) => dev
                    .apply_dsp_preset(preset)
                    .map(|_| format!("Applied DSP Preset: {preset:?}")),
                GuiCommand::SetButtonLed { button, mode } => {
                    dev.backlights.set_button(button, mode);
                    dev.sync_backlights()
                        .map(|_| format!("Set button {:?} to {:?}", button, mode))
                }
                GuiCommand::SetAllButtonsLed(mode) => {
                    dev.backlights.set_all_buttons(mode);
                    dev.sync_backlights()
                        .map(|_| format!("Set all buttons to {:?}", mode))
                }
                GuiCommand::SetLcdBrightness(val) => dev
                    .set_lcd_brightness(val)
                    .map(|_| format!("Set LCD brightness to {}%", val)),
                GuiCommand::SyncRgbMeters => dev
                    .sync_rgb_meters()
                    .map(|_| "Synced RGB Meters & VU Levels".to_string()),
                GuiCommand::SetScreenPower(pwr) => dev
                    .set_screen_power(pwr)
                    .map(|_| format!("Set screen power: {pwr}")),
                GuiCommand::SetScreenPage(page) => dev
                    .set_screen_page(page)
                    .map(|_| format!("Switched to Page {page}")),
                GuiCommand::SetScreenLayout(layout) => {
                    display::set_screen_layout(dev.transport_mut(), layout)
                        .map(|_| format!("Set screen layout mode {layout}"))
                }
                GuiCommand::FillScreenBackground { color, slot } => dev
                    .fill_screen_background_slot(color, slot)
                    .map(|_| format!("Filled background (slot {slot})")),
                GuiCommand::UploadImage {
                    rgb565_le,
                    slot,
                    splash,
                } => {
                    if splash {
                        dev.upload_rgb565_splash(&rgb565_le)
                            .map(|_| "Uploaded RGB565 Boot Splash".to_string())
                    } else {
                        dev.upload_rgb565_frame_slot(&rgb565_le, slot)
                            .map(|_| format!("Uploaded wallpaper to slot {slot}"))
                    }
                }
            };

            match res {
                Ok(msg) => {
                    let _ = event_tx.send(DeviceEvent::CommandSuccess(msg));
                }
                Err(err) => {
                    let _ = event_tx.send(DeviceEvent::CommandError(err.to_string()));
                }
            }
        }

        // 2. Poll hardware status & events (20 ms interval)
        match dev.poll_events() {
            Ok((report, events)) => {
                let _ = event_tx.send(DeviceEvent::InputUpdate { report, events });
                dev_opt = Some(dev);
            }
            Err(err) => {
                let _ = event_tx.send(DeviceEvent::Disconnected(format!(
                    "Lost connection: {err}"
                )));
                dev_opt = None; // Trigger re-connect on next loop
            }
        }

        thread::sleep(Duration::from_millis(20));
    }
}
