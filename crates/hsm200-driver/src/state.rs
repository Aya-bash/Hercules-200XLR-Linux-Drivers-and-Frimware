use crate::error::{DriverError, Result};

/// Minimum expected payload length for a `0x0101` Poll Status response (256 bytes).
pub const INPUT_REPORT_LEN: usize = 256;

/// All 14 physical buttons on the Hercules Stream 200 XLR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    /// Lower Action Button 1 (`raw_buttons[2]` bit 0 = `0x01`).
    Action1,
    /// Lower Action Button 2 (`raw_buttons[2]` bit 1 = `0x02`).
    Action2,
    /// Lower Action Button 3 (`raw_buttons[2]` bit 2 = `0x04`).
    Action3,
    /// Lower Action Button 4 (`raw_buttons[2]` bit 3 = `0x08`).
    Action4,
    /// Rotary Encoder 1 Push (`raw_buttons[1]` bit 0 = `0x01`).
    Encoder1Push,
    /// Rotary Encoder 2 Push (`raw_buttons[1]` bit 1 = `0x02`).
    Encoder2Push,
    /// Rotary Encoder 3 Push (`raw_buttons[1]` bit 2 = `0x04`).
    Encoder3Push,
    /// Rotary Encoder 4 Push (`raw_buttons[1]` bit 3 = `0x08`).
    Encoder4Push,
    /// Control Button A (`raw_buttons[0]` bit 0 = `0x01` — Page / Top Control 1).
    ControlA,
    /// Control Button B (`raw_buttons[0]` bit 1 = `0x02` — Top/Side Control 2).
    ControlB,
    /// Control Button C (`raw_buttons[0]` bit 2 = `0x04` — Top/Side Control 3).
    ControlC,
    /// Control Button D (`raw_buttons[0]` bit 3 = `0x08` — Top/Side Control 4).
    ControlD,
    /// Control Button E (`raw_buttons[0]` bit 4 = `0x10` — Top/Side Control 5).
    ControlE,
    /// Control Button F (`raw_buttons[2]` bit 4 = `0x10` — Mix / Link Control 6).
    ControlF,
}

impl Button {
    /// All 14 buttons in canonical order.
    pub const ALL: [Button; 14] = [
        Button::Action1,
        Button::Action2,
        Button::Action3,
        Button::Action4,
        Button::Encoder1Push,
        Button::Encoder2Push,
        Button::Encoder3Push,
        Button::Encoder4Push,
        Button::ControlA,
        Button::ControlB,
        Button::ControlC,
        Button::ControlD,
        Button::ControlE,
        Button::ControlF,
    ];

    /// Returns `(byte_index, bit_mask)` within the 4-byte `raw_buttons` field (`+0x00..+0x03`).
    pub const fn mask(self) -> (usize, u8) {
        match self {
            // Lower 4 Action Buttons: byte +0x02 bits 0..3 (0001, 0010, 0100, 1000)
            Button::Action1 => (2, 0x01),
            Button::Action2 => (2, 0x02),
            Button::Action3 => (2, 0x04),
            Button::Action4 => (2, 0x08),
            // 4 Rotary Encoder Push Buttons: byte +0x01 bits 0..3
            Button::Encoder1Push => (1, 0x01),
            Button::Encoder2Push => (1, 0x02),
            Button::Encoder3Push => (1, 0x04),
            Button::Encoder4Push => (1, 0x08),
            // 6 Control Buttons: byte +0x00 bits 0..4 and byte +0x02 bit 4
            Button::ControlA => (0, 0x01),
            Button::ControlB => (0, 0x02),
            Button::ControlC => (0, 0x04),
            Button::ControlD => (0, 0x08),
            Button::ControlE => (0, 0x10),
            Button::ControlF => (2, 0x10),
        }
    }

    /// Returns the LED index (`0..13`) used by `0x0601` `solid_mask` / `blink_mask`.
    pub const fn led_index(self) -> u8 {
        match self {
            Button::ControlA => 0,
            Button::ControlB => 1,
            Button::ControlC => 2,
            Button::ControlD => 3,
            Button::ControlE => 4,
            Button::Encoder1Push => 5,
            Button::Encoder2Push => 6,
            Button::Encoder3Push => 7,
            Button::Encoder4Push => 8,
            Button::Action1 => 9,
            Button::Action2 => 10,
            Button::Action3 => 11,
            Button::Action4 => 12,
            Button::ControlF => 13,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Button::Action1 => "Action 1 (Lower Button 1)",
            Button::Action2 => "Action 2 (Lower Button 2)",
            Button::Action3 => "Action 3 (Lower Button 3)",
            Button::Action4 => "Action 4 (Lower Button 4)",
            Button::Encoder1Push => "Encoder 1 Push",
            Button::Encoder2Push => "Encoder 2 Push",
            Button::Encoder3Push => "Encoder 3 Push",
            Button::Encoder4Push => "Encoder 4 Push",
            Button::ControlA => "Control A (Byte0.Bit0)",
            Button::ControlB => "Control B (Byte0.Bit1)",
            Button::ControlC => "Control C (Byte0.Bit2)",
            Button::ControlD => "Control D (Byte0.Bit3)",
            Button::ControlE => "Control E (Byte0.Bit4)",
            Button::ControlF => "Control F (Byte2.Bit4)",
        }
    }
}

/// High-level hardware change event computed between two consecutive `InputReport` snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum HardwareEvent {
    ButtonPressed(Button),
    ButtonReleased(Button),
    EncoderTurned {
        /// Encoder index `1..=4`.
        encoder: u8,
        /// Signed step delta since last poll.
        delta: i16,
        /// New cumulative 16-bit counter value.
        counter: u16,
    },
    AuxEncoderChanged {
        old_value: u8,
        new_value: u8,
    },
    HwFlagsChanged {
        old_flags: u8,
        new_flags: u8,
    },
}

/// Parsed 256-byte input report returned by command `0x0101` (`PollStatus`).
#[derive(Debug, Clone, PartialEq)]
pub struct InputReport {
    /// Raw 4-byte button bitmask (`+0x00..+0x03`).
    pub raw_buttons: [u8; 4],
    /// Creator Mix channel faders (`+0x04..+0x23`, 8 floats).
    pub creator_faders: [f32; 8],
    /// Audience Mix channel faders (`+0x24..+0x43`, 8 floats).
    pub audience_faders: [f32; 8],
    /// Headphone / monitor output volume (`+0x44..+0x47`).
    pub headphone_volume: f32,
    /// 4 main rotary encoder 16-bit cumulative counters (`+0x48..+0x4F`).
    pub encoders: [u16; 4],
    /// Auxiliary / headphone rotary encoder value (`+0x50`).
    pub aux_encoder: u8,
    /// Hardware state flags (`+0x51`: bit 0 = 48V state, bit 1 = mic source, bit 2 = mute/link).
    pub hw_flags: u8,
    /// Device / DSP status word (`+0x58..+0x5B`).
    pub dsp_status: u32,
}

impl InputReport {
    /// Parses a 256-byte (or longer, or shorter >= 92 bytes) `0x0101` payload.
    pub fn from_payload(payload: &[u8]) -> Result<Self> {
        if payload.len() < 0x5C {
            return Err(DriverError::ShortResponse {
                expected: 0x5C,
                actual: payload.len(),
            });
        }

        let mut raw_buttons = [0u8; 4];
        raw_buttons.copy_from_slice(&payload[0x00..0x04]);

        let mut creator_faders = [0.0f32; 8];
        for (i, slot) in creator_faders.iter_mut().enumerate() {
            let off = 0x04 + i * 4;
            *slot = f32::from_le_bytes([
                payload[off],
                payload[off + 1],
                payload[off + 2],
                payload[off + 3],
            ]);
        }

        let mut audience_faders = [0.0f32; 8];
        for (i, slot) in audience_faders.iter_mut().enumerate() {
            let off = 0x24 + i * 4;
            *slot = f32::from_le_bytes([
                payload[off],
                payload[off + 1],
                payload[off + 2],
                payload[off + 3],
            ]);
        }

        let headphone_volume = f32::from_le_bytes([
            payload[0x44],
            payload[0x45],
            payload[0x46],
            payload[0x47],
        ]);

        let encoders = [
            u16::from_le_bytes([payload[0x48], payload[0x49]]),
            u16::from_le_bytes([payload[0x4A], payload[0x4B]]),
            u16::from_le_bytes([payload[0x4C], payload[0x4D]]),
            u16::from_le_bytes([payload[0x4E], payload[0x4F]]),
        ];

        let aux_encoder = payload[0x50];
        let hw_flags = payload[0x51];
        let dsp_status = u32::from_le_bytes([
            payload[0x58],
            payload[0x59],
            payload[0x5A],
            payload[0x5B],
        ]);

        Ok(Self {
            raw_buttons,
            creator_faders,
            audience_faders,
            headphone_volume,
            encoders,
            aux_encoder,
            hw_flags,
            dsp_status,
        })
    }

    /// Returns `true` if `button` is currently pressed.
    pub fn is_pressed(&self, button: Button) -> bool {
        let (byte_idx, mask) = button.mask();
        (self.raw_buttons[byte_idx] & mask) != 0
    }

    /// Returns a list of all currently pressed buttons.
    pub fn pressed_buttons(&self) -> Vec<Button> {
        Button::ALL
            .into_iter()
            .filter(|&b| self.is_pressed(b))
            .collect()
    }

    /// Returns the lower 4 action buttons as a 4-bit nibble (`0x0..=0xF`, i.e., `0000..=1111`).
    pub fn action_buttons_nibble(&self) -> u8 {
        self.raw_buttons[2] & 0x0F
    }

    /// Returns the 4 rotary encoder push buttons as a 4-bit nibble (`0x0..=0xF`).
    pub fn encoder_push_nibble(&self) -> u8 {
        self.raw_buttons[1] & 0x0F
    }

    /// Computes all button, encoder, and flag changes from `previous` to `self`.
    pub fn diff(&self, previous: &Self) -> Vec<HardwareEvent> {
        let mut events = Vec::new();

        for button in Button::ALL {
            let was = previous.is_pressed(button);
            let now = self.is_pressed(button);
            if !was && now {
                events.push(HardwareEvent::ButtonPressed(button));
            } else if was && !now {
                events.push(HardwareEvent::ButtonReleased(button));
            }
        }

        for i in 0..4 {
            let delta = encoder_delta(previous.encoders[i], self.encoders[i]);
            if delta != 0 {
                events.push(HardwareEvent::EncoderTurned {
                    encoder: (i + 1) as u8,
                    delta,
                    counter: self.encoders[i],
                });
            }
        }

        if self.aux_encoder != previous.aux_encoder {
            events.push(HardwareEvent::AuxEncoderChanged {
                old_value: previous.aux_encoder,
                new_value: self.aux_encoder,
            });
        }

        if self.hw_flags != previous.hw_flags {
            events.push(HardwareEvent::HwFlagsChanged {
                old_flags: previous.hw_flags,
                new_flags: self.hw_flags,
            });
        }

        events
    }
}

/// Counter span the encoders wrap at. Hardware counters run `0..=65533/65534`, i.e. they
/// wrap just short of a full `u16`, so plain `wrapping_sub` is off by 1-2 clicks at the seam.
const ENCODER_WRAP: i32 = 65535;

/// Signed click delta between two encoder counter readings, handling wrap-around at 0.
/// A real turn never moves more than a few hundred counts per 20 ms poll, so any jump
/// larger than half the range must be the counter crossing zero.
pub fn encoder_delta(old: u16, new: u16) -> i16 {
    let mut d = i32::from(new) - i32::from(old);
    if d > ENCODER_WRAP / 2 {
        d -= ENCODER_WRAP;
    } else if d < -(ENCODER_WRAP / 2) {
        d += ENCODER_WRAP;
    }
    d.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod encoder_tests {
    use super::encoder_delta;

    #[test]
    fn plain_and_wrap() {
        assert_eq!(encoder_delta(100, 103), 3);
        assert_eq!(encoder_delta(103, 100), -3);
        assert_eq!(encoder_delta(0, 65534), -1);
        assert_eq!(encoder_delta(65534, 0), 1);
        assert_eq!(encoder_delta(1, 65534), -2);
    }
}
