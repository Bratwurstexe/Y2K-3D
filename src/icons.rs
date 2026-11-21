//! Small hand-drawn icons for the palettes (no artwork taken from the original).

use crate::scene::Kind;
use egui::{epaint::PathShape, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

fn shade(base: [u8; 3], k: f32) -> Color32 {
    let f = |c: u8| ((c as f32 * k).clamp(0.0, 255.0)) as u8;
    Color32::from_rgb(f(base[0]), f(base[1]), f(base[2]))
}

fn poly(p: &Painter, pts: Vec<Pos2>, fill: Color32) {
    p.add(Shape::Path(PathShape::convex_polygon(
        pts,
        fill,
        Stroke::new(0.6, Color32::from_black_alpha(140)),
    )));
}

pub fn draw_kind_icon(p: &Painter, r: Rect, kind: Kind) {
    let c = r.center();
    let s = r.width().min(r.height()) * 0.36;
    let stone = [190, 185, 178];
    match kind {
        Kind::Sphere => {
            for i in 0..8 {
                let k = i as f32 / 8.0;
                p.circle_filled(
                    c + Vec2::new(-s * 0.35, -s * 0.35) * k,
                    s * (1.0 - k * 0.8),
                    shade(stone, 0.55 + k * 0.7),
                );
            }
        }
        Kind::Cube => {
            let a = Vec2::new(s * 0.9, s * 0.45);
            let top = vec![
                c + Vec2::new(0.0, -s),
                c + Vec2::new(a.x, -s + a.y),
                c + Vec2::new(0.0, -s + 2.0 * a.y),
                c + Vec2::new(-a.x, -s + a.y),
            ];
            let left = vec![
                top[3],
                top[2],
                top[2] + Vec2::new(0.0, s * 1.1),
                top[3] + Vec2::new(0.0, s * 1.1),
            ];
            let right = vec![
                top[2],
                top[1],
                top[1] + Vec2::new(0.0, s * 1.1),
                top[2] + Vec2::new(0.0, s * 1.1),
            ];
            poly(p, top, shade(stone, 1.25));
            poly(p, left, shade(stone, 0.8));
            poly(p, right, shade(stone, 0.55));
        }
        Kind::Cylinder => {
            let rr = Rect::from_center_size(c, Vec2::new(s * 1.6, s * 1.6));
            p.rect_filled(rr, 0.0, shade(stone, 0.75));
            p.add(Shape::ellipse_filled(
                Pos2::new(c.x, rr.top()),
                Vec2::new(s * 0.8, s * 0.3),
                shade(stone, 1.2),
            ));
            p.add(Shape::ellipse_filled(
                Pos2::new(c.x, rr.bottom()),
                Vec2::new(s * 0.8, s * 0.3),
                shade(stone, 0.75),
            ));
        }
        Kind::Cone => {
            poly(
                p,
                vec![
                    c + Vec2::new(0.0, -s),
                    c + Vec2::new(s * 0.85, s * 0.75),
                    c + Vec2::new(-s * 0.85, s * 0.75),
                ],
                shade(stone, 0.85),
            );
            p.add(Shape::ellipse_filled(
                c + Vec2::new(0.0, s * 0.75),
                Vec2::new(s * 0.85, s * 0.25),
                shade(stone, 0.65),
            ));
        }
        Kind::Torus => {
            p.add(Shape::ellipse_stroke(
                c,
                Vec2::new(s * 0.8, s * 0.45),
                Stroke::new(s * 0.45, shade(stone, 0.85)),
            ));
            p.add(Shape::ellipse_stroke(
                c + Vec2::new(0.0, -s * 0.06),
                Vec2::new(s * 0.8, s * 0.42),
                Stroke::new(s * 0.15, shade(stone, 1.2)),
            ));
        }
        Kind::Pyramid => {
            poly(
                p,
                vec![
                    c + Vec2::new(0.0, -s),
                    c + Vec2::new(-s, s * 0.7),
                    c + Vec2::new(0.0, s * 0.95),
                ],
                shade(stone, 1.0),
            );
            poly(
                p,
                vec![
                    c + Vec2::new(0.0, -s),
                    c + Vec2::new(0.0, s * 0.95),
                    c + Vec2::new(s, s * 0.7),
                ],
                shade(stone, 0.6),
            );
        }
        Kind::Disk => {
            p.add(Shape::ellipse_filled(
                c,
                Vec2::new(s, s * 0.4),
                shade(stone, 1.0),
            ));
        }
        Kind::Square => {
            poly(
                p,
                vec![
                    c + Vec2::new(-s, 0.0),
                    c + Vec2::new(0.0, -s * 0.5),
                    c + Vec2::new(s, 0.0),
                    c + Vec2::new(0.0, s * 0.5),
                ],
                shade(stone, 1.0),
            );
        }
        Kind::Terrain => {
            let base = c.y + s * 0.8;
            poly(
                p,
                vec![
                    Pos2::new(c.x - s * 1.1, base),
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.9),
                    Pos2::new(c.x + s * 0.3, base),
                ],
                Color32::from_rgb(150, 120, 90),
            );
            poly(
                p,
                vec![
                    Pos2::new(c.x - s * 0.2, base),
                    Pos2::new(c.x + s * 0.5, c.y - s * 0.4),
                    Pos2::new(c.x + s * 1.1, base),
                ],
                Color32::from_rgb(120, 100, 75),
            );
            poly(
                p,
                vec![
                    Pos2::new(c.x - s * 0.52, c.y - s * 0.45),
                    Pos2::new(c.x - s * 0.3, c.y - s * 0.9),
                    Pos2::new(c.x - s * 0.08, c.y - s * 0.45),
                ],
                Color32::from_rgb(240, 240, 245),
            );
        }
        Kind::Stone => {
            p.add(Shape::ellipse_filled(
                c + Vec2::new(0.0, s * 0.2),
                Vec2::new(s, s * 0.65),
                shade([140, 128, 115], 1.0),
            ));
            p.add(Shape::ellipse_filled(
                c + Vec2::new(-s * 0.25, 0.0),
                Vec2::new(s * 0.45, s * 0.3),
                shade([140, 128, 115], 1.3),
            ));
        }
        Kind::Mesh => {
            let pts = [
                c + Vec2::new(-s, s * 0.6),
                c + Vec2::new(0.0, -s),
                c + Vec2::new(s, s * 0.6),
                c + Vec2::new(0.0, s * 0.1),
            ];
            for (a, b) in [(0, 1), (1, 2), (2, 0), (0, 3), (1, 3), (2, 3)] {
                p.line_segment(
                    [pts[a], pts[b]],
                    Stroke::new(1.4, Color32::from_rgb(170, 210, 160)),
                );
            }
        }
        Kind::Ground => {
            poly(
                p,
                vec![
                    c + Vec2::new(-s * 1.2, s * 0.1),
                    c + Vec2::new(s * 1.2, s * 0.1),
                    c + Vec2::new(s * 1.2, s * 0.8),
                    c + Vec2::new(-s * 1.2, s * 0.8),
                ],
                Color32::from_rgb(120, 105, 80),
            );
            p.rect_filled(
                Rect::from_min_max(
                    c + Vec2::new(-s * 1.2, -s * 0.8),
                    c + Vec2::new(s * 1.2, s * 0.1),
                ),
                0.0,
                Color32::from_rgb(110, 150, 200),
            );
        }
        Kind::Water => {
            p.rect_filled(
                Rect::from_min_max(
                    c + Vec2::new(-s * 1.2, -s * 0.8),
                    c + Vec2::new(s * 1.2, s * 0.8),
                ),
                0.0,
                Color32::from_rgb(40, 90, 140),
            );
            for k in 0..3 {
                let y = c.y - s * 0.4 + k as f32 * s * 0.4;
                p.line_segment(
                    [Pos2::new(c.x - s, y), Pos2::new(c.x + s, y)],
                    Stroke::new(1.0, Color32::from_rgb(150, 200, 235)),
                );
            }
        }
        Kind::CloudPlane => {
            p.rect_filled(
                Rect::from_min_max(
                    c + Vec2::new(-s * 1.2, -s * 0.8),
                    c + Vec2::new(s * 1.2, s * 0.8),
                ),
                0.0,
                Color32::from_rgb(90, 130, 190),
            );
            for (dx, dy, rr) in [(-0.4, 0.1, 0.4), (0.1, -0.1, 0.5), (0.55, 0.15, 0.35)] {
                p.circle_filled(
                    c + Vec2::new(dx * s, dy * s),
                    rr * s,
                    Color32::from_rgb(245, 245, 250),
                );
            }
        }
        Kind::Picture => {
            p.rect_filled(
                Rect::from_center_size(c, Vec2::new(s * 1.8, s * 1.5)),
                1.0,
                Color32::from_rgb(230, 225, 210),
            );
            p.rect_filled(
                Rect::from_center_size(c, Vec2::new(s * 1.4, s * 1.1)),
                0.0,
                Color32::from_rgb(90, 140, 190),
            );
            poly(
                p,
                vec![
                    c + Vec2::new(-s * 0.7, s * 0.55),
                    c + Vec2::new(-s * 0.1, -s * 0.2),
                    c + Vec2::new(s * 0.5, s * 0.55),
                ],
                Color32::from_rgb(80, 120, 60),
            );
        }
        Kind::RadialLight | Kind::SpotLight | Kind::ParallelLight => {
            let col = Color32::from_rgb(255, 225, 120);
            p.circle_filled(c, s * 0.45, col);
            match kind {
                Kind::RadialLight => {
                    for i in 0..8 {
                        let a = i as f32 / 8.0 * std::f32::consts::TAU;
                        let d = Vec2::new(a.cos(), a.sin());
                        p.line_segment([c + d * s * 0.6, c + d * s * 1.0], Stroke::new(1.5, col));
                    }
                }
                Kind::SpotLight => {
                    poly(
                        p,
                        vec![
                            c,
                            c + Vec2::new(-s * 0.7, s * 1.0),
                            c + Vec2::new(s * 0.7, s * 1.0),
                        ],
                        Color32::from_rgba_unmultiplied(255, 225, 120, 110),
                    );
                }
                _ => {
                    for k in -1..=1 {
                        let x = c.x + k as f32 * s * 0.5;
                        p.line_segment(
                            [Pos2::new(x, c.y + s * 0.5), Pos2::new(x, c.y + s * 1.0)],
                            Stroke::new(1.5, col),
                        );
                    }
                }
            }
        }
    }
}

/// A palette button: square, icon drawn by `draw`, hover text.
pub fn icon_button(
    ui: &mut egui::Ui,
    size: f32,
    selected: bool,
    tip: &str,
    draw: impl FnOnce(&Painter, Rect),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    let p = ui.painter_at(rect);
    let bg = if selected {
        Color32::from_rgb(120, 82, 45)
    } else if resp.hovered() {
        Color32::from_rgb(78, 78, 84)
    } else {
        Color32::from_rgb(56, 56, 61)
    };
    p.rect_filled(rect.shrink(1.0), 4.0, bg);
    p.rect_filled(
        Rect::from_min_size(
            rect.min + Vec2::splat(1.0),
            Vec2::new(rect.width() - 2.0, rect.height() * 0.35),
        ),
        4.0,
        Color32::from_white_alpha(10),
    );
    draw(&p, rect.shrink(3.0));
    resp.on_hover_text(tip)
}
