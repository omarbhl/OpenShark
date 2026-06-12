# OpenShark

OpenShark is a work in progress tray app for the Attack Shark X11 mouse on Windows.

It currently focuses on:

- live battery status in the tray icon and menu
- DPI display and click-to-cycle support
- wired / wireless / docked state handling
- an autorun toggle in the tray

The protocol work behind this project was helped a lot by the reverse-engineered driver here:

- [HarukaYamamoto0/attack-shark-x11-driver](https://github.com/HarukaYamamoto0/attack-shark-x11-driver)

Huge thanks to that repo for documenting the device behavior and making this project possible.

## Status

This is still a WIP.

- Settings are not implemented yet
- Bluetooth is not implemented
- The tray UI is intentionally minimal for now

## Requirements

- Windows 10 or Windows 11
- An Attack Shark X11 mouse
- The mouse connected through USB or the 2.4G wireless dongle

## How To Run

### From source

1. Install the Rust toolchain from [rustup.rs](https://rustup.rs/).
2. Clone this repository.
3. Open a terminal in the project folder.
4. Run:

```powershell
cargo run
```

### Release build

If you want the packaged executable:

```powershell
cargo build --release
```

The Windows executable icon is embedded from `assets/icon.ico`.

## How To Use

When OpenShark starts, it creates a tray icon and menu.

### Tray menu

- `Mouse` shows the detected mouse name
- `Battery` shows the current battery state
- `DPI` shows the current DPI and lets you cycle to the next value
- `Autorun on boot` enables or disables startup with Windows
- `Settings (soon)` is a placeholder for future settings
- `Exit` closes the app

### Battery icon

The tray icon uses different visuals depending on state:

- battery percentage when available
- a question mark when battery is unknown
- a charging icon when wired
- a docked icon when the mouse is inferred to be on the dock
- a warning icon when battery is low

## Notes

- The app currently reads the device over HID.
- Battery updates are based on what the mouse reports.
- Docked state is inferred from battery stream behavior, not from a dedicated dock sensor.
- DPI cycling uses the factory stages discovered during reverse engineering.

## Troubleshooting

### The mouse is not detected

- Make sure the mouse is plugged in or the 2.4G dongle is connected.
- Try replugging the dongle.
- Launch the app after the mouse is already connected.

### Battery shows unknown

- The app may need a few seconds to receive the first battery report.
- Wireless battery data is only available when the mouse is reporting it.

### DPI cycling fails

- This usually means the device did not accept the report on the current HID path.
- Replugging the mouse or dongle can help.

## Development

The source is organized like this:

- `src/main.rs` - tray app and menu wiring
- `src/mouse.rs` - HID communication and device state
- `src/icon.rs` - tray icon rendering
- `docs/ATTACK_SHARK_X11_PROTOCOL.md` - reverse-engineered protocol notes

## License

No license has been added yet.
