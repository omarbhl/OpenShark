#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod icon;
mod mouse;

use std::{
    env,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::Result;
use auto_launch::{AutoLaunch, WindowsEnableMode};
use mouse::{ConnectionMode, MouseController, MouseStatus};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

struct TrayMenu {
    mouse_name: MenuItem,
    battery: MenuItem,
    dpi: MenuItem,
    autorun: CheckMenuItem,
    settings: MenuItem,
    exit: MenuItem,
}

impl TrayMenu {
    fn new(autorun_enabled: bool) -> Result<Self> {
        Ok(Self {
            mouse_name: MenuItem::new("Mouse: scanning...", false, None),
            battery: MenuItem::new("Battery: scanning...", false, None),
            dpi: MenuItem::new("DPI: scanning...", true, None),
            autorun: CheckMenuItem::new("Autorun on boot", true, autorun_enabled, None),
            settings: MenuItem::new("Settings (soon)", false, None),
            exit: MenuItem::new("Exit", true, None),
        })
    }

    fn build_menu(&self) -> Result<Menu> {
        let menu = Menu::new();
        menu.append(&self.mouse_name)?;
        menu.append(&self.battery)?;
        menu.append(&self.dpi)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&self.autorun)?;
        menu.append(&self.settings)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&self.exit)?;
        Ok(menu)
    }
}

fn main() -> Result<()> {
    let auto_launch = create_autorun()?;
    let autorun_enabled = auto_launch.is_enabled().unwrap_or(false);
    if autorun_enabled {
        if let Err(error) = auto_launch.enable() {
            eprintln!("Failed to refresh autorun entry: {error}");
        }
    }

    let event_loop = EventLoop::new();
    let menu_items = TrayMenu::new(autorun_enabled)?;
    let menu = menu_items.build_menu()?;
    let exit_id = menu_items.exit.id().clone();
    let dpi_id = menu_items.dpi.id().clone();
    let autorun_id = menu_items.autorun.id().clone();
    let mut mouse = MouseController::new()?;
    let mut latest_status = mouse.refresh();
    let mut next_refresh = Instant::now() + REFRESH_INTERVAL;
    let mut tray: Option<TrayIcon> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(next_refresh);

        match event {
            Event::NewEvents(StartCause::Init) if tray.is_none() => {
                match TrayIconBuilder::new()
                    .with_tooltip(tooltip(&latest_status))
                    .with_menu(Box::new(menu.clone()))
                    .with_menu_on_left_click(true)
                    .with_icon(icon::tray_icon(&latest_status).expect("failed to build tray icon"))
                    .build()
                {
                    Ok(created_tray) => {
                        tray = Some(created_tray);
                        apply_status(&menu_items, tray.as_ref(), &latest_status);
                    }
                    Err(error) => {
                        eprintln!("Failed to create tray icon: {error}");
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                latest_status = mouse.refresh();
                apply_status(&menu_items, tray.as_ref(), &latest_status);
                next_refresh = Instant::now() + REFRESH_INTERVAL;
                *control_flow = ControlFlow::WaitUntil(next_refresh);
            }
            _ => {}
        }

        while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
            let id = menu_event.id;

            if id == exit_id {
                *control_flow = ControlFlow::Exit;
                return;
            }

            if id == dpi_id {
                match mouse.cycle_dpi() {
                    Ok(dpi) => latest_status.dpi = dpi,
                    Err(error) => eprintln!("Failed to cycle DPI: {error}"),
                }

                latest_status = mouse.refresh();
                apply_status(&menu_items, tray.as_ref(), &latest_status);
                next_refresh = Instant::now() + REFRESH_INTERVAL;
                *control_flow = ControlFlow::WaitUntil(next_refresh);
                continue;
            }

            if id == autorun_id {
                if menu_items.autorun.is_checked() {
                    if let Err(error) = auto_launch.enable() {
                        eprintln!("Failed to enable autorun: {error}");
                        menu_items.autorun.set_checked(false);
                    }
                } else if let Err(error) = auto_launch.disable() {
                    eprintln!("Failed to disable autorun: {error}");
                    menu_items.autorun.set_checked(true);
                }
            }
        }
    });
}

fn apply_status(menu: &TrayMenu, tray: Option<&TrayIcon>, status: &MouseStatus) {
    menu.mouse_name.set_text(format!("Mouse: {}", status.name));
    menu.battery.set_text(battery_label(status));
    menu.dpi.set_text(if status.available {
        format!("DPI: {} (click to cycle)", status.dpi)
    } else {
        "DPI: unavailable".to_string()
    });
    menu.dpi.set_enabled(status.available);
    menu.settings.set_text("Settings (soon)");
    menu.settings.set_enabled(false);

    if let Some(tray) = tray {
        if let Err(error) = tray.set_tooltip(Some(tooltip(status))) {
            eprintln!("Failed to update tray tooltip: {error}");
        }

        match icon::tray_icon(status) {
            Ok(icon) => {
                if let Err(error) = tray.set_icon(Some(icon)) {
                    eprintln!("Failed to update tray icon: {error}");
                }
            }
            Err(error) => eprintln!("Failed to build battery icon: {error}"),
        }
    }
}

fn battery_label(status: &MouseStatus) -> String {
    if status.connection_mode == Some(ConnectionMode::Wired) {
        return "Battery: Wired".to_string();
    }

    if status.docked {
        return "Battery: Docked".to_string();
    }

    let Some(percent) = status.battery else {
        return "Battery: Unknown".to_string();
    };

    let freshness = battery_freshness(status.battery_last_seen);

    match (status.remaining_hours, freshness) {
        (Some(hours), Some(freshness)) => {
            format!("Battery: {percent}% ({hours:.0}h remaining, {freshness})")
        }
        (Some(hours), None) => format!("Battery: {percent}% ({hours:.0}h remaining)"),
        (None, Some(freshness)) => format!("Battery: {percent}% ({freshness})"),
        (None, None) => format!("Battery: {percent}%"),
    }
}

fn tooltip(status: &MouseStatus) -> String {
    format!("OpenShark\n{}\n{}", status.name, battery_label(status))
}

fn battery_freshness(last_seen: Option<Instant>) -> Option<String> {
    let last_seen = last_seen?;
    let elapsed = last_seen.elapsed();

    if elapsed.as_secs() < 5 {
        Some("fresh".to_string())
    } else if elapsed.as_secs() < 60 {
        Some(format!("last update {}s ago", elapsed.as_secs()))
    } else {
        Some(format!("last update {}m ago", elapsed.as_secs() / 60))
    }
}

fn create_autorun() -> Result<AutoLaunch> {
    let app_path = preferred_autorun_path()?;
    let app_path = app_path.to_string_lossy().into_owned();
    Ok(AutoLaunch::new(
        "OpenShark",
        &app_path,
        WindowsEnableMode::CurrentUser,
        &[] as &[&str],
    ))
}

fn preferred_autorun_path() -> Result<PathBuf> {
    let current_exe = env::current_exe()?;

    if cfg!(debug_assertions) {
        if let Some(release_exe) = current_exe
            .parent()
            .and_then(|dir| dir.parent())
            .map(|target_dir| target_dir.join("release").join("openshark.exe"))
            .filter(|candidate| candidate.exists())
        {
            return Ok(release_exe);
        }
    }

    Ok(current_exe)
}
