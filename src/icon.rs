use tray_icon::Icon;

const SIZE: usize = 32;

const DIGITS: [[[u8; 3]; 5]; 10] = [
    [[1, 1, 1], [1, 0, 1], [1, 0, 1], [1, 0, 1], [1, 1, 1]],
    [[0, 1, 0], [1, 1, 0], [0, 1, 0], [0, 1, 0], [1, 1, 1]],
    [[1, 1, 1], [0, 0, 1], [1, 1, 1], [1, 0, 0], [1, 1, 1]],
    [[1, 1, 1], [0, 0, 1], [0, 1, 1], [0, 0, 1], [1, 1, 1]],
    [[1, 0, 1], [1, 0, 1], [1, 1, 1], [0, 0, 1], [0, 0, 1]],
    [[1, 1, 1], [1, 0, 0], [1, 1, 1], [0, 0, 1], [1, 1, 1]],
    [[1, 1, 1], [1, 0, 0], [1, 1, 1], [1, 0, 1], [1, 1, 1]],
    [[1, 1, 1], [0, 0, 1], [0, 1, 0], [1, 0, 0], [1, 0, 0]],
    [[1, 1, 1], [1, 0, 1], [1, 1, 1], [1, 0, 1], [1, 1, 1]],
    [[1, 1, 1], [1, 0, 1], [1, 1, 1], [0, 0, 1], [1, 1, 1]],
];

const QUESTION: [[u8; 3]; 5] = [
    [1, 1, 1],
    [0, 0, 1],
    [0, 1, 1],
    [0, 0, 0],
    [0, 1, 0],
];

pub fn battery_icon(battery: Option<u8>) -> anyhow::Result<Icon> {
    let color = match battery {
        Some(60..=100) => [31, 150, 84, 255],
        Some(30..=59) => [220, 161, 38, 255],
        Some(_) => [207, 57, 49, 255],
        None => [98, 108, 122, 255],
    };

    let mut rgba = vec![0_u8; SIZE * SIZE * 4];

    match battery {
        Some(value) => draw_text(&mut rgba, &value.min(100).to_string(), color),
        None => draw_question(&mut rgba, color),
    }

    Ok(Icon::from_rgba(rgba, SIZE as u32, SIZE as u32)?)
}

fn draw_text(rgba: &mut [u8], text: &str, color: [u8; 4]) {
    let scale = match text.len() {
        1 => 5,
        2 => 4,
        _ => 3,
    };
    let glyph_width = 3 * scale;
    let gap = scale;
    let total_width = text.len() * glyph_width + text.len().saturating_sub(1) * gap;
    let start_x = ((SIZE - total_width) / 2) as i32;
    let start_y = ((SIZE - 5 * scale) / 2) as i32;

    for (index, ch) in text.chars().enumerate() {
        let Some(digit) = ch.to_digit(10) else {
            continue;
        };
        let x = start_x + (index * (glyph_width + gap)) as i32;
        draw_glyph(rgba, &DIGITS[digit as usize], x, start_y, scale as i32, color);
    }
}

fn draw_question(rgba: &mut [u8], color: [u8; 4]) {
    draw_glyph(rgba, &QUESTION, 10, 5, 4, color);
}

fn draw_glyph(rgba: &mut [u8], glyph: &[[u8; 3]; 5], x: i32, y: i32, scale: i32, color: [u8; 4]) {
    for (row, values) in glyph.iter().enumerate() {
        for (col, value) in values.iter().enumerate() {
            if *value == 0 {
                continue;
            }
            let px = x + col as i32 * scale;
            let py = y + row as i32 * scale;
            draw_rect_aa(rgba, px, py, scale, scale, color);
        }
    }
}

fn draw_rect_aa(rgba: &mut [u8], x: i32, y: i32, width: i32, height: i32, color: [u8; 4]) {
    for py in (y - 1)..=(y + height) {
        for px in (x - 1)..=(x + width) {
            let fx = (x as f32 - px as f32).max(0.0).max(px as f32 - (x + width - 1) as f32);
            let fy = (y as f32 - py as f32).max(0.0).max(py as f32 - (y + height - 1) as f32);
            let alpha = (1.0 - fx.max(fy)).clamp(0.0, 1.0);

            if alpha <= 0.0 {
                continue;
            }

            blend_pixel(rgba, px, py, color, alpha);
        }
    }
}

fn blend_pixel(rgba: &mut [u8], x: i32, y: i32, color: [u8; 4], alpha: f32) {
    if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
        return;
    }

    let offset = (y as usize * SIZE + x as usize) * 4;
    let a = alpha * (color[3] as f32 / 255.0);
    let inv = 1.0 - a;

    rgba[offset]     = (color[0] as f32 * a + rgba[offset]     as f32 * inv) as u8;
    rgba[offset + 1] = (color[1] as f32 * a + rgba[offset + 1] as f32 * inv) as u8;
    rgba[offset + 2] = (color[2] as f32 * a + rgba[offset + 2] as f32 * inv) as u8;
    rgba[offset + 3] = ((a + rgba[offset + 3] as f32 / 255.0 * inv) * 255.0).min(255.0) as u8;
}