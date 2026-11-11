//! Host-side audio control for Linux (PipeWire / PulseAudio via `pactl`).
//!
//! The Stream 200 XLR shows up as a single stereo sound card on Linux, so "channels"
//! are implemented here in software: each knob can be bound to the system volume,
//! the default microphone, or any running application's playback stream.

use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Something a knob can control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Target {
    None,
    /// Default output device.
    System,
    /// Default input device (microphone).
    Mic,
    /// Every playback stream whose application name contains this text.
    App(String),
}

impl Target {
    pub fn label(&self) -> String {
        match self {
            Target::None => "Unassigned".into(),
            Target::System => "System Volume".into(),
            Target::Mic => "Microphone".into(),
            Target::App(n) if n.is_empty() => "Pick an app".into(),
            Target::App(n) => n.clone(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AudioApp {
    pub id: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Level {
    pub volume: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AudioSnapshot {
    pub apps: Vec<AudioApp>,
    pub system: Option<Level>,
    pub mic: Option<Level>,
}

impl AudioSnapshot {
    fn matches<'a>(&'a self, needle: &str) -> Vec<&'a AudioApp> {
        let n = needle.to_lowercase();
        if n.is_empty() {
            return Vec::new();
        }
        self.apps
            .iter()
            .filter(|a| a.name.to_lowercase().contains(&n))
            .collect()
    }

    /// Current level of a target, or `None` if it is unassigned / not running.
    pub fn level(&self, t: &Target) -> Option<Level> {
        match t {
            Target::None => None,
            Target::System => self.system,
            Target::Mic => self.mic,
            Target::App(n) => {
                let m = self.matches(n);
                if m.is_empty() {
                    None
                } else {
                    Some(Level {
                        volume: m.iter().map(|a| a.volume).fold(0.0, f32::max),
                        muted: m.iter().all(|a| a.muted),
                    })
                }
            }
        }
    }

    fn apply_local(&mut self, t: &Target, volume: Option<f32>, muted: Option<bool>) {
        let upd = |l: &mut Level| {
            if let Some(v) = volume {
                l.volume = v;
            }
            if let Some(m) = muted {
                l.muted = m;
            }
        };
        match t {
            Target::System => self.system.as_mut().map(upd).unwrap_or(()),
            Target::Mic => self.mic.as_mut().map(upd).unwrap_or(()),
            Target::App(n) => {
                let n = n.to_lowercase();
                for a in self.apps.iter_mut().filter(|a| a.name.to_lowercase().contains(&n)) {
                    if let Some(v) = volume {
                        a.volume = v;
                    }
                    if let Some(m) = muted {
                        a.muted = m;
                    }
                }
            }
            Target::None => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCmd {
    PlayPause,
    Next,
    Previous,
}

#[derive(Debug)]
enum AudioCmd {
    SetVolume(Target, f32),
    SetMute(Target, bool),
    Media(MediaCmd),
    Run(String),
}

/// Handle to the background audio worker.
pub struct AudioBridge {
    tx: Sender<AudioCmd>,
    snap: Arc<Mutex<AudioSnapshot>>,
}

impl AudioBridge {
    pub fn spawn() -> Self {
        let (tx, rx) = channel::<AudioCmd>();
        let snap = Arc::new(Mutex::new(refresh()));
        let worker_snap = Arc::clone(&snap);

        thread::spawn(move || {
            let mut last_refresh = Instant::now();
            let mut last_cmd = Instant::now() - Duration::from_secs(5);
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(cmd) => {
                        last_cmd = Instant::now();
                        let current = worker_snap.lock().unwrap().clone();
                        execute(cmd, &current);
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                // Don't overwrite optimistic UI state right after the user changed something.
                if last_refresh.elapsed() > Duration::from_millis(700)
                    && last_cmd.elapsed() > Duration::from_millis(500)
                {
                    *worker_snap.lock().unwrap() = refresh();
                    last_refresh = Instant::now();
                }
            }
        });

        Self { tx, snap }
    }

    pub fn snapshot(&self) -> AudioSnapshot {
        self.snap.lock().unwrap().clone()
    }

    pub fn set_volume(&self, t: &Target, volume: f32) {
        let v = volume.clamp(0.0, 1.0);
        self.snap.lock().unwrap().apply_local(t, Some(v), None);
        let _ = self.tx.send(AudioCmd::SetVolume(t.clone(), v));
    }

    pub fn set_mute(&self, t: &Target, muted: bool) {
        self.snap.lock().unwrap().apply_local(t, None, Some(muted));
        let _ = self.tx.send(AudioCmd::SetMute(t.clone(), muted));
    }

    /// Toggles mute; returns the new state if the target exists.
    pub fn toggle_mute(&self, t: &Target) -> Option<bool> {
        let lvl = self.snapshot().level(t)?;
        self.set_mute(t, !lvl.muted);
        Some(!lvl.muted)
    }

    pub fn media(&self, cmd: MediaCmd) {
        let _ = self.tx.send(AudioCmd::Media(cmd));
    }

    pub fn run_command(&self, cmd: String) {
        let _ = self.tx.send(AudioCmd::Run(cmd));
    }
}

fn pactl(args: &[&str]) -> Option<String> {
    let out = Command::new("pactl")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn pactl_json(args: &[&str]) -> Vec<Value> {
    let mut full = vec!["--format=json"];
    full.extend_from_slice(args);
    pactl(&full)
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

fn volume_of(v: &Value) -> f32 {
    v.get("volume")
        .and_then(|o| o.as_object())
        .map(|o| {
            o.values()
                .filter_map(|c| c.get("value").and_then(|x| x.as_f64()))
                .fold(0.0, f64::max)
        })
        .unwrap_or(0.0) as f32
        / 65536.0
}

fn device_level(list: &[Value], default_name: &str) -> Option<Level> {
    list.iter()
        .find(|d| d.get("name").and_then(|n| n.as_str()) == Some(default_name))
        .map(|d| Level {
            volume: volume_of(d),
            muted: d.get("mute").and_then(|m| m.as_bool()).unwrap_or(false),
        })
}

fn refresh() -> AudioSnapshot {
    let apps = pactl_json(&["list", "sink-inputs"])
        .iter()
        .filter_map(|v| {
            let name = v
                .pointer("/properties/application.name")
                .or_else(|| v.pointer("/properties/media.name"))
                .and_then(|n| n.as_str())?
                .to_string();
            Some(AudioApp {
                id: v.get("index")?.as_u64()? as u32,
                name,
                volume: volume_of(v),
                muted: v.get("mute").and_then(|m| m.as_bool()).unwrap_or(false),
            })
        })
        .collect();

    let default_sink = pactl(&["get-default-sink"]).unwrap_or_default();
    let default_source = pactl(&["get-default-source"]).unwrap_or_default();
    let system = device_level(&pactl_json(&["list", "sinks"]), default_sink.trim());
    let mic = device_level(&pactl_json(&["list", "sources"]), default_source.trim());

    AudioSnapshot { apps, system, mic }
}

fn execute(cmd: AudioCmd, snap: &AudioSnapshot) {
    match cmd {
        AudioCmd::SetVolume(t, v) => {
            let pct = format!("{}%", (v * 100.0).round() as i32);
            match &t {
                Target::System => {
                    pactl(&["set-sink-volume", "@DEFAULT_SINK@", &pct]);
                }
                Target::Mic => {
                    pactl(&["set-source-volume", "@DEFAULT_SOURCE@", &pct]);
                }
                Target::App(n) => {
                    for a in snap.matches(n) {
                        pactl(&["set-sink-input-volume", &a.id.to_string(), &pct]);
                    }
                }
                Target::None => {}
            }
        }
        AudioCmd::SetMute(t, m) => {
            let flag = if m { "1" } else { "0" };
            match &t {
                Target::System => {
                    pactl(&["set-sink-mute", "@DEFAULT_SINK@", flag]);
                }
                Target::Mic => {
                    pactl(&["set-source-mute", "@DEFAULT_SOURCE@", flag]);
                }
                Target::App(n) => {
                    for a in snap.matches(n) {
                        pactl(&["set-sink-input-mute", &a.id.to_string(), flag]);
                    }
                }
                Target::None => {}
            }
        }
        AudioCmd::Media(m) => media(m),
        AudioCmd::Run(c) => {
            let _ = Command::new("sh")
                .args(["-c", &c])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
    }
}

/// Controls the first MPRIS media player found on the session bus.
fn media(cmd: MediaCmd) {
    let Ok(out) = Command::new("busctl").args(["--user", "--no-pager", "list"]).output() else {
        return;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let Some(player) = text
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .find(|n| n.starts_with("org.mpris.MediaPlayer2.") && !n.contains("plasma-browser"))
        .or_else(|| {
            text.lines()
                .filter_map(|l| l.split_whitespace().next())
                .find(|n| n.starts_with("org.mpris.MediaPlayer2."))
        })
    else {
        return;
    };
    let method = match cmd {
        MediaCmd::PlayPause => "PlayPause",
        MediaCmd::Next => "Next",
        MediaCmd::Previous => "Previous",
    };
    let _ = Command::new("busctl")
        .args([
            "--user",
            "call",
            player,
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
            method,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
