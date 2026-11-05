//! Crosshair geometrisi ve egui ile çizimi.
//!
//! Şekil önce platformdan bağımsız ilkellere ([`Primitive`]) dönüştürülür;
//! aynı liste hem ayarlardaki önizlemede (egui) hem de Windows overlay'inde
//! (tiny-skia, bkz. `raster`) çizilir. Böylece ikisi her zaman aynı görünür.

use crate::config::{Crosshair, Rgba8, Style};
use egui::{Color32, Painter, Pos2, Stroke, Vec2};

/// Merkezi (0, 0) olan koordinat sisteminde tek bir çizim ilkeli.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Primitive {
    /// Düz uçlu (butt cap) çizgi.
    Line {
        a: Pos2,
        b: Pos2,
        width: f32,
        color: Rgba8,
    },
    /// İçi boş daire.
    Ring {
        radius: f32,
        width: f32,
        color: Rgba8,
    },
    /// İçi dolu daire.
    Disc { radius: f32, color: Rgba8 },
}

// Kapsam hesabı yalnızca Windows overlay'inde (raster) kullanılır.
#[cfg_attr(not(windows), allow(dead_code))]
impl Primitive {
    /// Merkezden en uzak çizilen noktanın uzaklığı.
    fn reach(&self) -> f32 {
        match *self {
            Primitive::Line { a, b, width, .. } => {
                a.to_vec2().length().max(b.to_vec2().length()) + width * 0.5
            }
            Primitive::Ring { radius, width, .. } => radius + width * 0.5,
            Primitive::Disc { radius, .. } => radius,
        }
    }
}

/// Rengin alfasını genel opaklıkla ölçekler.
fn with_opacity(c: Rgba8, opacity: f32) -> Rgba8 {
    let alpha = (c.alpha as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    Rgba8 { alpha, ..c }
}

/// Verilen biçim için birim kol yönleri (ekranda y aşağı doğru artar).
fn arm_directions(style: Style) -> Vec<Vec2> {
    let d = std::f32::consts::FRAC_1_SQRT_2;
    match style {
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
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Tek bir kolun uç noktaları.
fn arm_segment(dir: Vec2, gap: f32, length: f32) -> [Pos2; 2] {
    [Pos2::ZERO + dir * gap, Pos2::ZERO + dir * (gap + length)]
}

/// Crosshair'i çizim sırasına göre ilkellere dönüştürür: önce konturlar
/// (daha kalın, kontur rengiyle), sonra gövde, en son merkez noktası.
///
/// Konum kayması (offset) burada uygulanmaz; onu çizen taraf uygular.
pub fn primitives(ch: &Crosshair) -> Vec<Primitive> {
    let mut out = Vec::new();
    let opacity = ch.opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return out;
    }

    let main = with_opacity(ch.color, opacity);
    let outline = with_opacity(ch.outline_color, opacity);
    let dot = with_opacity(ch.center_dot_color, opacity);
    let ol = if ch.outline {
        ch.outline_thickness.max(0.0)
    } else {
        0.0
    };
    let thickness = ch.thickness.max(0.1);

    // --- Kollar ---
    if ch.style.has_arms() && ch.arm_length > 0.0 {
        let dirs: Vec<Vec2> = arm_directions(ch.style)
            .into_iter()
            .map(|d| rotate(d, ch.rotation))
            .collect();
        if ol > 0.0 {
            // Kontur, kolu her yönde `ol` kadar büyütür.
            for &d in &dirs {
                let [a, b] = arm_segment(d, (ch.gap - ol).max(0.0), ch.arm_length + ol * 2.0);
                out.push(Primitive::Line {
                    a,
                    b,
                    width: thickness + ol * 2.0,
                    color: outline,
                });
            }
        }
        for &d in &dirs {
            let [a, b] = arm_segment(d, ch.gap, ch.arm_length);
            out.push(Primitive::Line {
                a,
                b,
                width: thickness,
                color: main,
            });
        }
    }

    // --- Daire ---
    if ch.style.has_circle() && ch.circle_radius > 0.0 {
        if ch.circle_filled {
            if ol > 0.0 {
                out.push(Primitive::Disc {
                    radius: ch.circle_radius + ol,
                    color: outline,
                });
            }
            out.push(Primitive::Disc {
                radius: ch.circle_radius,
                color: main,
            });
        } else {
            if ol > 0.0 {
                out.push(Primitive::Ring {
                    radius: ch.circle_radius,
                    width: thickness + ol * 2.0,
                    color: outline,
                });
            }
            out.push(Primitive::Ring {
                radius: ch.circle_radius,
                width: thickness,
                color: main,
            });
        }
    }

    // --- Merkez noktası ---
    // `Dot` biçiminde nokta her zaman çizilir, diğerlerinde isteğe bağlı.
    let draw_dot = ch.style == Style::Dot || ch.center_dot;
    if draw_dot && ch.center_dot_radius > 0.0 {
        if ol > 0.0 {
            out.push(Primitive::Disc {
                radius: ch.center_dot_radius + ol,
                color: outline,
            });
        }
        out.push(Primitive::Disc {
            radius: ch.center_dot_radius,
            color: dot,
        });
    }

    out
}

/// İlkellerin tamamını içine alan dairenin yarıçapı (piksel).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn extent(prims: &[Primitive]) -> f32 {
    prims.iter().map(Primitive::reach).fold(0.0, f32::max)
}

fn color32(c: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.rgb[0], c.rgb[1], c.rgb[2], c.alpha)
}

/// İlkelleri egui ile `center` noktasına çizer. `scale`, piksel cinsinden
/// tanımlı ölçüleri egui noktalarına çevirir (`1 / pixels_per_point`).
pub fn paint_egui(painter: &Painter, center: Pos2, prims: &[Primitive], scale: f32) {
    let at = |p: Pos2| center + p.to_vec2() * scale;
    for prim in prims {
        match *prim {
            Primitive::Line { a, b, width, color } => {
                painter.line_segment([at(a), at(b)], Stroke::new(width * scale, color32(color)));
            }
            Primitive::Ring {
                radius,
                width,
                color,
            } => {
                painter.circle_stroke(
                    center,
                    radius * scale,
                    Stroke::new(width * scale, color32(color)),
                );
            }
            Primitive::Disc { radius, color } => {
                painter.circle_filled(center, radius * scale, color32(color));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_scales_alpha() {
        let c = Rgba8::new(10, 20, 30, 200);
        assert_eq!(with_opacity(c, 0.5).alpha, 100);
        assert_eq!(with_opacity(c, 0.0).alpha, 0);
        assert_eq!(with_opacity(c, 1.0).alpha, 200);
        assert_eq!(with_opacity(c, 1.0).rgb, c.rgb);
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
        assert!(v.x.abs() < 1e-5, "x = {}", v.x);
        assert!((v.y - 1.0).abs() < 1e-5, "y = {}", v.y);
    }

    #[test]
    fn arm_segment_respects_gap_and_length() {
        let seg = arm_segment(Vec2::new(1.0, 0.0), 5.0, 20.0);
        assert_eq!(seg[0], Pos2::new(5.0, 0.0));
        assert_eq!(seg[1], Pos2::new(25.0, 0.0));
    }

    #[test]
    fn default_crosshair_has_outline_body_and_dot() {
        let prims = primitives(&Crosshair::default());
        // 4 kontur kolu + 4 kol + nokta konturu + nokta
        assert_eq!(prims.len(), 10);
        assert!(matches!(prims.last(), Some(Primitive::Disc { .. })));
    }

    #[test]
    fn zero_opacity_draws_nothing() {
        let ch = Crosshair {
            opacity: 0.0,
            ..Crosshair::default()
        };
        assert!(primitives(&ch).is_empty());
    }

    #[test]
    fn extent_covers_arms_and_outline() {
        // gap 4 + kol 10 + kontur 1 = 15, kalınlık yarısı (2 + 2) / 2 = 2
        let ext = extent(&primitives(&Crosshair::default()));
        assert!((ext - 17.0).abs() < 1e-4, "extent = {ext}");
    }
}
