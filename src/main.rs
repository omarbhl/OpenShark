mod icon;
mod mouse;

use std::time::{Duration, Instant};

use anyhow::Result;
use mouse::{ConnectionMode, MouseController, MouseStatus};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

struct TrayMenu {
    mouse_name: MenuItem,
    battery: MenuItem,
    dpi: MenuItem,
    settings: MenuItem,
    exit: MenuItem,
}

impl TrayMenu {
    fn new() -> Result<Self> {
        Ok(Self {
            mouse_name: MenuItem::new("Mouse: scanning...", false, None),
            battery: MenuItem::new("Battery: scanning...", false, None),
            dpi: MenuItem::new("DPI: scanning...", true, None),
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
        menu.append(&self.settings)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&self.exit)?;
        Ok(menu)
    }
}

fn main() -> Result<()> {
    println!("OpenShark starting...");

    let event_loop = EventLoop::new();
    let menu_items = TrayMenu::new()?;
    let menu = menu_items.build_menu()?;
    let exit_id = menu_items.exit.id().clone();
    let dpi_id = menu_items.dpi.id().clone();
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
                    .with_icon(
                        icon::battery_icon(latest_status.battery)
                            .expect("failed to build tray icon"),
                    )
                    .build()
                {
                    Ok(created_tray) => {
                        tray = Some(created_tray);
                        apply_status(&menu_items, tray.as_ref(), &latest_status);
                        println!("Tray initialized.");
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
                println!("Exit clicked.");
                *control_flow = ControlFlow::Exit;
                return;
            }

            if id == dpi_id {
                match mouse.cycle_dpi() {
                    Ok(dpi) => {
                        println!("DPI changed to {dpi}.");
                        latest_status.dpi = dpi;
                    }
                    Err(error) => {
                        eprintln!("Failed to cycle DPI: {error}");
                    }
                }

                latest_status = mouse.refresh();
                apply_status(&menu_items, tray.as_ref(), &latest_status);
                next_refresh = Instant::now() + REFRESH_INTERVAL;
                *control_flow = ControlFlow::WaitUntil(next_refresh);
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

        match icon::battery_icon(status.battery) {
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
    let Some(percent) = status.battery else {
        return if status.connection_mode == Some(ConnectionMode::Wired) {
            "Battery: Wired mode".to_string()
        } else {
            "Battery: Unknown".to_string()
        };
    };

    match status.remaining_hours {
        Some(hours) => format!("Battery: {percent}% ({hours:.0}h remaining)"),
        None => format!("Battery: {percent}%"),
    }
}

fn tooltip(status: &MouseStatus) -> String {
    format!("OpenShark\n{}\n{}", status.name, battery_label(status))
}
