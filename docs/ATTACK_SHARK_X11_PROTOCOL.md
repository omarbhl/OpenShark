# Attack Shark X11 - USB HID Protocol Reference

> Reverse-engineered from the Windows vendor software via Wireshark/USBPcap.
> Source: [HarukaYamamoto0/attack-shark-x11-driver](https://github.com/HarukaYamamoto0/attack-shark-x11-driver)
> This document is the implementation reference for OpenShark.

## 1. Device Identity

| Property | Wired | Wireless |
|---|---:|---:|
| `idVendor` | `0x1D57` | `0x1D57` |
| `idProduct` | `0xFA55` | `0xFA60` |
| Product string | `USB Gaming Mouse` | `2.4G Wireless Device` |

The current app detects the mode from `idProduct`.

## 2. Interface Map

The driver uses Interface 2.

| Interface | Endpoint | Direction | Purpose |
|---|---:|---|---|
| 0 | `0x81` | IN | Keyboard |
| 1 | `0x82` | IN | Mouse |
| 2 | `0x83` | IN | Config and battery |
| 3 | `0x84` | IN | Vendor secondary |

## 3. Control Transfers

All configuration commands use HID `SET_REPORT` control transfers.

| Field | Value |
|---|---:|
| `bmRequestType` | `0x21` |
| `bRequest` | `0x09` |
| `wIndex` | `0x0002` |

Report IDs:

| Feature | `wValue` | Payload |
|---|---:|---:|
| DPI config | `0x0304` | 52 bytes wired, 56 bytes wireless |
| User preferences | `0x0305` | 13 bytes wired, 15 bytes wireless |
| Polling rate | `0x0306` | 9 bytes |
| Macros | `0x0308` | varies |
| Internal reset | `0x030C` | 6 bytes wired, 10 bytes wireless |

Wait at least 250 ms between control transfers.

## 4. Battery Monitoring

Battery comes from endpoint `0x83` on Interface 2 as an interrupt IN report.

Frame format:

```text
03 55 40 01 LEVEL
```

Where `LEVEL` is `0x00` to `0x64`.

Implementation notes:

- Use a 64-byte buffer.
- Match the prefix `03 55 40 01`.
- Battery is only reported in wireless mode.
- OpenShark treats the latest wireless reading as the current battery percentage.

## 5. DPI Configuration

`wValue = 0x0304`

Buffer layout:

| Offset | Field | Notes |
|---|---:|---|
| 0 | Header 1 | `0x04` |
| 1 | Header 2 | `0x38` |
| 2 | Header 3 | `0x01` |
| 3 | Angle snap | `0x00` or `0x01` |
| 4 | Ripple control | `0x00` or `0x01` |
| 5 | Fixed | `0x3F` |
| 6-7 | Stage mask | Bit set when stage DPI is above `12000` |
| 8-13 | Stage DPI bytes | Encoded stage values |
| 16-21 | High flags | Set for the 10,100-12,000 and 20,100-22,000 ranges |
| 24 | Active stage | 1-indexed |
| 50-51 | Checksum | Big-endian sum of bytes 3-49 |

The current app uses the factory stages:

- 800
- 1600
- 2400
- 3200
- 5000
- 22000

## 6. User Preferences

`wValue = 0x0305`

This report covers lighting, sleep, and key response settings. It is not wired into the tray UI yet.

## 7. Polling Rate

`wValue = 0x0306`

The encoded values are:

| Hz | Byte |
|---|---:|
| 125 | `0x08` |
| 250 | `0x04` |
| 500 | `0x02` |
| 1000 | `0x01` |

## 8. Button Macros

`wValue = 0x0308`

Macro support exists in the upstream driver, but OpenShark does not expose it yet.

## 9. Internal Reset

`wValue = 0x030C`

This clears volatile configuration and must be followed by a full reconfiguration sequence.

## 10. Full Sequence

Recommended order after reset:

1. `0x030C` Internal reset
2. `0x0304` DPI config
3. `0x0305` User preferences
4. `0x0306` Polling rate
5. `0x0308` Macros

## 11. OpenShark Notes

- The tray app shows the latest battery percentage it has seen from the mouse.
- If the battery stream goes quiet for a short period, OpenShark infers that the mouse is probably docked.
- The battery icon uses the numeric percentage and color coding.
- DPI click-to-cycle is meant to use the factory stages above.
- Settings is intentionally disabled for now.
