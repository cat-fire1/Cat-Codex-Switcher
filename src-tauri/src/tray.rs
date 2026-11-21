use crate::worker::Snapshot;

pub fn percentage(snapshot: &Snapshot) -> Option<(i32, bool)> {
    let row = snapshot
        .rows
        .iter()
        .find(|row| Some(row.profile.id) == snapshot.active)?;
    let usage = row.usage.as_ref()?;
    let value = if snapshot.settings.status_bar_usage_window == "fiveHour" {
        usage
            .five_hour_remaining_percent
            .unwrap_or(usage.remaining_percent)
    } else {
        usage.remaining_percent
    };
    Some((value.clamp(0, 100), row.stale))
}

pub fn tooltip(snapshot: &Snapshot) -> String {
    let Some(row) = snapshot
        .rows
        .iter()
        .find(|row| Some(row.profile.id) == snapshot.active)
    else {
        return "Cat Codex Switcher".into();
    };
    let mut text = format!("Cat Codex Switcher\n{}", row.title);
    if let Some(usage) = &row.usage {
        text.push_str(&format!("\n7d: {}%", usage.remaining_percent));
        if let Some(five) = usage.five_hour_remaining_percent {
            text.push_str(&format!(" | 5h: {five}%"));
        }
    }
    if row.stale {
        text.push_str("\nCached / stale");
    }
    if let Some(at) = row.fetched_at {
        text.push_str(&format!("\n{}", at.format("%Y-%m-%d %H:%M UTC")));
    }
    text
}

pub fn number_icon(value: i32, stale: bool) -> tauri::image::Image<'static> {
    const DIGITS: [[u8; 5]; 10] = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 1, 1, 1],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
    ];
    let text = value.clamp(0, 100).to_string();
    let scale = if text.len() == 3 { 3 } else { 4 };
    let width = text.len() * (3 * scale + 1) - 1;
    let mut rgba = vec![0; 32 * 32 * 4];
    let color = if stale {
        [255, 190, 70, 255]
    } else {
        [255, 122, 69, 255]
    };
    for (index, digit) in text.bytes().enumerate() {
        for (y, bits) in DIGITS[(digit - b'0') as usize].iter().enumerate() {
            for x in 0..3 {
                if bits & (1 << (2 - x)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = (32 - width) / 2 + index * (3 * scale + 1) + x * scale + dx;
                        let py = (32 - 5 * scale) / 2 + y * scale + dy;
                        rgba[(py * 32 + px) * 4..(py * 32 + px) * 4 + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
    tauri::image::Image::new_owned(rgba, 32, 32)
}
