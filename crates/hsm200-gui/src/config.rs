//! Persistent knob / button assignments, stored as JSON in `~/.config/hsm200/config.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::audio::Target;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ButtonAction {
    None,
    /// Toggle between LCD page 1 and 2 (knobs get a different set of assignments).
    NextPage,
    /// Mute / unmute whatever knob `n` (0..3) controls on the current page.
    MuteKnob(u8),
    MuteTarget(Target),
    PlayPause,
    NextTrack,
    PrevTrack,
    /// Run a shell command.
    Command(String),
}

impl ButtonAction {
    pub fn label(&self) -> String {
        match self {
            ButtonAction::None => "Nothing".into(),
            ButtonAction::NextPage => "Switch page".into(),
            ButtonAction::MuteKnob(n) => format!("Mute knob {}", n + 1),
            ButtonAction::MuteTarget(Target::System) => "Mute system".into(),
            ButtonAction::MuteTarget(Target::Mic) => "Mute microphone".into(),
            ButtonAction::MuteTarget(t) => format!("Mute {}", t.label()),
            ButtonAction::PlayPause => "Play / Pause".into(),
            ButtonAction::NextTrack => "Next track".into(),
            ButtonAction::PrevTrack => "Previous track".into(),
            ButtonAction::Command(c) if c.is_empty() => "Run command".into(),
            ButtonAction::Command(c) => format!("Run: {c}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnobBinding {
    pub target: Target,
    /// Volume change per encoder click, in percent.
    pub step_pct: f32,
    /// Action when the knob is pushed.
    pub push: ButtonAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// 8 entries: page 1 knobs 1-4, then page 2 knobs 1-4.
    pub knobs: Vec<KnobBinding>,
    /// The 4 lower action buttons.
    pub actions: Vec<ButtonAction>,
    /// Control buttons A..F.
    pub controls: Vec<ButtonAction>,
}

impl Default for Config {
    fn default() -> Self {
        let knob = |t: Target, i: u8| KnobBinding {
            target: t,
            step_pct: 1.0,
            push: ButtonAction::MuteKnob(i),
        };
        let app = |n: &str| Target::App(n.into());
        Self {
            knobs: vec![
                knob(Target::Mic, 0),
                knob(Target::System, 1),
                knob(app("Chrome"), 2),
                knob(app("Spotify"), 3),
                knob(app("Discord"), 0),
                knob(app("Firefox"), 1),
                knob(app("OBS"), 2),
                knob(Target::None, 3),
            ],
            actions: vec![
                ButtonAction::PlayPause,
                ButtonAction::NextTrack,
                ButtonAction::PrevTrack,
                ButtonAction::MuteTarget(Target::Mic),
            ],
            controls: vec![
                ButtonAction::NextPage,
                ButtonAction::None,
                ButtonAction::None,
                ButtonAction::None,
                ButtonAction::None,
                ButtonAction::None,
            ],
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("hsm200").join("config.json"))
    }

    pub fn load() -> Self {
        let mut cfg: Config = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Repair truncated / hand-edited files.
        let d = Config::default();
        cfg.knobs.resize(8, d.knobs[7].clone());
        cfg.actions.resize(4, ButtonAction::None);
        cfg.controls.resize(6, ButtonAction::None);
        cfg
    }

    pub fn save(&self) {
        if let Some(p) = Self::path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }
}
