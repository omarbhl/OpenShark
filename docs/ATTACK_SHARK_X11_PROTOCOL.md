# Attack Shark X11 — Complete USB HID Protocol Reference

> Reverse-engineered from the Windows vendor software via Wireshark/USBPcap.
> Source: [HarukaYamamoto0/attack-shark-x11-driver](https://github.com/HarukaYamamoto0/attack-shark-x11-driver)
> This document is the single authoritative reference for implementing any feature of the X11 protocol.

---

## Table of Contents

1. [Device Identity](#1-device-identity)
2. [USB Interface Map](#2-usb-interface-map)
3. [Transfer Mechanics](#3-transfer-mechanics)
4. [Battery Monitoring — Interrupt IN](#4-battery-monitoring--interrupt-in)
5. [Report 0x04 — DPI Configuration](#5-report-0x04--dpi-configuration)
6. [Report 0x05 — User Preferences (LED / Sleep / Debounce)](#6-report-0x05--user-preferences-led--sleep--debounce)
7. [Report 0x06 — Polling Rate](#7-report-0x06--polling-rate)
8. [Report 0x08 — Button Macros](#8-report-0x08--button-macros)
9. [Report 0x0C — Internal State Reset](#9-report-0x0c--internal-state-reset)
10. [DPI Lookup Table](#10-dpi-lookup-table)
11. [Full Reconfiguration Sequence](#11-full-reconfiguration-sequence)
12. [Safety Rules and Recovery](#12-safety-rules-and-recovery)
13. [Linux udev Setup](#13-linux-udev-setup)

---

## 1. Device Identity

| Property       | Wired (USB cable)      | Wireless (2.4G dongle) |
|----------------|------------------------|------------------------|
| `idVendor`     | `0x1D57` (Xenta)       | `0x1D57`               |
| `idProduct`    | `0xFA55`               | `0xFA60`               |
| Product string | "USB Gaming Mouse"     | "2.4G Wireless Device" |
| Manufacturer   | "Beken"                | *(none)*               |
| `bcdUSB`       | 1.10                   | 1.10                   |

The connection mode is detected by reading `idProduct` after enumeration. Bluetooth mode exists on the hardware but has not been tested with this protocol.

---

## 2. USB Interface Map

The device exposes **4 HID interfaces**. The driver only uses Interface 2.

| Interface | Protocol    | Endpoint | Direction | Purpose                        |
|-----------|-------------|----------|-----------|--------------------------------|
| 0         | Keyboard    | `0x81`   | IN        | Standard keyboard (macros)     |
| 1         | Mouse       | `0x82`   | IN        | Standard mouse movement/clicks |
| **2**     | Generic HID | `0x83`   | IN        | **Config + battery monitoring**|
| 3         | Keyboard    | `0x84`   | IN        | Vendor secondary               |

**Claim Interface 2 before any operation.**
Detach any active kernel driver from Interface 2 first (`libusb_detach_kernel_driver`).

Packet sizes for Interface 2:
- Wired: 16 bytes max packet
- Wireless: 64 bytes max packet

---

## 3. Transfer Mechanics

### All configuration commands use USB HID SET_REPORT (control transfer)

Every command is a **synchronous control transfer** to Interface 2. No interrupt write is used for configuration — only interrupt reads are used (for battery).

#### Fixed fields — identical for every command

| Field           | Value  | Meaning                                    |
|-----------------|--------|--------------------------------------------|
| `bmRequestType` | `0x21` | Host→Device, Class-specific, Interface     |
| `bRequest`      | `0x09` | SET_REPORT                                 |
| `wIndex`        | `0x0002` | Interface 2                              |

#### Variable fields — change per command

| Command            | `wValue` | Report ID | Payload size (wired/wireless) |
|--------------------|----------|-----------|-------------------------------|
| DPI config         | `0x0304` | `0x04`    | 52 / 56 bytes                 |
| User preferences   | `0x0305` | `0x05`    | 13 / 15 bytes                 |
| Polling rate       | `0x0306` | `0x06`    | 9 / 9 bytes                   |
| Button macros      | `0x0308` | `0x08`    | varies                        |
| Internal reset     | `0x030C` | `0x0C`    | 6 / 10 bytes                  |

`wValue` encodes `(0x03 << 8) | report_id`. High byte `0x03` = HID Feature Report type.

#### Minimum delay between packets

**250 ms minimum** between any two consecutive control transfers. The firmware will stop responding if commands arrive too quickly. If the device becomes unresponsive: switch to Bluetooth, wait a few seconds, switch back to 2.4G. Then send a full reconfiguration sequence.

---

## 4. Battery Monitoring — Interrupt IN

Battery data comes from **endpoint `0x83`** on Interface 2 via interrupt IN transfer (device → host). This is a read, not a write.

### Battery report frame format

```
Offset:  [0]   [1]   [2]   [3]   [4]
         0x03  0x55  0x40  0x01  LEVEL
```

| Offset | Value            | Description               |
|--------|------------------|---------------------------|
| 0      | `0x03`           | Report tag (battery type) |
| 1–3    | `0x55 0x40 0x01` | Fixed signature           |
| 4      | `0x00`–`0x64`    | Battery level 0–100%      |

### Implementation notes

- Buffer size: 64 bytes. Only the first 5 bytes are meaningful.
- Identify the frame by checking `buf[0..4] == [0x03, 0x55, 0x40, 0x01]`. Discard all other frames silently.
- Only valid in **wireless mode** (`0xFA60`). In wired mode there is no battery; treat as charging/100%.
- Poll interval: 1 outstanding transfer, 64-byte buffer, 1 ms polling interval.
- Recommended read timeout: 1000–1500 ms per call.

---

## 5. Report 0x04 — DPI Configuration

**wValue:** `0x0304`
**Payload:** 52 bytes (wired) / 56 bytes (wireless, 4 trailing `0x00` padding)

### Buffer layout

| Offset | Field          | Default | Notes                                                  |
|--------|----------------|---------|--------------------------------------------------------|
| 0      | Header 1       | `0x04`  | Fixed — must equal Report ID                           |
| 1      | Header 2       | `0x38`  | Fixed firmware constant                                |
| 2      | Header 3       | `0x01`  | Fixed firmware constant                                |
| 3      | Angle snap     | `0x00`  | `0x01` = on, `0x00` = off                              |
| 4      | Ripple control | `0x01`  | `0x01` = on, `0x00` = off                              |
| 5      | Fixed          | `0x3F`  | Firmware constant — always `0x3F`                      |
| 6      | Stage mask A   | computed| Bitmask: bit `i` set if stage `i+1` DPI > 12 000       |
| 7      | Stage mask B   | computed| Duplicate of offset 6 (same value)                     |
| 8      | Stage 1 DPI    | `0x12`  | Encoded byte from DPI lookup table (default 800)        |
| 9      | Stage 2 DPI    | `0x25`  | Encoded byte (default 1600)                            |
| 10     | Stage 3 DPI    | `0x38`  | Encoded byte (default 2400)                            |
| 11     | Stage 4 DPI    | `0x4B`  | Encoded byte (default 3200)                            |
| 12     | Stage 5 DPI    | `0x75`  | Encoded byte (default 5000)                            |
| 13     | Stage 6 DPI    | `0x81`  | Encoded byte (default 22000)                           |
| 14–15  | Reserved       | `0x00`  | Always zero                                            |
| 16     | High flag S1   | computed| `0x01` if stage 1 DPI in [10100–12000] or [20100–22000]|
| 17     | High flag S2   | computed| Same logic for stage 2                                 |
| 18     | High flag S3   | computed| Same logic for stage 3                                 |
| 19     | High flag S4   | computed| Same logic for stage 4                                 |
| 20     | High flag S5   | computed| Same logic for stage 5                                 |
| 21     | High flag S6   | computed| Same logic for stage 6                                 |
| 22–23  | Reserved       | `0x00`  | Always zero                                            |
| 24     | Active stage   | `0x02`  | 1-indexed current stage (1–6)                          |
| 25–49  | Reserved       | `0x00`  | Always zero                                            |
| 50     | Checksum high  | computed| High byte of 16-bit sum                                |
| 51     | Checksum low   | computed| Low byte of 16-bit sum                                 |
| 52–55  | Padding        | `0x00`  | Wireless only                                          |

### DPI encoding

DPI values are **not linear**. They map through a firmware-specific lookup table (see [Section 10](#10-dpi-lookup-table)).

When fewer than 6 stages are provided, pad remaining stages with the last provided value.

### Stage mask calculation

```
stage_mask = 0
for i in 0..5:
    if dpi_values[i] > 12_000:
        stage_mask |= (1 << i)
buf[6] = stage_mask
buf[7] = stage_mask   // duplicate
```

### High flag calculation

```
for i in 0..5:
    dpi = dpi_values[i]
    if (10_100 <= dpi <= 12_000) or (20_100 <= dpi <= 22_000):
        buf[16 + i] = 0x01
    else:
        buf[16 + i] = 0x00
```

### Checksum

16-bit unsigned sum of `buf[3..=49]`, written big-endian:

```
checksum: u16 = sum(buf[3..50])
buf[50] = (checksum >> 8) as u8
buf[51] = (checksum & 0xFF) as u8
```

---

## 6. Report 0x05 — User Preferences (LED / Sleep / Debounce)

**wValue:** `0x0305`
**Payload:** 13 bytes (wired) / 15 bytes (wireless, 2 trailing `0x00` padding)

### Buffer layout

| Offset | Field         | Default       | Description                                            |
|--------|---------------|---------------|--------------------------------------------------------|
| 0      | Header 1      | `0x05`        | Fixed — Report ID                                      |
| 1      | Header 2      | `0x0F`        | Fixed firmware constant                                |
| 2      | Header 3      | `0x01`        | Fixed firmware constant                                |
| 3      | Light mode    | `0x00`        | LED animation mode (see table below)                   |
| 4      | Config byte 1 | computed      | High nibble = deep sleep high nibble, low nibble = LED speed |
| 5      | Config byte 2 | computed      | High nibble = deep sleep low nibble, low nibble = brightness |
| 6      | RGB red       | `0x00`        | Red component 0–255                                    |
| 7      | RGB green     | `0xFF`        | Green component 0–255                                  |
| 8      | RGB blue      | `0x00`        | Blue component 0–255                                   |
| 9      | Sleep timer   | `0x01`        | Standby sleep (half-minutes: `minutes * 2`)            |
| 10     | Debounce      | `0x04`        | Key response time encoding                             |
| 11     | State flag    | computed      | Dynamic firmware status flag                           |
| 12     | Checksum      | computed      | `sum(buf[3..=10]) & 0xFF`                              |
| 13–14  | Padding       | `0x00`        | Wireless only                                          |

### Light mode values (offset 3)

| Mode name        | Byte   | Description                                   |
|------------------|--------|-----------------------------------------------|
| Off              | `0x00` | LEDs disabled                                 |
| Static           | `0x10` | Fixed color (uses RGB at offsets 6–8)         |
| Breathing        | `0x20` | Pulsing single color                          |
| Neon             | `0x30` | Cycling rainbow                               |
| Color Breathing  | `0x40` | Pulsing, cycling colors                       |
| Static DPI       | `0x50` | Color tracks active DPI stage                 |
| Breathing DPI    | `0x60` | Pulsing, color tracks active DPI stage        |

### Nibble packing (offsets 4 and 5)

Deep sleep timer (1–60 minutes) is split into two hex nibbles and packed with LED speed and brightness:

```
deep_sleep_hex = deep_sleep_minutes as hex  // e.g. 40 min → 0x28
deep_sleep_high_nibble = deep_sleep_hex >> 4
deep_sleep_low_nibble  = deep_sleep_hex & 0x0F

led_speed_encoded = 6 - user_speed     // user_speed range 1–5
                                        // 1 (slow) → 0x5, 5 (fast) → 0x1

buf[4] = (deep_sleep_high_nibble << 4) | led_speed_encoded
buf[5] = (deep_sleep_low_nibble  << 4) | brightness_nibble
```

Brightness is stored as a nibble (0–15, maps linearly to 0–100%).

### Sleep timer (offset 9)

Light sleep (standby) before LEDs dim. Encoded as `minutes * 2`:

| Minutes | Byte   |
|---------|--------|
| 0.5     | `0x01` |
| 1       | `0x02` |
| 5       | `0x0A` |
| 30      | `0x3C` |

Default: `0x01` (0.5 minutes).

### Debounce / key response (offset 10)

Range: 4 ms to 50 ms.

```
encoded = ((ms - 4) / 2) + 2
```

| ms  | Encoded |
|-----|---------|
| 4   | `0x02`  |
| 8   | `0x04`  |
| 20  | `0x0A`  |
| 50  | `0x19`  |

Default: 4 ms → `0x04`.

### State flag (offset 11)

Dynamic firmware hint. Compute as:

```
active_channels = count of (R, G, B) where value >= 100
if light_mode == 0x60 (BreathingDpi):
    active_channels += 1
buf[11] = active_channels
```

### Checksum (offset 12)

```
buf[12] = (buf[3] + buf[4] + buf[5] + buf[6] + buf[7] + buf[8] + buf[9] + buf[10]) & 0xFF
```

---

## 7. Report 0x06 — Polling Rate

**wValue:** `0x0306`
**Payload:** 9 bytes (identical for wired and wireless)

### Buffer layout

| Offset | Field        | Description                          |
|--------|--------------|--------------------------------------|
| 0      | Header 1     | Fixed `0x06`                         |
| 1      | Header 2     | Fixed `0x09`                         |
| 2      | Header 3     | Fixed `0x01`                         |
| 3      | Rate byte    | Encoded polling rate (see table)     |
| 4      | Checksum     | `0xFF - buf[3]` (one's complement)   |
| 5–8    | Padding      | Fixed `0x00 0x00 0x00 0x00`          |

### Polling rate encoding

Higher Hz = smaller byte value (inverted).

| Rate (Hz) | `buf[3]` | `buf[4]` | Profile name  | Full packet                     |
|-----------|----------|----------|---------------|---------------------------------|
| 125       | `0x08`   | `0xF7`   | Power Saving  | `06 09 01 08 F7 00 00 00 00`   |
| 250       | `0x04`   | `0xFB`   | Office        | `06 09 01 04 FB 00 00 00 00`   |
| 500       | `0x02`   | `0xFD`   | Gaming        | `06 09 01 02 FD 00 00 00 00`   |
| 1000      | `0x01`   | `0xFE`   | eSports       | `06 09 01 01 FE 00 00 00 00`   |

Checksum formula: `buf[4] = 0xFF - buf[3]`

---

## 8. Report 0x08 — Button Macros

**wValue:** `0x0308`

Button remapping uses a 56-byte (wireless) / 52-byte (wired) payload. Each button entry maps a physical button to a logical action.

The Custom Macro protocol (`0x0309`) is a 4-packet extension of this report used for complex macro sequences. It shares the same control transfer parameters.

> Full macro packet layout and button code tables are not included in this revision.
> Refer to `src/protocols/MacrosBuilder.ts` and `docs/` in the source repo for byte-level button code maps.
> The safe approach is to call `resetMacro()` after any config reset (see Section 11) to restore defaults.

---

## 9. Report 0x0C — Internal State Reset

**wValue:** `0x030C`

> ⚠️ **DANGEROUS.** This clears the device's volatile RAM configuration. Buttons will stop working until a full reconfiguration sequence is sent. Never send this report alone. Never send it twice in a row.

**Payload:** 6 bytes (wired) / 10 bytes (wireless, 4 trailing `0x00`)

### Buffer layout

| Offset | Value  | Description                        |
|--------|--------|------------------------------------|
| 0      | `0x0C` | Report ID                          |
| 1      | `0x0A` | Magic constant (observed in vendor)|
| 2      | `0x01` | Magic constant                     |
| 3      | `0xFE` | Magic constant                     |
| 4      | `0x01` | Magic constant                     |
| 5      | `0xFE` | Magic constant                     |
| 6–9    | `0x00` | Wireless padding only              |

No checksum. All values are fixed magic constants observed from vendor software captures.

Full packet examples:
- **Wired**: `0C 0A 01 FE 01 FE`
- **Wireless**: `0C 0A 01 FE 01 FE 00 00 00 00`

### What it does and does not do

Does:
- Clears the active in-RAM configuration structure
- Temporarily disables button mapping

Does NOT:
- Erase EEPROM / persistent storage
- Restore factory defaults
- Commit or finalize any configuration

After sending this, you **must** immediately send the full reconfiguration sequence (Section 11) or the device will remain non-functional.

---

## 10. DPI Lookup Table

DPI values are not linear. Each human-readable DPI maps to a firmware-specific byte via a lookup table. Below 10 000 DPI, steps are 50 DPI. Above 10 000 DPI, steps increase to 100+ DPI.

The encoding is non-monotonic in some ranges — the byte range "wraps" at 10 100 and 20 100 DPI, disambiguated by the Stage Mask and High Flag fields in Report 0x04.

### Key anchor points

| DPI    | Byte   | DPI    | Byte   | DPI    | Byte   |
|--------|--------|--------|--------|--------|--------|
| 50     | `0x01` | 800    | `0x12` | 5000   | `0x75` |
| 100    | `0x02` | 1000   | `0x1A` | 8000   | `0xB5` |
| 200    | `0x04` | 1200   | `0x1F` | 10000  | `0xEB` |
| 300    | `0x06` | 1600   | `0x25` | 10100† | `0x01` |
| 400    | `0x09` | 2400   | `0x38` | 12000† | `0x12` |
| 500    | `0x0C` | 3200   | `0x4B` | 16000‡ | `0x40` |
| 600    | `0x0F` | 4000   | `0x5C` | 20000‡ | `0x75` |
| 700    | `0x12` | 4400   | `0x67` | 20100§ | `0x01` |
| 750    | `0x14` | 4800   | `0x6D` | 22000§ | `0x81` |

† `high_flag = 1`, `stage_mask bit = 0`  
‡ `high_flag = 0`, `stage_mask bit = 1`  
§ `high_flag = 1`, `stage_mask bit = 1`

### Encoding algorithm

1. Look up the requested DPI in the table. If an exact match exists, use that byte.
2. If not exact, snap up to the nearest supported step (first table entry ≥ requested DPI).
3. Compute `stage_mask` bit for this stage: set if `dpi > 12_000`.
4. Compute `high_flag` for this stage: set if `dpi in [10_100, 12_000]` or `dpi in [20_100, 22_000]`.

The full 322-entry table is in `src/tables/dpi-map.ts` in the reference repo. The 50 DPI step entries from 50–9950 follow a consistent pattern and can be interpolated if the full table is unavailable.

### Default DPI stages (factory)

| Stage | DPI   | Encoded |
|-------|-------|---------|
| 1     | 800   | `0x12`  |
| 2     | 1600  | `0x25`  |
| 3     | 2400  | `0x38`  |
| 4     | 3200  | `0x4B`  |
| 5     | 5000  | `0x75`  |
| 6     | 22000 | `0x81`  |

---

## 11. Full Reconfiguration Sequence

This is the mandatory order for pushing a complete profile to the device. **Always send all 4 reports after a RAM reset.** Observe the 250 ms delay between each transfer.

```
1. Send Report 0x0C  (Internal State Reset)      ← clears RAM
   [wait 250 ms]
2. Send Report 0x04  (DPI config)
   [wait 250 ms]
3. Send Report 0x05  (User Preferences)
   [wait 250 ms]
4. Send Report 0x06  (Polling Rate)
   [wait 250 ms]
5. Send Report 0x08  (Button Macros / defaults)
```

If only updating a single setting (e.g. just polling rate) without a full reset, you may skip step 1 and send only the relevant report. However, some firmware versions may require the full sequence to commit changes persistently.

---

## 12. Safety Rules and Recovery

### Hard rules

- **Always wait ≥ 250 ms between control transfers.** Violating this causes firmware lockup.
- **Never send Report 0x0C alone.** Always follow it immediately with the full reconfiguration sequence.
- **Never send Report 0x0C twice consecutively.** Undefined behavior.
- **Only send to Interface 2.** Do not send configuration to interfaces 0, 1, or 3.

### Device recovery

If the mouse becomes unresponsive after a bad packet sequence:

1. Switch the physical mode switch to **Bluetooth**
2. Wait 3–5 seconds
3. Switch back to **2.4GHz**
4. Send the full reconfiguration sequence from Section 11

If Bluetooth is unavailable: unplug the USB dongle, wait 5 seconds, replug, then send the full reconfiguration sequence.

---

## 13. Linux udev Setup

To access the device without root, create a udev rule:

```bash
sudo nano /etc/udev/rules.d/99-attack-shark-x11.rules
```

Add:

```
SUBSYSTEM=="usb", ATTR{idVendor}=="1d57", ATTR{idProduct}=="fa60", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="1d57", ATTR{idProduct}=="fa55", MODE="0666", GROUP="plugdev"
```

Reload:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Or reboot.

---

## Appendix: Quick Reference — All wValue Codes

| Feature              | wValue   | Payload bytes (wired/wireless) | Has checksum | Type     |
|----------------------|----------|-------------------------------|--------------|----------|
| DPI config           | `0x0304` | 52 / 56                       | Yes (16-bit) | Computed |
| User preferences     | `0x0305` | 13 / 15                       | Yes (8-bit)  | Computed |
| Polling rate         | `0x0306` | 9 / 9                         | Yes (1-byte) | `0xFF-byte[3]` |
| Button macros        | `0x0308` | 52 / 56                       | Yes          | TBD      |
| Custom macro         | `0x0309` | multi-packet                  | Yes          | TBD      |
| Internal state reset | `0x030C` | 6 / 10                        | No           | —        |

All use: `bmRequestType=0x21`, `bRequest=0x09`, `wIndex=0x0002`

---

*Protocol source: reverse-engineered by HarukaYamamoto0 using Wireshark + USBPcap against the official Windows vendor software. Not affiliated with Attack Shark.*
