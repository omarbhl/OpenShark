use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use hidapi::{HidApi, HidDevice};

const VENDOR_ID: u16 = 0x1d57;
const PRODUCT_WIRELESS: u16 = 0xfa60;
const PRODUCT_WIRED: u16 = 0xfa55;
const WRITE_DELAY: Duration = Duration::from_millis(500);
const BATTERY_TIMEOUT_MS: i32 = 1000;
const FULL_BATTERY_HOURS: f32 = 80.0;

const DPI_STAGES: [u16; 6] = [800, 1600, 2400, 3200, 5000, 22000];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionMode {
    Wireless,
    Wired,
}

#[derive(Clone, Debug)]
pub struct MouseStatus {
    pub name: String,
    pub connection_mode: Option<ConnectionMode>,
    pub battery: Option<u8>,
    pub remaining_hours: Option<f32>,
    pub dpi: u16,
    pub available: bool,
}

pub struct MouseController {
    api: HidApi,
    current_stage: usize,
    battery_samples: Vec<(Instant, u8)>,
    battery_rx: Receiver<u8>,
    last_battery: Option<u8>,
}

impl MouseController {
    pub fn new() -> Result<Self> {
        let (battery_tx, battery_rx) = mpsc::channel();
        spawn_battery_listener(battery_tx);

        Ok(Self {
            api: HidApi::new().context("failed to initialize HID API")?,
            current_stage: 1,
            battery_samples: Vec::new(),
            battery_rx,
            last_battery: None,
        })
    }

    pub fn refresh(&mut self) -> MouseStatus {
        if let Err(error) = self.api.refresh_devices() {
            eprintln!("Failed to refresh HID devices: {error}");
        }

        while let Ok(percent) = self.battery_rx.try_recv() {
            self.last_battery = Some(percent);
            self.push_battery_sample(percent);
        }

        let found = self
            .find_device(ConnectionMode::Wireless)
            .or_else(|| self.find_device(ConnectionMode::Wired));
        let Some(device_info) = found else {
            return MouseStatus {
                name: "Attack Shark X11 not found".to_string(),
                connection_mode: None,
                battery: None,
                remaining_hours: None,
                dpi: self.current_dpi(),
                available: false,
            };
        };

        let battery = if device_info.mode == ConnectionMode::Wireless {
            self.read_battery().ok().flatten().or(self.last_battery)
        } else {
            None
        };

        if let Some(percent) = battery {
            self.last_battery = Some(percent);
            self.push_battery_sample(percent);
        }

        MouseStatus {
            name: device_info.name,
            connection_mode: Some(device_info.mode),
            battery,
            remaining_hours: self.remaining_hours(battery),
            dpi: self.current_dpi(),
            available: true,
        }
    }

    pub fn cycle_dpi(&mut self) -> Result<u16> {
        let next_stage = (self.current_stage + 1) % DPI_STAGES.len();
        if let Err(error) = self.api.refresh_devices() {
            eprintln!("Failed to refresh HID devices before DPI write: {error}");
        }

        let mode = self
            .find_device(ConnectionMode::Wireless)
            .or_else(|| self.find_device(ConnectionMode::Wired))
            .map(|device| device.mode)
            .ok_or_else(|| anyhow!("Attack Shark X11 not found"))?;

        self.send_dpi_to_any_path(mode, next_stage)
            .with_context(|| format!("failed to set DPI stage {}", next_stage + 1))?;
        std::thread::sleep(WRITE_DELAY);

        self.current_stage = next_stage;
        Ok(self.current_dpi())
    }

    pub fn current_dpi(&self) -> u16 {
        DPI_STAGES[self.current_stage]
    }

    fn read_battery(&self) -> Result<Option<u8>> {
        for device in self.open_devices(ConnectionMode::Wireless) {
            let mut buf = [0_u8; 128];

            let Ok(bytes_read) = device.read_timeout(&mut buf, 25) else {
                continue;
            };

            if let Some(percent) = parse_battery_report(&buf[..bytes_read]) {
                return Ok(Some(percent));
            }
        }

        Ok(None)
    }

    fn send_dpi_to_any_path(&self, mode: ConnectionMode, stage: usize) -> Result<()> {
        let mut errors = Vec::new();

        for device in self.open_devices(mode) {
            for report in dpi_report_variants(mode, stage) {
                match device.send_feature_report(&report) {
                    Ok(_) => return Ok(()),
                    Err(error) => errors.push(error.to_string()),
                }

                match device.write(&report) {
                    Ok(_) => return Ok(()),
                    Err(error) => errors.push(error.to_string()),
                }
            }
        }

        Err(anyhow!(
            "all HID feature report attempts failed: {}",
            if errors.is_empty() {
                "no matching HID paths opened".to_string()
            } else {
                errors.join("; ")
            }
        ))
    }

    fn open_devices(&self, mode: ConnectionMode) -> Vec<HidDevice> {
        self.api
            .device_list()
            .filter(|device| is_x11_device(device, mode))
            .filter_map(|device| match device.open_device(&self.api) {
                Ok(device) => Some(device),
                Err(error) => {
                    eprintln!("Failed to open HID path for {:?}: {error}", mode);
                    None
                }
            })
            .collect()
    }

    fn find_device(&self, mode: ConnectionMode) -> Option<DetectedMouse> {
        self.api
            .device_list()
            .find(|device| is_x11_device(device, mode))
            .map(|device| DetectedMouse {
                mode,
                name: device
                    .product_string()
                    .unwrap_or("Attack Shark X11")
                    .to_string(),
            })
    }

    fn push_battery_sample(&mut self, percent: u8) {
        let now = Instant::now();

        if self
            .battery_samples
            .last()
            .is_some_and(|(_, last)| *last == percent)
        {
            return;
        }

        self.battery_samples.push((now, percent));

        if self.battery_samples.len() > 8 {
            self.battery_samples.remove(0);
        }
    }

    fn remaining_hours(&self, battery: Option<u8>) -> Option<f32> {
        let battery = battery?;

        let Some(first_higher_sample) = self
            .battery_samples
            .iter()
            .find(|(_, percent)| *percent > battery)
        else {
            return Some(FULL_BATTERY_HOURS * (battery as f32 / 100.0));
        };

        let latest = self.battery_samples.last()?;
        let elapsed_hours = latest.0.duration_since(first_higher_sample.0).as_secs_f32() / 3600.0;
        let consumed = (first_higher_sample.1 - latest.1) as f32;

        if elapsed_hours > 0.0 && consumed > 0.0 {
            return Some((battery as f32 / (consumed / elapsed_hours)).max(0.0));
        }

        Some(FULL_BATTERY_HOURS * (battery as f32 / 100.0))
    }
}

struct DetectedMouse {
    mode: ConnectionMode,
    name: String,
}

impl ConnectionMode {
    fn product_id(self) -> u16 {
        match self {
            ConnectionMode::Wireless => PRODUCT_WIRELESS,
            ConnectionMode::Wired => PRODUCT_WIRED,
        }
    }
}

fn is_x11_device(device: &hidapi::DeviceInfo, mode: ConnectionMode) -> bool {
    device.vendor_id() == VENDOR_ID
        && device.product_id() == mode.product_id()
        && device.interface_number() == 2
}

fn dpi_report_variants(mode: ConnectionMode, stage: usize) -> Vec<Vec<u8>> {
    let exact = dpi_report(mode, stage);
    let mut reports = vec![exact.clone()];

    for padded_len in [56, 64, 65] {
        if exact.len() < padded_len {
            let mut padded = exact.clone();
            padded.resize(padded_len, 0);
            reports.push(padded);
        }
    }

    reports
}

fn dpi_report(mode: ConnectionMode, stage: usize) -> Vec<u8> {
    let mut buffer = vec![0_u8; 56];
    buffer[0] = 0x04;
    buffer[1] = 0x38;
    buffer[2] = 0x01;
    buffer[3] = 0x00;
    buffer[4] = 0x01;
    buffer[5] = 0x3f;
    buffer[8] = 0x12;
    buffer[9] = 0x25;
    buffer[10] = 0x38;
    buffer[11] = 0x4b;
    buffer[12] = 0x75;
    buffer[13] = 0x81;
    buffer[24] = (stage + 1) as u8;

    update_dpi_stage_metadata(&mut buffer);
    let checksum = buffer[3..=49]
        .iter()
        .fold(0_u16, |sum, byte| sum.wrapping_add(*byte as u16));
    buffer[50] = (checksum >> 8) as u8;
    buffer[51] = checksum as u8;

    if mode == ConnectionMode::Wired {
        buffer.truncate(52);
    }

    buffer
}

fn spawn_battery_listener(tx: mpsc::Sender<u8>) {
    thread::spawn(move || {
        loop {
            match HidApi::new() {
                Ok(api) => {
                    for info in api
                        .device_list()
                        .filter(|device| is_x11_device(device, ConnectionMode::Wireless))
                    {
                        let Ok(device) = info.open_device(&api) else {
                            continue;
                        };

                        listen_for_battery(&device, &tx);
                    }
                }
                Err(error) => eprintln!("Battery listener failed to initialize HID API: {error}"),
            }

            thread::sleep(Duration::from_secs(5));
        }
    });
}

fn listen_for_battery(device: &HidDevice, tx: &mpsc::Sender<u8>) {
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(3) {
        let mut buf = [0_u8; 128];

        match device.read_timeout(&mut buf, BATTERY_TIMEOUT_MS) {
            Ok(bytes_read) => {
                if let Some(percent) = parse_battery_report(&buf[..bytes_read]) {
                    let _ = tx.send(percent);
                    return;
                }
            }
            Err(error) => {
                eprintln!("Battery listener read failed: {error}");
                return;
            }
        }
    }
}

fn parse_battery_report(report: &[u8]) -> Option<u8> {
    if report.len() >= 5 && report[0..4] == [0x03, 0x55, 0x40, 0x01] && report[4] <= 100 {
        Some(report[4])
    } else {
        None
    }
}

fn update_dpi_stage_metadata(buffer: &mut [u8]) {
    let mut mask = 0_u8;

    for (index, dpi) in DPI_STAGES.iter().enumerate() {
        if *dpi > 12000 {
            mask |= 1 << index;
        }

        buffer[16 + index] = if (10100..=12000).contains(dpi) || (20100..=22000).contains(dpi) {
            0x01
        } else {
            0x00
        };
    }

    buffer[6] = mask;
    buffer[7] = mask;
}
