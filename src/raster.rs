//! Crosshair'i CPU'da piksel tamponuna çizer (tiny-skia).
//!
//! Windows overlay'i bu tamponu `UpdateLayeredWindow` ile ekrana basar;
//! sistem tepsisi simgesi de buradan üretilir.

// `render` yalnızca Windows overlay'inde kullanılır; diğer platformlarda
// testler için derlenir.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::config::{Crosshair, Rgba8};
use crate::crosshair::{self, Primitive};
use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Çizilmiş crosshair ve ekrana nasıl yerleştirileceği.
pub struct Raster {
    pub pixmap: Pixmap,
    /// Crosshair merkezinin tampon içindeki konumu (piksel). Pencere,
    /// bu nokta ekran merkezine denk gelecek şekilde yerleştirilir.
    pub origin: (i32, i32),
}

impl Raster {
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Windows DIB'leri için önceden çarpılmış BGRA baytları.
    pub fn bgra_premultiplied(&self) -> Vec<u8> {
        let mut out = self.pixmap.data().to_vec();
        for px in out.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        out
    }
}

fn paint_for(color: Rgba8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(
        color.rgb[0],
        color.rgb[1],
        color.rgb[2],
        color.alpha,
    ));
    paint.anti_alias = true;
    paint
}

/// İlkelleri `center` merkezli olarak `pixmap`'e çizer.
fn draw_into(pixmap: &mut Pixmap, center: (f32, f32), prims: &[Primitive], scale: f32) {
    let (cx, cy) = center;
    for prim in prims {
        match *prim {
            Primitive::Line { a, b, width, color } => {
                let mut pb = PathBuilder::new();
                pb.move_to(cx + a.x * scale, cy + a.y * scale);
                pb.line_to(cx + b.x * scale, cy + b.y * scale);
                if let Some(path) = pb.finish() {
                    let stroke = Stroke {
                        width: width * scale,
                        line_cap: LineCap::Butt,
                        ..Stroke::default()
                    };
                    pixmap.stroke_path(
                        &path,
                        &paint_for(color),
                        &stroke,
                        Transform::identity(),
                        None,
                    );
                }
            }
            Primitive::Ring {
                radius,
                width,
                color,
            } => {
                if let Some(path) = PathBuilder::from_circle(cx, cy, radius * scale) {
                    let stroke = Stroke {
                        width: width * scale,
                        ..Stroke::default()
                    };
                    pixmap.stroke_path(
                        &path,
                        &paint_for(color),
                        &stroke,
                        Transform::identity(),
                        None,
                    );
                }
            }
            Primitive::Disc { radius, color } => {
                if let Some(path) = PathBuilder::from_circle(cx, cy, radius * scale) {
                    pixmap.fill_path(
                        &path,
                        &paint_for(color),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
    }
}

/// Crosshair'i gereken en küçük tampona çizer. Çizilecek bir şey yoksa
/// (ör. opaklık 0) `None` döner.
pub fn render(ch: &Crosshair) -> Option<Raster> {
    let prims = crosshair::primitives(ch);
    let reach = crosshair::extent(&prims);
    if prims.is_empty() || reach <= 0.0 {
        return None;
    }

    // Kenar yumuşatma için her yanda 2 piksel pay; boyut çift sayı olsun ki
    // merkez tam bir piksel sınırına düşsün.
    let half = reach.ceil() as u32 + 2;
    let size = half * 2;
    let mut pixmap = Pixmap::new(size, size)?;

    // Tek sayılı kalınlıktaki çizgiler piksel sınırına çizilirse iki piksele
    // yayılıp bulanıklaşır; merkezi yarım piksel kaydırmak bunu önler.
    let mut c = half as f32;
    if ch.pixel_snap && (ch.thickness.round() as i64) % 2 == 1 {
        c += 0.5;
    }
    draw_into(&mut pixmap, (c, c), &prims, 1.0);

    Some(Raster {
        pixmap,
        origin: (half as i32, half as i32),
    })
}

/// Sistem tepsisi için kare simge üretir: çarpılmamış RGBA baytları.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let Some(mut pixmap) = Pixmap::new(size, size) else {
        return Vec::new();
    };
    let s = size as f32 / 32.0;
    let green = Rgba8::new(40, 230, 90, 255);
    let black = Rgba8::new(0, 0, 0, 255);
    let ring = |r: f32, w: f32, color| Primitive::Ring {
        radius: r,
        width: w,
        color,
    };
    let line = |ax: f32, ay: f32, bx: f32, by: f32, w: f32, color| Primitive::Line {
        a: egui::pos2(ax, ay),
        b: egui::pos2(bx, by),
        width: w,
        color,
    };
    let prims = [
        ring(11.0, 5.0, black),
        line(0.0, -15.0, 0.0, -4.0, 5.0, black),
        line(0.0, 4.0, 0.0, 15.0, 5.0, black),
        line(-15.0, 0.0, -4.0, 0.0, 5.0, black),
        line(4.0, 0.0, 15.0, 0.0, 5.0, black),
        ring(11.0, 2.5, green),
        line(0.0, -14.0, 0.0, -5.0, 2.5, green),
        line(0.0, 5.0, 0.0, 14.0, 2.5, green),
        line(-14.0, 0.0, -5.0, 0.0, 2.5, green),
        line(5.0, 0.0, 14.0, 0.0, 2.5, green),
    ];
    let c = size as f32 / 2.0;
    draw_into(&mut pixmap, (c, c), &prims, s);
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let d = p.demultiply();
            [d.red(), d.green(), d.blue(), d.alpha()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Style;

    fn pixel(r: &Raster, x: i32, y: i32) -> tiny_skia::PremultipliedColorU8 {
        r.pixmap.pixel(x as u32, y as u32).unwrap()
    }

    #[test]
    fn default_crosshair_renders_arms_and_transparent_corners() {
        let r = render(&Crosshair::default()).expect("çizilmeli");
        let (ox, oy) = r.origin;
        // Sağ kolun ortası: merkez + gap(4) + kol(10)/2 = 9 piksel sağ.
        let arm = pixel(&r, ox + 9, oy);
        assert!(
            arm.green() > 200 && arm.alpha() > 200,
            "kol yeşil değil: {arm:?}"
        );
        // Köşe tamamen saydam olmalı.
        assert_eq!(pixel(&r, 0, 0).alpha(), 0);
        // Merkez boşluğu ile nokta arası (2 piksel sağ) kontur rengi/ boş;
        // ama tam merkezdeki nokta yeşil.
        let center = pixel(&r, ox, oy);
        assert!(center.green() > 100, "merkez noktası yok: {center:?}");
    }

    #[test]
    fn origin_is_buffer_center() {
        let r = render(&Crosshair::default()).unwrap();
        assert_eq!(r.width(), r.height());
        assert_eq!(r.origin.0 as u32 * 2, r.width());
    }

    #[test]
    fn nothing_to_draw_returns_none() {
        let ch = Crosshair {
            opacity: 0.0,
            ..Crosshair::default()
        };
        assert!(render(&ch).is_none());
    }

    #[test]
    fn bgra_swaps_red_and_blue() {
        let ch = Crosshair {
            style: Style::Dot,
            center_dot_radius: 4.0,
            center_dot_color: Rgba8::new(255, 0, 0, 255),
            outline: false,
            ..Crosshair::default()
        };
        let r = render(&ch).unwrap();
        let bgra = r.bgra_premultiplied();
        let (ox, oy) = r.origin;
        let i = ((oy as u32 * r.width() + ox as u32) * 4) as usize;
        assert_eq!(&bgra[i..i + 4], &[0, 0, 255, 255], "BGRA beklenirdi");
    }

    #[test]
    fn icon_has_expected_size_and_content() {
        let px = icon_rgba(32);
        assert_eq!(px.len(), 32 * 32 * 4);
        assert!(
            px.chunks(4).any(|p| p[3] == 255 && p[1] > 200),
            "yeşil piksel yok"
        );
        assert_eq!(px[3], 0, "köşe saydam olmalı");
    }
}
