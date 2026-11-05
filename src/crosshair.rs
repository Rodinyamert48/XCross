//! Crosshair'in vektörel çizimi.

use crate::config::{Crosshair, Rgba8, Style};
use egui::{Color32, Painter, Pos2, Stroke, Vec2};

/// Rengi, genel opaklıkla ölçeklenmiş alfayla birlikte `Color32`'ye çevirir.
fn color_of(c: Rgba8, opacity: f32) -> Color32 {
    let a = (c.alpha as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.rgb[0], c.rgb[1], c.rgb[2], a)
}

/// Verilen biçim için birim kol yönleri.
fn arm_directions(style: Style) -> Vec<Vec2> {
    let d = std::f32::consts::FRAC_1_SQRT_2;
    match style {
        // Ekran koordinatlarında y aşağı doğru artar.
        Style::Cross | Style::CrossCircle => vec![
            Vec2::new(0.0, -1.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
        ],
        // T biçimi: üst kol yok.
        Style::TShape => vec![
            Vec2::new(0.0, 1.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
        ],
        Style::XShape => vec![
            Vec2::new(-d, -d),
            Vec2::new(d, -d),
            Vec2::new(-d, d),
            Vec2::new(d, d),
        ],
        Style::Dot | Style::Circle => Vec::new(),
    }
}

/// Bir vektörü saat yönünde `deg` derece döndürür.
fn rotate(v: Vec2, deg: f32) -> Vec2 {
    if deg == 0.0 {
        return v;
    }
    let r = deg.to_radians();
    let (s, c) = r.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Tek bir kolun uç noktalarını hesaplar.
fn arm_segment(center: Pos2, dir: Vec2, gap: f32, length: f32) -> [Pos2; 2] {
    [center + dir * gap, center + dir * (gap + length)]
}

/// Crosshair'i `center` noktasına çizer.
///
/// Çizim sırası: önce kontur (daha kalın, kontur rengiyle), sonra ana gövde.
pub fn draw(painter: &Painter, center: Pos2, ch: &Crosshair) {
    let center = if ch.pixel_snap {
        Pos2::new(center.x.round(), center.y.round())
    } else {
        center
    };
    let center = center + Vec2::new(ch.offset[0], ch.offset[1]);

    let opacity = ch.opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    let main = color_of(ch.color, opacity);
    let outline = color_of(ch.outline_color, opacity);
    let dot = color_of(ch.center_dot_color, opacity);
    let ol = if ch.outline {
        ch.outline_thickness.max(0.0)
    } else {
        0.0
    };

    // --- Kollar ---
    if ch.style.has_arms() && ch.arm_length > 0.0 {
        let dirs = arm_directions(ch.style);
        if ol > 0.0 {
            // Kontur, kolu her yönde `ol` kadar büyütür: kalınlık ve uzunluk.
            for dir in &dirs {
                let d = rotate(*dir, ch.rotation);
                let seg = arm_segment(center, d, (ch.gap - ol).max(0.0), ch.arm_length + ol * 2.0);
                painter.line_segment(seg, Stroke::new(ch.thickness + ol * 2.0, outline));
            }
        }
        for dir in &dirs {
            let d = rotate(*dir, ch.rotation);
            let seg = arm_segment(center, d, ch.gap, ch.arm_length);
            painter.line_segment(seg, Stroke::new(ch.thickness, main));
        }
    }

    // --- Daire ---
    if ch.style.has_circle() && ch.circle_radius > 0.0 {
        if ch.circle_filled {
            if ol > 0.0 {
                painter.circle_filled(center, ch.circle_radius + ol, outline);
            }
            painter.circle_filled(center, ch.circle_radius, main);
        } else {
            if ol > 0.0 {
                painter.circle_stroke(
                    center,
                    ch.circle_radius,
                    Stroke::new(ch.thickness + ol * 2.0, outline),
                );
            }
            painter.circle_stroke(center, ch.circle_radius, Stroke::new(ch.thickness, main));
        }
    }

    // --- Merkez noktası ---
    // `Dot` biçiminde nokta her zaman çizilir, diğerlerinde seçenek olarak.
    let draw_dot = ch.style == Style::Dot || ch.center_dot;
    if draw_dot && ch.center_dot_radius > 0.0 {
        if ol > 0.0 {
            painter.circle_filled(center, ch.center_dot_radius + ol, outline);
        }
        painter.circle_filled(center, ch.center_dot_radius, dot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_scales_alpha() {
        let c = Rgba8::new(10, 20, 30, 200);
        assert_eq!(color_of(c, 0.5).a(), 100);
        assert_eq!(color_of(c, 0.0).a(), 0);
        assert_eq!(color_of(c, 1.0).a(), 200);
    }

    #[test]
    fn arm_counts_match_style() {
        assert_eq!(arm_directions(Style::Cross).len(), 4);
        assert_eq!(arm_directions(Style::TShape).len(), 3);
        assert_eq!(arm_directions(Style::XShape).len(), 4);
        assert!(arm_directions(Style::Dot).is_empty());
        assert!(arm_directions(Style::Circle).is_empty());
    }

    #[test]
    fn t_shape_has_no_upward_arm() {
        assert!(arm_directions(Style::TShape).iter().all(|d| d.y >= 0.0));
    }

    #[test]
    fn rotate_90_degrees_maps_axes() {
        let v = rotate(Vec2::new(1.0, 0.0), 90.0);
        assert!((v.x - 0.0).abs() < 1e-5, "x = {}", v.x);
        assert!((v.y - 1.0).abs() < 1e-5, "y = {}", v.y);
    }

    #[test]
    fn arm_segment_respects_gap_and_length() {
        let seg = arm_segment(Pos2::new(100.0, 100.0), Vec2::new(1.0, 0.0), 5.0, 20.0);
        assert_eq!(seg[0], Pos2::new(105.0, 100.0));
        assert_eq!(seg[1], Pos2::new(125.0, 100.0));
    }
}
