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

const QUESTION: [[u8; 3]; 5] = [[1, 1, 1], [0, 0, 1], [0, 1, 1], [0, 0, 0], [0, 1, 0]];

pub fn battery_icon(battery: Option<u8>) -> anyhow::Result<Icon> {
    let bg = match battery {
        Some(60..=100) => [31, 150, 84, 255],
        Some(30..=59) => [220, 161, 38, 255],
        Some(_) => [207, 57, 49, 255],
        None => [98, 108, 122, 255],
    };

    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    draw_circle(&mut rgba, 15.5, 15.5, 15.0, [23, 27, 34, 255]);
    draw_circle(&mut rgba, 15.5, 15.5, 13.2, bg);

    match battery {
        Some(value) => draw_text(&mut rgba, &value.min(100).to_string()),
        None => draw_question(&mut rgba),
    }

    Ok(Icon::from_rgba(rgba, SIZE as u32, SIZE as u32)?)
}

fn draw_text(rgba: &mut [u8], text: &str) {
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
        draw_glyph(rgba, &DIGITS[digit as usize], x, start_y, scale as i32);
    }
}

fn draw_question(rgba: &mut [u8]) {
    draw_glyph(rgba, &QUESTION, 10, 5, 4);
}

fn draw_glyph(rgba: &mut [u8], glyph: &[[u8; 3]; 5], x: i32, y: i32, scale: i32) {
    for (row, values) in glyph.iter().enumerate() {
        for (col, value) in values.iter().enumerate() {
            if *value == 0 {
                continue;
            }

            let px = x + col as i32 * scale;
            let py = y + row as i32 * scale;
            draw_rect(rgba, px, py, scale, scale, [255, 255, 255, 255]);
        }
    }
}

fn draw_circle(rgba: &mut [u8], cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
    let radius_sq = radius * radius;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;

            if dx * dx + dy * dy <= radius_sq {
                set_pixel(rgba, x as i32, y as i32, color);
            }
        }
    }
}

fn draw_rect(rgba: &mut [u8], x: i32, y: i32, width: i32, height: i32, color: [u8; 4]) {
    for py in y..y + height {
        for px in x..x + width {
            set_pixel(rgba, px, py, color);
        }
    }
}

fn set_pixel(rgba: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
        return;
    }

    let offset = (y as usize * SIZE + x as usize) * 4;
    rgba[offset..offset + 4].copy_from_slice(&color);
}
