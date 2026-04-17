use egui::Color32;
use zaned_chart_core::Rgba;

/// Convert a chart-core `Rgba` into an `egui::Color32`. Boundary helper so
/// indicator draw code doesn't have to unpack bytes at every use site.
pub fn egui_color(r: Rgba) -> Color32 {
    let [r, g, b, a] = r.0;
    Color32::from_rgba_premultiplied(r, g, b, a)
}

/// Convert an `egui::Color32` back into a chart-core `Rgba`. Used when the
/// egui color picker hands us an edited value to write back into params.
pub fn core_color(c: Color32) -> Rgba {
    Rgba(c.to_array())
}

pub fn format_volume(vol: f32) -> String {
    if vol >= 1_000_000_000.0 {
        format!("{:.2}B", vol / 1_000_000_000.0)
    } else if vol >= 1_000_000.0 {
        format!("{:.2}M", vol / 1_000_000.0)
    } else if vol >= 1_000.0 {
        format!("{:.2}K", vol / 1_000.0)
    } else {
        format!("{}", vol as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_volume_scales() {
        assert_eq!(format_volume(500.0), "500");
        assert_eq!(format_volume(1_500.0), "1.50K");
        assert_eq!(format_volume(1_500_000.0), "1.50M");
        assert_eq!(format_volume(2_300_000_000.0), "2.30B");
    }
}
