use crate::mouse::{ConnectionMode, MouseStatus};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg::{Options, Tree},
};
use tray_icon::Icon;

const SIZE: u32 = 256;
const BADGE_MARGIN: u32 = 4;
const ICON_MARGIN: u32 = 0;

pub fn tray_icon(status: &MouseStatus) -> anyhow::Result<Icon> {
    if status.connection_mode == Some(ConnectionMode::Wired) {
        return render_svg(include_bytes!("../assets/battery-charging.svg"), ICON_MARGIN);
    }

    if status.docked {
        return render_svg(include_bytes!("../assets/battery-docked.svg"), ICON_MARGIN);
    }

    if status.battery.is_none() {
        return render_svg(include_bytes!("../assets/unknown.svg"), ICON_MARGIN);
    }

    if matches!(status.battery, Some(0..=19)) {
        return render_svg(include_bytes!("../assets/battery-warning.svg"), ICON_MARGIN);
    }

    render_battery_percentage(status.battery.unwrap())
}

fn render_svg(svg: &[u8], margin: u32) -> anyhow::Result<Icon> {
    let mut options = Options::default();
    options.fontdb_mut().load_system_fonts();

    let tree = Tree::from_data(svg, &options).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(SIZE, SIZE).ok_or_else(|| anyhow::anyhow!("failed to create icon pixmap"))?;
    let scale_x = SIZE as f32 / size.width() as f32;
    let scale_y = SIZE as f32 / size.height() as f32;

    resvg::render(
        &tree,
        Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );

    let cropped = crop_and_scale(pixmap.data(), SIZE, SIZE, margin)?;
    Ok(Icon::from_rgba(cropped, SIZE, SIZE)?)
}

fn render_battery_percentage(battery: u8) -> anyhow::Result<Icon> {
    let color = match battery {
        51..=100 => "rgb(31,150,84)",
        25..=50 => "rgb(220,161,38)",
        0..=24 => "rgb(207,57,49)",
        _ => "rgb(98,108,122)",
    };

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">
<rect width="{size}" height="{size}" fill="transparent"/>
<text x="50%" y="50%"
      fill="{color}"
      font-family="Segoe UI Variable Text, Segoe UI, Arial, sans-serif"
      font-size="118"
      font-weight="700"
      text-anchor="middle"
      dominant-baseline="middle"
      letter-spacing="0">{battery}</text>
</svg>"#,
        size = SIZE,
        color = color,
        battery = battery.min(100),
    );

    render_svg(svg.as_bytes(), BADGE_MARGIN)
}

fn crop_and_scale(rgba: &[u8], width: u32, height: u32, margin: u32) -> anyhow::Result<Vec<u8>> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..height {
        for x in 0..width {
            let alpha = rgba[((y * width + x) * 4 + 3) as usize];
            if alpha == 0 {
                continue;
            }

            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if !found {
        return Ok(rgba.to_vec());
    }

    let crop_w = max_x - min_x + 1;
    let crop_h = max_y - min_y + 1;
    let target_w = width.saturating_sub(margin * 2).max(1);
    let target_h = height.saturating_sub(margin * 2).max(1);
    let scale = (target_w as f32 / crop_w as f32).min(target_h as f32 / crop_h as f32);
    let scaled_w = (crop_w as f32 * scale).round().max(1.0) as u32;
    let scaled_h = (crop_h as f32 * scale).round().max(1.0) as u32;
    let offset_x = (width - scaled_w) / 2;
    let offset_y = (height - scaled_h) / 2;
    let mut out = vec![0_u8; (width * height * 4) as usize];

    for y in 0..scaled_h {
        for x in 0..scaled_w {
            let src_x = min_x + ((x as f32 / scaled_w as f32) * crop_w as f32).floor() as u32;
            let src_y = min_y + ((y as f32 / scaled_h as f32) * crop_h as f32).floor() as u32;
            let src_x = src_x.min(width - 1);
            let src_y = src_y.min(height - 1);
            let src_offset = ((src_y * width + src_x) * 4) as usize;
            let dst_offset = (((y + offset_y) * width + (x + offset_x)) * 4) as usize;
            out[dst_offset..dst_offset + 4].copy_from_slice(&rgba[src_offset..src_offset + 4]);
        }
    }

    Ok(out)
}
