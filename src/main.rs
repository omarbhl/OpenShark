use anyhow::Result;
use image::ImageReader;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIconBuilder,
};

fn load_icon(path: &str) -> Result<Icon> {
    let img = ImageReader::open(path)?.decode()?.into_rgba8();
    let (width, height) = img.dimensions();

    Ok(Icon::from_rgba(
        img.into_raw(),
        width,
        height,
    )?)
}

fn main() -> Result<()> {
    println!("OpenShark starting...");

    let event_loop = EventLoop::new();

    let open_item = MenuItem::new("Open OpenShark", true, None);
    let refresh_item = MenuItem::new("Refresh", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let open_id = open_item.id().clone();
    let refresh_id = refresh_item.id().clone();
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    menu.append(&open_item)?;
    menu.append(&refresh_item)?;
    menu.append(&quit_item)?;

    let icon = load_icon("assets/icon.ico")?;

    let mut tray_created = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) if !tray_created => {
                let tray = TrayIconBuilder::new()
                    .with_tooltip("OpenShark")
                    .with_menu(Box::new(menu.clone()))
                    .with_icon(icon.clone())
                    .build()
                    .expect("Failed to create tray icon");

                // Keep tray alive
                Box::leak(Box::new(tray));

                tray_created = true;

                println!("Tray initialized.");
            }

            _ => {}
        }

        // Process menu events
        if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
            let id = menu_event.id;

            if id == quit_id {
                println!("Quit clicked.");
                *control_flow = ControlFlow::Exit;
            } else if id == refresh_id {
                println!("Refresh clicked.");
            } else if id == open_id {
                println!("Open clicked.");
            }
        }
    });
}