# Hercules Stream 200 XLR — Linux Drivers, Control Software & Firmware Docs

Reverse-engineered USB protocol documentation, Rust user-space Linux driver (`hsm200-driver`), and CLI/daemon control software (`hsm200-control`) for the **Hercules Stream 200 XLR** (`HSM02`, USB ID `06f8:e054`).

---

## Repository Structure

- [`docs/usb-protocol.md`](docs/usb-protocol.md) — Complete USB Bulk vendor protocol specification (`0x0101`–`0x0A01`), button bitmasks, rotary encoders, AK7735EQ DSP blocks, audio mixer routing, LEDs, and `480x272` LCD framebuffer protocol.
- [`udev/99-hercules-stream-200xlr.rules`](udev/99-hercules-stream-200xlr.rules) — Linux `udev` rules granting non-root USB access to `06f8:e054` (Runtime) and `06f8:e055` (DFU Bootloader).
- [`crates/hsm200-driver`](crates/hsm200-driver) — Rust library crate implementing the synchronous USB Bulk transport (`Interface 3`, `AltSetting 1`, `EP 0x01 OUT` / `EP 0x81 IN`) and high-level hardware state APIs.
- [`crates/hsm200-control`](crates/hsm200-control) — Rust CLI & live event monitor binary (`hsm200-control`).
- [`crates/hsm200-gui`](crates/hsm200-gui) — Modern Rust desktop companion GUI (`hsm200-gui`) featuring deep black/gray elevated surfaces, sunken fader tracks, and `#FF0080` neon pink/purple studio aesthetic.

---

## 1. Install `udev` Rules (Non-Root USB Access)

Copy the `udev` rules file and reload `udev` so your user can access the Stream 200 XLR without `sudo`:

```bash
sudo cp udev/99-hercules-stream-200xlr.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

---

## 2. Build the Rust Driver & Control CLI

```bash
cargo build --release
```

The compiled binary will be located at `./target/release/hsm200-control`.

---

## 3. Usage (`hsm200-control`)

### Probe Device Info & Live Hardware Snapshot (`0x0101` / `0x0A01`)
```bash
./target/release/hsm200-control info
```

### Live Button & Rotary Encoder Event Monitor (`0x0101`)
```bash
./target/release/hsm200-control monitor --raw
```

### Audio Mixer, +48V Phantom Power & Mic Source (`0x0401`)
```bash
# Enable +48V Phantom Power and select rear XLR input
./target/release/hsm200-control audio --phantom-48v on --mic-source xlr

# Set Channel 1 Creator Mix to 0.0 dB and Audience Mix to -6.0 dB
./target/release/hsm200-control audio --channel 1 --creator-db 0.0 --audience-db -6.0
```

### AK7735EQ Microphone DSP Presets & Tuning (`0x0301`)
```bash
# Apply official "Radio Dark" DSP preset
./target/release/hsm200-control dsp --preset radio-dark

# Apply "Default" preset with custom 14 dB preamp gain and -55 dB noise gate
./target/release/hsm200-control dsp --preset default --preamp-db 14.0 --gate-threshold-db -55.0
```

### Button LEDs, LCD Brightness & RGB Meters (`0x0501` / `0x0601`)
```bash
# Set LCD backlight brightness to 90% and blink Action Button 1
./target/release/hsm200-control led --brightness 90 --button action1 --mode blink

# Set all button LEDs solid on and channel meter color to cyan
./target/release/hsm200-control led --button all --mode solid --meter-color 00FFFF
```

### 480x272 LCD Display Control & Custom Image Upload (`0x0801`)
```bash
# Upload a 480x272 color-bars test pattern to the LCD
./target/release/hsm200-control display --test-pattern

# Upload any PNG/JPEG/BMP image (auto-resized to 480x272 RGB565)
./target/release/hsm200-control display --image /path/to/wallpaper.png

# Switch between Mixer Page 1 and Page 2, or sleep/wake the screen
./target/release/hsm200-control display --page 2
./target/release/hsm200-control display --power off
```

---

## 4. Modern Desktop Companion GUI (`hsm200-gui`)

The repository includes a modern hardware companion GUI written in Rust (`eframe`/`egui`) with a custom `#FF0080` (Electric Neon Pink / Magenta) dark studio theme:

```bash
./target/release/hsm200-gui
```

### Features
- **Dual-Bus Studio Audio Mixer**: 4-channel strip faders with Creator and Audience mix buses, link/split mode, instant mute toggles, and dedicated headphone monitor bus.
- **AK7735EQ Studio Vocal DSP Processor**: Full control of the onboard hardware audio DSP, including analog preamp gain (+0..65 dB), noise gate, 3-band parametric EQ spline visualizer, compressor, and peak limiter. Quick switching between all 8 hardware presets.
- **Hardware Lighting & RGB Meters**: Per-button backlight modes (`Solid`, `Blink`, `Off`) for lower action buttons, encoder buttons, and control buttons; LCD brightness control; and 3-point gradient color customizer for channel VU meters.
- **480x272 Virtual LCD Display**: Live virtual LCD screen preview, instant page switching (Page 1 / Page 2), built-in test pattern and solid color fill generators, and arbitrary PNG/JPEG wallpaper uploading with automatic scaling and paletting.
- **Live Hardware Event Inspector**: Real-time 50 Hz USB telemetry feed, action button bitmask inspector, 16-bit encoder delta tracking, and hardware flag monitor.

