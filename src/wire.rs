//! Wireframe outlines of objects (world-space line segments) and their projection to the screen.

use crate::render::View;
use crate::scene::{Kind, Object};
use egui::{Pos2, Rect};
use glam::Vec3;
use std::f32::consts::TAU;

fn circle(segs: usize, y: f32, r: f32, out: &mut Vec<(Vec3, Vec3)>) {
    for i in 0..segs {
        let a0 = i as f32 / segs as f32 * TAU;
        let a1 = (i + 1) as f32 / segs as f32 * TAU;
        out.push((
            Vec3::new(a0.cos() * r, y, a0.sin() * r),
            Vec3::new(a1.cos() * r, y, a1.sin() * r),
        ));
    }
}

fn box_edges(lo: Vec3, hi: Vec3, out: &mut Vec<(Vec3, Vec3)>) {
    let c = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    };
    for (a, b) in [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        out.push((c(a), c(b)));
    }
}

/// Local-space segments of an object.
fn local_segments(o: &Object) -> Vec<(Vec3, Vec3)> {
    let mut s = Vec::new();
    match o.kind {
        Kind::Sphere => {
            for k in 1..6 {
                let y = -1.0 + k as f32 / 3.0;
                circle(20, y, (1.0 - y * y).max(0.0).sqrt(), &mut s);
            }
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::PI;
                for i in 0..20 {
                    let b0 = i as f32 / 20.0 * TAU;
                    let b1 = (i + 1) as f32 / 20.0 * TAU;
                    let p = |b: f32| Vec3::new(b.sin() * a.cos(), b.cos(), b.sin() * a.sin());
                    s.push((p(b0), p(b1)));
                }
            }
        }
        Kind::Cube => box_edges(Vec3::splat(-1.0), Vec3::splat(1.0), &mut s),
        Kind::Cylinder => {
            circle(20, 1.0, 1.0, &mut s);
            circle(20, -1.0, 1.0, &mut s);
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                s.push((
                    Vec3::new(a.cos(), -1.0, a.sin()),
                    Vec3::new(a.cos(), 1.0, a.sin()),
                ));
            }
        }
        Kind::Cone => {
            circle(20, -1.0, 1.0, &mut s);
            circle(16, 0.0, 0.5, &mut s);
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                s.push((Vec3::new(a.cos(), -1.0, a.sin()), Vec3::Y));
            }
        }
        Kind::Torus => {
            circle(24, 0.0, 1.0, &mut s);
            circle(24, 0.0, 0.4, &mut s);
            circle(24, 0.3, 0.7, &mut s);
            circle(24, -0.3, 0.7, &mut s);
            for i in 0..12 {
                let a = i as f32 / 12.0 * TAU;
                let c = Vec3::new(a.cos() * 0.7, 0.0, a.sin() * 0.7);
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                for j in 0..10 {
                    let b0 = j as f32 / 10.0 * TAU;
                    let b1 = (j + 1) as f32 / 10.0 * TAU;
                    let p = |b: f32| c + dir * b.cos() * 0.3 + Vec3::Y * b.sin() * 0.3;
                    s.push((p(b0), p(b1)));
                }
            }
        }
        Kind::Pyramid => {
            let b = [
                Vec3::new(-1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, -1.0),
                Vec3::new(1.0, -1.0, 1.0),
                Vec3::new(-1.0, -1.0, 1.0),
            ];
            for i in 0..4 {
                s.push((b[i], b[(i + 1) % 4]));
                s.push((b[i], Vec3::Y));
            }
        }
        Kind::Disk => {
            circle(24, 0.0, 1.0, &mut s);
            circle(16, 0.0, 0.5, &mut s);
        }
        Kind::Square | Kind::Picture => {
            let c = [
                Vec3::new(-1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(-1.0, 0.0, 1.0),
            ];
            for i in 0..4 {
                s.push((c[i], c[(i + 1) % 4]));
            }
            s.push((c[0], c[2]));
            s.push((c[1], c[3]));
        }
        Kind::Terrain => {
            if let Some(t) = &o.terrain {
                let g = 28usize;
                let p = |i: usize, j: usize| {
                    let u = i as f32 / g as f32;
                    let v = j as f32 / g as f32;
                    Vec3::new(u * 2.0 - 1.0, t.sample(u, v), v * 2.0 - 1.0)
                };
                for j in 0..=g {
                    for i in 0..g {
                        s.push((p(i, j), p(i + 1, j)));
                        s.push((p(j, i), p(j, i + 1)));
                    }
                }
            }
        }
        Kind::Stone | Kind::Mesh => {
            if let Some(m) = &o.mesh {
                let step = (m.tris.len() / 2500).max(1);
                for t in m.tris.iter().step_by(step) {
                    let a = m.verts[t[0] as usize];
                    let b = m.verts[t[1] as usize];
                    let c = m.verts[t[2] as usize];
                    s.push((a, b));
                    s.push((b, c));
                    s.push((c, a));
                }
            }
        }
        Kind::RadialLight => {
            for d in [
                Vec3::X,
                Vec3::Y,
                Vec3::Z,
                Vec3::new(0.7, 0.7, 0.0),
                Vec3::new(-0.7, 0.7, 0.0),
            ] {
                s.push((-d, d));
            }
            circle(12, 0.0, 0.5, &mut s);
        }
        Kind::SpotLight | Kind::ParallelLight => {
            circle(12, 0.0, 0.6, &mut s);
            let len = if o.kind == Kind::SpotLight { 3.0 } else { 4.0 };
            s.push((Vec3::ZERO, Vec3::NEG_Y * len));
            for i in 0..4 {
                let a = i as f32 / 4.0 * TAU;
                let spread = if o.kind == Kind::SpotLight {
                    (o.light.cone.to_radians()).tan() * len * 0.5
                } else {
                    0.6
                };
                s.push((
                    Vec3::new(a.cos() * 0.6, 0.0, a.sin() * 0.6),
                    Vec3::new(a.cos() * spread, -len, a.sin() * spread),
                ));
            }
        }
        Kind::Ground | Kind::Water | Kind::CloudPlane => {}
    }
    s
}

pub fn world_segments(o: &Object, cam_pos: Vec3) -> Vec<(Vec3, Vec3)> {
    if o.kind.is_infinite() {
        // a grid around the camera
        let y = o.pos.y;
        let step = if o.kind == Kind::CloudPlane {
            20.0
        } else {
            5.0
        };
        let n = 24;
        let cx = (cam_pos.x / step).round() * step;
        let cz = (cam_pos.z / step).round() * step;
        let ext = step * n as f32;
        let mut s = Vec::new();
        for i in -n..=n {
            let k = i as f32 * step;
            s.push((
                Vec3::new(cx - ext, y, cz + k),
                Vec3::new(cx + ext, y, cz + k),
            ));
            s.push((
                Vec3::new(cx + k, y, cz - ext),
                Vec3::new(cx + k, y, cz + ext),
            ));
        }
        return s;
    }
    let m = if o.kind.is_light() {
        glam::Mat4::from_rotation_translation(o.rotation(), o.pos)
    } else {
        o.matrix()
    };
    local_segments(o)
        .into_iter()
        .map(|(a, b)| (m.transform_point3(a), m.transform_point3(b)))
        .collect()
}

/// Project a world segment into the document rectangle. Returns None when fully behind the camera.
pub fn project_segment(view: &View, rect: Rect, a: Vec3, b: Vec3) -> Option<(Pos2, Pos2)> {
    let mut ca = view.to_cam(a);
    let mut cb = view.to_cam(b);
    let near = view.near();
    if ca.z < near && cb.z < near {
        return None;
    }
    if ca.z < near {
        let t = (near - ca.z) / (cb.z - ca.z);
        ca = ca + (cb - ca) * t;
    } else if cb.z < near {
        let t = (near - cb.z) / (ca.z - cb.z);
        cb = cb + (ca - cb) * t;
    }
    let to = |c: Vec3| {
        let (x, y) = view.cam_to_ndc(c);
        Pos2::new(
            rect.center().x + x * rect.width() * 0.5,
            rect.center().y - y * rect.height() * 0.5,
        )
    };
    let pa = to(ca);
    let pb = to(cb);
    let big = 1e5;
    if !pa.x.is_finite()
        || !pb.x.is_finite()
        || pa.x.abs() > big
        || pb.x.abs() > big
        || pa.y.abs() > big
        || pb.y.abs() > big
    {
        return None;
    }
    Some((pa, pb))
}

pub fn project_point(view: &View, rect: Rect, p: Vec3) -> Option<Pos2> {
    let c = view.to_cam(p);
    if c.z < view.near() {
        return None;
    }
    let (x, y) = view.cam_to_ndc(c);
    Some(Pos2::new(
        rect.center().x + x * rect.width() * 0.5,
        rect.center().y - y * rect.height() * 0.5,
    ))
}

#[allow(dead_code)]
pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}
