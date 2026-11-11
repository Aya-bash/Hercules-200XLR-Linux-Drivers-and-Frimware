# Hercules Stream 200 XLR (`HSM02`) — USB Protocol Specification

This document details the USB communication protocol for the **Hercules Stream 200 XLR** (`HSM02`), reverse-engineered from the official Windows SDK (`hsm_api_core_x64.dll` v2.3.8 / `2025_HSM_1`).

---

## 1. Hardware & USB Overview

### 1.1 Core Hardware Components
* **MCU**: NXP i.MX RT1062 (`MIMXRT1062DVJ6B` — ARM Cortex-M7 @ 600 MHz, High-Speed USB 2.0 OTG)
* **Audio DSP / Codec**: Asahi Kasei `AK7735EQ` (24-bit Audio DSP with integrated ADC/DAC)
* **External Flash**: Macronix `MX25L3233FM1I-08G` (32 Mbit / 4 MB SPI NOR Flash)
* **LCD Display**: Winstar `WF43WSWAEDNN0` (4.3" TFT LCD, **480 × 272** resolution)

### 1.2 USB Vendor & Product IDs
| VID | PID | Internal Code | Mode |
| :--- | :--- | :--- | :--- |
| `0x06F8` (Guillemot/Hercules) | `0xE054` | `HSM02` | **Hercules Stream 200 XLR (Runtime Mode)** |
| `0x06F8` (Guillemot/Hercules) | `0xE055` | `HSM02` | **Hercules Stream 200 XLR (DFU Bootloader Mode)** |
| `0x06F8` (Guillemot/Hercules) | `0xE053` | `HSM01` | Hercules Stream 100 (STM32L4R7-based) |

### 1.3 USB Composite Interfaces (`06f8:e054`)
1. **Interfaces 0–2 (`MI_00`) — USB Audio Class 2.0 (UAC2)**:
   * Grouped via an Interface Association Descriptor (`bFirstInterface = 0`, `bInterfaceCount = 3`):
     * **Interface 0**: AudioControl (`EP 0x82 IN` Interrupt)
     * **Interface 1**: AudioStreaming Capture (`EP 0x83 IN` Isochronous, 10 channels, 24-bit PCM in 32-bit subslots)
     * **Interface 2**: AudioStreaming Playback (`EP 0x02 OUT` Isochronous + `EP 0x84 IN` Feedback, 10 channels, 24-bit PCM in 32-bit subslots)
   * Compatible with the standard Linux `snd-usb-audio` kernel driver.
   * **Playback (10 channels / 5 stereo pairs)**:
     * `Ch 0–1`: System
     * `Ch 2–3`: Game
     * `Ch 4–5`: Music
     * `Ch 6–7`: Chat
     * `Ch 8–9`: Aux
   * **Capture (10 channels / 5 stereo pairs)**:
     * `Ch 0–1`: Audience Mix
     * `Ch 2–3`: Creator Mix
     * `Ch 4–5`: Microphone
     * `Ch 6–7`: Line In
     * `Ch 8–9`: Aux
2. **Interface 3 (`MI_02`) — Bulk Vendor Control Interface (`bInterfaceClass = 0xFF`)**:
   * **Important**: `bAlternateSetting = 0` has **0 endpoints** (`bNumEndpoints = 0`). The driver **must** select **`bAlternateSetting = 1`** (`libusb_set_interface_alt_setting(handle, 3, 1)`) after claiming Interface `3`.
   * Note: On Windows, the composite driver groups Interfaces 0–2 as Function 0 (`MI_00`), so `TLUSBAPI_ClaimInterface(hDevice, 1)` claims the second composite function (which is USB `bInterfaceNumber = 3`).
   * Under `bAlternateSetting = 1`, it exposes two High-Speed USB Bulk endpoints (`wMaxPacketSize = 512` bytes):
     * **`EP 0x01 OUT`** (Host $\to$ Device)
     * **`EP 0x81 IN`** (Device $\to$ Host)
   * **EP0 Vendor-to-Interface Handshake (`bmRequestType = 0xC1`, `wIndex = 3`)**:
     Immediately after selecting `bAlternateSetting = 1` on Interface `3`, the host issues three EP0 Control IN transfers (`USB_TYPE_VENDOR | USB_RECIP_INTERFACE | USB_ENDPOINT_IN = 0xC1`, `wIndex = 3`):
     1. `bRequest = 0x10`, `wValue = 0x0000`, `wIndex = 3`, `wLength = 2` $\to$ returns `uint16_le` protocol readiness (`0x0001`).
     2. `bRequest = 0x01`, `wValue = 0x0000`, `wIndex = 3`, `wLength = 4` $\to$ returns `uint32_le` maximum Bulk IN (`EP 0x81`) packet size (`0x00000800` = `2048` bytes).
     3. `bRequest = 0x01`, `wValue = 0x0001`, `wIndex = 3`, `wLength = 4` $\to$ returns `uint32_le` maximum Bulk OUT (`EP 0x01`) packet size (`0x00000800` = `2048` bytes).
   * Every control transaction is synchronous: the host sends a command packet on **Bulk OUT (`0x01`)** and immediately reads the response packet on **Bulk IN (`0x81`)** (default timeout: `5000 ms`).
   * If an outbound packet size is an exact multiple of `wMaxPacketSize` (`512` bytes) and smaller than `tx_max_transfer_size` (`2048` bytes), a Zero-Length Packet (ZLP) is sent before reading the response.
3. **Interface 4 — Runtime Device Firmware Update (DFU 1.10)**:
   * `bInterfaceClass = 0xFE` (Application Specific), `bInterfaceSubClass = 0x01` (DFU), `wTransferSize = 4096` bytes, `wDetachTimeout = 2000` ms.

---

## 2. Common 12-Byte Packet Header

All Bulk OUT commands and Bulk IN responses on the Stream 200 XLR begin with a **12-byte (`0x0C`) little-endian header**:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        total_size (u32)                       | +0x00
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          cmd_id (u16)         |    seq (u8)   | reserved (u8) | +0x04
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          status (u32)                         | +0x08
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        payload (variable)                     | +0x0C
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Offset | Type | Name | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `total_size` | Total packet length in bytes, **including** this 12-byte header. |
| `+0x04` | `uint16_le` | `cmd_id` | Command Opcode (`0x0101`..`0x0A01` in Bulk OUT requests; **`cmd_id \| 0x8000`** in Bulk IN responses, e.g., `0x8101`). |
| `+0x06` | `uint8` | `seq` | Sequence counter (`0x01`..`0xFF`). Increments by 1 per command and wraps from `0xFF` to `0x01` (never `0x00`). |
| `+0x07` | `uint8` | `reserved` | Always `0x00`. |
| `+0x08` | `uint32_le` | `status` | Always `0x00000000` in host requests and `0x00000000` in successful device responses. |

---

## 3. Command `0x0101` — Poll Device Status & Input Report

The host polls hardware inputs (buttons, rotary encoders, and DSP fader levels) by sending a 12-byte `0x0101` command.

### 3.1 Request (`12` bytes)
| Offset | Type | Value | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `0x0000000C` | Total size = 12 bytes |
| `+0x04` | `uint16_le` | `0x0101` | Poll Status opcode |
| `+0x06` | `uint8` | `seq` | Sequence counter (`1..255`) |
| `+0x07` | `uint8` | `0x00` | Reserved |
| `+0x08` | `uint32_le` | `0x00000000` | Reserved |

### 3.2 Response (`104` bytes: `12`-byte header + `92`-byte payload)
Bytes `+0x00..+0x0B` contain the 12-byte response header (`total_size = 0x00000068`, `cmd_id = 0x8101`), followed by a **92-byte (`0x5C`) status payload** starting at packet offset `+0x0C` (payload offset `+0x00`):

| Payload Offset | Packet Offset | Type | Field | Description |
| :--- | :--- | :--- | :--- | :--- |
| `+0x00..+0x03` | `+0x0C..+0x0F` | `uint32_le` | `raw_buttons` | **Raw Hardware Button Bitmask** (14 buttons; see Section 3.3). |
| `+0x04..+0x23` | `+0x10..+0x2F` | `float32[8]` | `creator_faders` | **Creator Mix Fader Levels** (Channels 0–7, raw `0.0`–`10000.0`; divide by `100.0` for `0.0%`–`100.0%`). |
| `+0x24..+0x43` | `+0x30..+0x4F` | `float32[8]` | `audience_faders` | **Audience Mix Fader Levels** (Channels 0–7, raw `0.0`–`10000.0`; divide by `100.0` for `0.0%`–`100.0%`). |
| `+0x44..+0x47` | `+0x50..+0x53` | `float32` | `monitor_volume` | **Headphone / Monitor Volume Level** (raw `0.0`–`10000.0`; divide by `100.0` for `0.0%`–`100.0%`). |
| `+0x48..+0x49` | `+0x54..+0x55` | `uint16_le` | `encoder[0]` | **Rotary Encoder 1** counter/value. |
| `+0x4A..+0x4B` | `+0x56..+0x57` | `uint16_le` | `encoder[1]` | **Rotary Encoder 2** counter/value. |
| `+0x4C..+0x4D` | `+0x58..+0x59` | `uint16_le` | `encoder[2]` | **Rotary Encoder 3** counter/value. |
| `+0x4E..+0x4F` | `+0x5A..+0x5B` | `uint16_le` | `encoder[3]` | **Rotary Encoder 4** counter/value. |
| `+0x50` | `+0x5C` | `uint8` | `aux_encoder` | **Auxiliary Encoder / Brightness dial** (SDK applies `val ^ 0x80`). |
| `+0x51..+0x53` | `+0x5D..+0x5F` | `uint8[3]` | `padding` | Unused / padding. |
| `+0x54..+0x57` | `+0x60..+0x63` | `uint32_le` | `hw_flags` | **Hardware Status Flags** (masked with `0xFF01`: 48V phantom power, input source, mute state). |
| `+0x58..+0x5B` | `+0x64..+0x67` | `uint32_le` | `dsp_status` | **Firmware / DSP State Word**. |

### 3.3 Button Bitmask Layout (`raw_buttons` at Payload `+0x00..+0x03`)
Each physical button is represented as a single bit (`1` = pressed, `0` = released) in the 32-bit `raw_buttons` word. Multiple simultaneous button presses bitwise-OR their bits together.

* **Byte `+0x02` (Bits 16–20)** holds the **Lower 4 Action Buttons** in bits `0..3` (`0x01`, `0x02`, `0x04`, `0x08` — i.e., `0001`, `0010`, `0100`, `1000` in binary) plus Control Button F in bit `4` (`0x10`).
* **Byte `+0x01` (Bits 8–11)** holds the **4 Rotary Encoder Push Buttons** in bits `0..3` (`0x01`, `0x02`, `0x04`, `0x08`).
* **Byte `+0x00` (Bits 0–4)** holds the remaining **5 Control Buttons** in bits `0..4` (`0x01`, `0x02`, `0x04`, `0x08`, `0x10`).

| Button Group | Physical Button | Payload Byte | Bit in Byte | Binary in Byte | Raw `uint32_le` Mask | SDK Normalized Bitmask |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Lower 4 Action Buttons** | **Action Button 1** | `+0x02` | Bit 0 | `00000001` (`0x01`) | `0x00010000` (Bit 16) | `0x0001` (Bit 0) |
| **Lower 4 Action Buttons** | **Action Button 2** | `+0x02` | Bit 1 | `00000010` (`0x02`) | `0x00020000` (Bit 17) | `0x0002` (Bit 1) |
| **Lower 4 Action Buttons** | **Action Button 3** | `+0x02` | Bit 2 | `00000100` (`0x04`) | `0x00040000` (Bit 18) | `0x0004` (Bit 2) |
| **Lower 4 Action Buttons** | **Action Button 4** | `+0x02` | Bit 3 | `00001000` (`0x08`) | `0x00080000` (Bit 19) | `0x0008` (Bit 3) |
| **4 Encoder Push Buttons** | **Encoder 1 Push** | `+0x01` | Bit 0 | `00000001` (`0x01`) | `0x00000100` (Bit 8) | `0x0010` (Bit 4) |
| **4 Encoder Push Buttons** | **Encoder 2 Push** | `+0x01` | Bit 1 | `00000010` (`0x02`) | `0x00000200` (Bit 9) | `0x0020` (Bit 5) |
| **4 Encoder Push Buttons** | **Encoder 3 Push** | `+0x01` | Bit 2 | `00000100` (`0x04`) | `0x00000400` (Bit 10) | `0x0040` (Bit 6) |
| **4 Encoder Push Buttons** | **Encoder 4 Push** | `+0x01` | Bit 3 | `00001000` (`0x08`) | `0x00000800` (Bit 11) | `0x0080` (Bit 7) |
| **Top/Side Control Buttons** | **Control Button A** | `+0x00` | Bit 0 | `00000001` (`0x01`) | `0x00000001` (Bit 0) | `0x0100` (Bit 8) |
| **Top/Side Control Buttons** | **Control Button B** | `+0x00` | Bit 3 | `00001000` (`0x08`) | `0x00000008` (Bit 3) | `0x0200` (Bit 9) |
| **Top/Side Control Buttons** | **Control Button C** | `+0x00` | Bit 1 | `00000010` (`0x02`) | `0x00000002` (Bit 1) | `0x0400` (Bit 10) |
| **Top/Side Control Buttons** | **Control Button D** | `+0x00` | Bit 2 | `00000100` (`0x04`) | `0x00000004` (Bit 2) | `0x0800` (Bit 11) |
| **Top/Side Control Buttons** | **Control Button E** | `+0x00` | Bit 4 | `00010000` (`0x10`) | `0x00000010` (Bit 4) | `0x1000` (Bit 12) |
| **Top/Side Control Buttons** | **Control Button F** | `+0x02` | Bit 4 | `00010000` (`0x10`) | `0x00100000` (Bit 20) | `0x2000` (Bit 13) |

---

## 4. Command `0x0301` — Microphone DSP Configuration (`AK7735EQ`)

Configures the onboard `AK7735EQ` DSP voice processing chain (Preamp, High-Pass Filter, Noise Gate, Compressor, De-Esser, and Equalizer). Sent as up to **12 packets** (`sub_idx = 0..11`) of **88 bytes (`0x58`)** each.

### 4.1 Packet Layout (`88` bytes)
| Packet Offset | Type | Field | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `total_size` | `0x00000058` (88 bytes) |
| `+0x04` | `uint16_le` | `cmd_id` | `0x0301` |
| `+0x06` | `uint8` | `seq` | Sequence counter (`1..255`) |
| `+0x07` | `uint8` | `reserved` | `0x00` |
| `+0x08` | `uint32_le` | `status` | `0x00000000` |
| `+0x0C` | `uint32_le` | `enable_mask` | `1 << sub_idx` (e.g., `0x001`..`0x800`) |
| `+0x10..+0x57` | `uint8[72]` | `dsp_params` | Sub-block specific `float32` / `uint32_le` parameters (see below) |

### 4.2 Sub-Block Parameter Offsets (`sub_idx = 0..11`)
| `sub_idx` | `enable_mask` (`+0x0C`) | Active Parameter Offsets in Packet |
| :--- | :--- | :--- |
| `0` | `0x0001` | `+0x10`: `u32` enable (`0`/`1`), `+0x14`: `float32` param |
| `1` | `0x0002` | `+0x18`: `float32` param |
| `2` | `0x0004` | `+0x1C`: `u32` enable, `+0x20`: `float32`, `+0x24`: `float32`, `+0x28`: `u32`, `+0x2C`: `u32` |
| `3` | `0x0008` | `+0x30`: `u32` enable, `+0x34`: `float32`, `+0x38`: `float32`, `+0x3C`: `u32`, `+0x40`: `u32`, `+0x44`: `float32` |
| `4` | `0x0010` | `+0x48`: `u32` enable, `+0x4C`: `float32`, `+0x50`: `float32`, `+0x54`: `u32` |
| `5..9` | `0x0020..0x0200` | EQ Bands 0–4 (`band = sub_idx - 5`):<br>• `+0x10`: `u32` band index (`0..4`)<br>• `+0x14`: `u32` filter type<br>• `+0x18`: `u32` enable (`0`/`1`)<br>• `+0x1C`: `float32` gain (dB)<br>• `+0x20`: `u32` frequency (Hz)<br>• `+0x24`: `float32` Q-factor |
| `10` | `0x0400` | `+0x28`: `u32` enable (`0`/`1`), `+0x2C`: `float32` param |
| `11` | `0x0800` | `+0x30`: `u32` enable (`0`/`1`) |

---

## 5. Command `0x0401` — Audio Mixer, Routing & Hardware Input Config

Controls hardware microphone settings (48V phantom power, input source/gain), channel fader volumes (Creator & Audience mixes), channel mutes, and host sync lock.

### 5.1 Packet Layout (`40` bytes)
| Packet Offset | Type | Field | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `total_size` | `0x00000028` (40 bytes) |
| `+0x04` | `uint16_le` | `cmd_id` | `0x0401` |
| `+0x06` | `uint8` | `seq` | Sequence counter (`1..255`) |
| `+0x07` | `uint8` | `reserved` | `0x00` |
| `+0x08` | `uint32_le` | `status` | `0x00000000` |
| `+0x0C` | `uint32_le` | `update_type` | Bitmask of updated sections: `0x01` (Hardware Config), `0x02` (Fader Gain), `0x04` (Channel Mute/Routing). |
| `+0x10` | `uint32_le` | `hw_mask` | Bitmask of modified hardware flags (when `update_type & 0x01`):<br>• `0x00000002`: 48V Phantom Power<br>• `0x00000078`: Mic input source / preamp mode (`0x08`, `0x10`, `0x20`, `0x40`)<br>• `0x00000080`: Mic Mute<br>• `0x80000000`: Host Sync / Lock state |
| `+0x14` | `uint32_le` | `hw_value` | Active hardware flag bits corresponding to `hw_mask`. |
| `+0x18` | `uint32_le` | `vol_ch_mask` | Channel bitmask `1 << ch_idx` (when `update_type & 0x02`).<br>• Bits `0..7`: Creator Mix channels 0–7<br>• Bits `8..15`: Audience Mix channels 0–7<br>• Bits `16..22`: Aux / Monitor outputs |
| `+0x1C` | `float32` | `vol_db` | Channel volume in dB (`20.0 * log10(percent / 100.0)`, or `-144.0f` when `<= 0`). |
| `+0x20` | `uint32_le` | `mute_ch_mask`| Channel bitmask `1 << ch_idx` (when `update_type & 0x04`). |
| `+0x24` | `uint32_le` | `mute_value` | `1 << ch_idx` if active/muted, `0` otherwise. |

---

## 6. Command `0x0501` — RGB LEDs, Channel Themes & Real-Time VU Meters

Controls the 4-channel RGB LED rings/meters, channel mute LED colors, and real-time software VU meter levels.

### 6.1 Packet Layout (`72` bytes)
| Packet Offset | Type | Field | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `total_size` | `0x00000048` (72 bytes) |
| `+0x04` | `uint16_le` | `cmd_id` | `0x0501` |
| `+0x06` | `uint8` | `seq` | Sequence counter (`1..255`) |
| `+0x07` | `uint8` | `reserved` | `0x00` |
| `+0x08` | `uint32_le` | `status` | `0x00000000` |
| `+0x0C` | `uint32_le` | `sub_cmd` | Subcommand selector (see Section 6.2). |
| `+0x10` | `uint32_le` | `channel_idx` | Target channel index (`0..3` or `0..7`). |
| `+0x14..+0x1B` | `uint8[8]` | `led_states` | Used when `sub_cmd == 0x02`: 8 state bytes for channel/button indicators. |
| `+0x28` | `uint32_le` | `sub_target` | Sub-target / stereo side (`1` = Left/Primary, `2` = Right/Secondary) or style flags. |
| `+0x2C..+0x3B` | `uint32_le[4]`| `rgb_colors` | 4 × 24-bit RGB888 colors (`0x00RRGGBB`) for VU meter gradient / LED zones. |
| `+0x3C` | `uint32_le` | `mute_param1` | Used when `sub_cmd == 0x20`. |
| `+0x40` | `uint32_le` | `mute_param2` | Used when `sub_cmd == 0x20`. |
| `+0x44` | `uint32_le` | `vu_level` | Used when `sub_cmd == 0x40`: Real-time VU meter level (`0..10000`). |

### 6.2 `0x0501` Subcommands (`+0x0C`)
* `0x02`: Set 8 channel/indicator state bytes at `+0x14..+0x1B`.
* `0x04` / `0x08` / `0x10` (or `0x18`): Set RGB888 color palette (`+0x2C..+0x3B`) and style flags (`+0x28`) for channel `channel_idx` (`0..3`).
* `0x20`: Update channel mute indicator parameters (`+0x3C`, `+0x40`) for `channel_idx` (`0..3`), side `sub_target` (`1` or `2`).
* `0x40`: Push real-time VU meter level (`+0x44`, scaled `0..10000`) for `channel_idx` (`0..7`), channel side `sub_target` (`1` = L, `2` = R).

---

## 7. Command `0x0601` — Button Backlights & LCD Brightness

Controls the on/off/blink states of the physical button backlights and the LCD screen backlight brightness.

### 7.1 Packet Layout (`32` bytes)
| Packet Offset | Type | Field | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `total_size` | `0x00000020` (32 bytes) |
| `+0x04` | `uint16_le` | `cmd_id` | `0x0601` |
| `+0x06` | `uint8` | `seq` | Sequence counter (`1..255`) |
| `+0x07` | `uint8` | `reserved` | `0x00` |
| `+0x08` | `uint32_le` | `status` | `0x00000000` |
| `+0x0C` | `uint32_le` | `sub_cmd` | `0x01` = Button Backlights, `0x02` = LCD Backlight Brightness |
| `+0x10` | `uint32_le` | `btn_mask` | (`sub_cmd == 0x01`) Bitmask of buttons being updated |
| `+0x14` | `uint32_le` | `btn_solid` | (`sub_cmd == 0x01`) Bitmask of buttons lit solid (`on & ~blink`) |
| `+0x18` | `uint32_le` | `btn_blink` | (`sub_cmd == 0x01`) Bitmask of buttons blinking |
| `+0x1C` | `uint32_le` | `brightness` | (`sub_cmd == 0x02`) LCD Backlight Brightness (`0x00`–`0xFF`) |

---

## 8. Command `0x0701` — UI Icon & Widget Graphic Transfer

Uploads UI icons, channel labels, and volume meter graphics to the device in chunked Bulk OUT packets (up to `0x3D0` = 976 bytes per packet).

### 8.1 Packet Layout (`32`-byte header + up to `944` bytes payload)
| Packet Offset | Type | Field | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `total_size` | `0x20 + chunk_size` (max `0x3D0` = 976 bytes) |
| `+0x04` | `uint16_le` | `cmd_id` | `0x0701` |
| `+0x06` | `uint8` | `seq` | Sequence counter (`1..255`) |
| `+0x07` | `uint8` | `reserved` | `0x00` |
| `+0x08` | `uint32_le` | `status` | `0x00000000` |
| `+0x0C` | `uint32_le` | `element_type`| UI element category (`0x01`..`0x08` for 4-bit paletted icons; `0x09`..`0x0A` for 8-bit paletted widgets) |
| `+0x10` | `uint32_le` | `slot_index` | Target UI slot (`0..7`) |
| `+0x14` | `uint32_le` | `byte_offset` | Current byte offset in the graphic buffer (`0`, `944`, `1888`, ...) |
| `+0x18` | `uint32_le` | `bytes_rem` | Remaining bytes after this chunk (starts at total graphic size, e.g. `0x800`, and counts down to `0`) |
| `+0x1C` | `uint32_le` | `chunk_size` | Payload byte count in this packet (up to `0x3B0` = 944 bytes) |
| `+0x20..` | `uint8[]` | `chunk_data` | Paletted image chunk: starts with BGRA8888 color lookup table (16 × 4B = 64B for 4-bit, or 256 × 4B = 1024B for 8-bit) followed by packed 4-bit or 8-bit pixel indices. |

---

## 9. Command `0x0801` — Full-Screen Framebuffer & Display Control

Controls screen power state (wake/sleep), page switching, UI layer visibility, and full-screen background transfers (`480 × 272` LCD).

### 9.1 Packet Layout (`40`-byte header + optional chunk payload)
| Packet Offset | Type | Field | Description |
| :--- | :--- | :--- | :--- |
| `+0x00` | `uint32_le` | `total_size` | `0x28 + chunk_size` (`0x28` = 40 bytes when `chunk_size == 0`) |
| `+0x04` | `uint16_le` | `cmd_id` | `0x0801` |
| `+0x06` | `uint8` | `seq` | Sequence counter (`1..255`) |
| `+0x07` | `uint8` | `reserved` | `0x00` |
| `+0x08` | `uint32_le` | `status` | `0x00000000` |
| `+0x0C` | `uint32_le` | `sub_cmd` | Display operation code (see Section 9.2) |
| `+0x10` | `uint32_le` | `param0` | Reserved / `0` |
| `+0x14` | `uint32_le` | `byte_offset` | Byte offset for multi-packet framebuffer transfers |
| `+0x18` | `uint32_le` | `bytes_rem` | Remaining bytes after current chunk |
| `+0x1C` | `uint32_le` | `chunk_size` | Payload byte count in this packet (`0` for non-transfer commands) |
| `+0x20` | `uint32_le` | `commit_arg` | Used when `sub_cmd == 0x23` (background slot ID) |
| `+0x24` | `uint32_le` | `page_arg` | Used when `sub_cmd` has `0x10` or `0x200` set |
| `+0x28..` | `uint8[]` | `chunk_data` | Optional image chunk payload |

### 9.2 `0x0801` Subcommands (`+0x0C`)
| `sub_cmd` | Total Transfer Size | Description |
| :--- | :--- | :--- |
| `0x0001` | `40` bytes (`chunk_size = 0`) | **Screen Wake / Enable Display** |
| `0x0003` | `40` bytes (`chunk_size = 0`) | **Screen Sleep / Blank Display** |
| `0x0005` | `0x3FC00` (`261,120` bytes) | **Full-Screen Raw RGB565 Framebuffer Transfer**: `480 × 272 × 2` bytes streamed in chunks at `+0x28`. |
| `0x0009` | `0x20200` (`131,584` bytes) | **Full-Screen 8-Bit Paletted Background Transfer**: `1024`-byte BGRA8888 palette (256 × 4 bytes) + `130,560` bytes (`480 × 272` 8-bit pixel indices), followed by a 40-byte commit packet with `sub_cmd = 0x0023`. |
| `0x0023` | `40` bytes (`chunk_size = 0`) | **Commit Background Transfer**: Finalizes a `0x0009` transfer (`+0x20` = slot/preset ID). |
| `0x0010` / `0x0200` | `40` bytes (`chunk_size = 0`) | **Switch Active UI Page / Layout** (`+0x24` = page index). |
| `0x0080` / `0x0100` | `40` bytes (`chunk_size = 0`) | **Toggle UI Overlay / Layer Visibility**. |

---

## 10. Commands `0x0901` & `0x0A01` — Device Preset & Status Query

### 10.1 Command `0x0901` (`20` bytes)
* `+0x00`: `0x00000014` (20 bytes)
* `+0x04`: `0x0901`
* `+0x06`: `seq` (`1..255`)
* `+0x0C`: `0x00000003` (`uint32_le`)
* `+0x10`: `param` (`uint32_le`, 8-bit preset/mode value)

### 10.2 Command `0x0A01` (`12` bytes)
* `+0x00`: `0x0000000C` (12 bytes)
* `+0x04`: `0x0A01`
* `+0x06`: `seq` (`1..255`)
* Reads a 1-byte device status/version value at `response[+0x0C]`.
