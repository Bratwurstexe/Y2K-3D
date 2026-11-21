//! Ray tracer: sky & fog, primitives, terrains, meshes, materials, lights and the progressive renderer.

use crate::mesh::{Aabb, Bvh, TriMesh};
use crate::noise::{fbm, hash2, noise};
use crate::scene::{Camera, Kind, Material, Scene, Sky, TexKind};
use crate::terrain::HeightMap;
use glam::{Mat3, Mat4, Vec3};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

// ---------------------------------------------------------------------------------------------
// View (shared by the wireframe and the renderer)

#[derive(Clone, Copy)]
pub struct View {
    pub pos: Vec3,
    pub f: Vec3,
    pub r: Vec3,
    pub u: Vec3,
    pub tan_half: f32,
    pub aspect: f32,
    pub ortho: Option<f32>,
}

impl View {
    pub fn from_camera(c: &Camera, aspect: f32) -> View {
        let (f, r, u) = c.basis();
        View {
            pos: c.pos,
            f,
            r,
            u,
            tan_half: (c.fov.to_radians() * 0.5).tan(),
            aspect,
            ortho: None,
        }
    }

    /// Orthographic view looking along `dir`, centred on `center`, showing `half_width` units.
    pub fn ortho(center: Vec3, dir: Vec3, half_width: f32, aspect: f32) -> View {
        let f = dir.normalize();
        let up_hint = if f.y.abs() > 0.99 {
            Vec3::NEG_Z
        } else {
            Vec3::Y
        };
        let r = f.cross(up_hint).normalize();
        let u = r.cross(f).normalize();
        View {
            pos: center - f * 500.0,
            f,
            r,
            u,
            tan_half: 1.0,
            aspect,
            ortho: Some(half_width),
        }
    }

    /// sx, sy in -1..1 (sy up). Returns origin and unit direction.
    pub fn ray(&self, sx: f32, sy: f32) -> (Vec3, Vec3) {
        match self.ortho {
            Some(hw) => (
                self.pos + self.r * sx * hw + self.u * sy * hw / self.aspect,
                self.f,
            ),
            None => {
                let d = self.f
                    + self.r * (sx * self.tan_half)
                    + self.u * (sy * self.tan_half / self.aspect);
                (self.pos, d.normalize())
            }
        }
    }

    /// Camera-space coordinates (x right, y up, z forward).
    pub fn to_cam(&self, p: Vec3) -> Vec3 {
        let d = p - self.pos;
        Vec3::new(d.dot(self.r), d.dot(self.u), d.dot(self.f))
    }

    /// Camera space to normalised screen coordinates (-1..1, y up).
    pub fn cam_to_ndc(&self, c: Vec3) -> (f32, f32) {
        match self.ortho {
            Some(hw) => (c.x / hw, c.y * self.aspect / hw),
            None => (
                c.x / (c.z * self.tan_half),
                c.y * self.aspect / (c.z * self.tan_half),
            ),
        }
    }

    pub fn near(&self) -> f32 {
        if self.ortho.is_some() {
            -1e6
        } else {
            0.05
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Caches for prepared data (keyed by the shared data's address)

struct PTerrain {
    map: Arc<HeightMap>,
    cellmax: Vec<f32>,
    blockmax: Vec<f32>,
    nb: usize,
    normals: Vec<Vec3>,
}

const BLOCK: usize = 8;

impl PTerrain {
    fn new(map: Arc<HeightMap>) -> PTerrain {
        let n = map.size;
        let c = n - 1;
        let mut cellmax = vec![0.0; c * c];
        for j in 0..c {
            for i in 0..c {
                cellmax[j * c + i] = map
                    .get(i, j)
                    .max(map.get(i + 1, j))
                    .max(map.get(i, j + 1))
                    .max(map.get(i + 1, j + 1));
            }
        }
        let nb = c.div_ceil(BLOCK);
        let mut blockmax = vec![0.0f32; nb * nb];
        for j in 0..c {
            for i in 0..c {
                let b = (j / BLOCK) * nb + i / BLOCK;
                blockmax[b] = blockmax[b].max(cellmax[j * c + i]);
            }
        }
        let step = 2.0 / c as f32;
        let mut normals = vec![Vec3::Y; n * n];
        for j in 0..n {
            for i in 0..n {
                let hx = (map.get_clamped(i as i32 + 1, j as i32)
                    - map.get_clamped(i as i32 - 1, j as i32))
                    / (2.0 * step);
                let hz = (map.get_clamped(i as i32, j as i32 + 1)
                    - map.get_clamped(i as i32, j as i32 - 1))
                    / (2.0 * step);
                normals[j * n + i] = Vec3::new(-hx, 1.0, -hz);
            }
        }
        PTerrain {
            map,
            cellmax,
            blockmax,
            nb,
            normals,
        }
    }
}

type Cache<T, K> = Mutex<HashMap<usize, (Weak<K>, Arc<T>)>>;

fn terrain_cache() -> &'static Cache<PTerrain, HeightMap> {
    static C: OnceLock<Cache<PTerrain, HeightMap>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}
fn bvh_cache() -> &'static Cache<Bvh, TriMesh> {
    static C: OnceLock<Cache<Bvh, TriMesh>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_terrain(m: &Arc<HeightMap>) -> Arc<PTerrain> {
    let key = Arc::as_ptr(m) as usize;
    let mut c = terrain_cache().lock().unwrap();
    if let Some((w, p)) = c.get(&key) {
        if w.upgrade().is_some() {
            return p.clone();
        }
    }
    c.retain(|_, (w, _)| w.upgrade().is_some());
    let p = Arc::new(PTerrain::new(m.clone()));
    c.insert(key, (Arc::downgrade(m), p.clone()));
    p
}

fn cached_bvh(m: &Arc<TriMesh>) -> Arc<Bvh> {
    let key = Arc::as_ptr(m) as usize;
    let mut c = bvh_cache().lock().unwrap();
    if let Some((w, p)) = c.get(&key) {
        if w.upgrade().is_some() {
            return p.clone();
        }
    }
    c.retain(|_, (w, _)| w.upgrade().is_some());
    let p = Arc::new(Bvh::build(m));
    c.insert(key, (Arc::downgrade(m), p.clone()));
    p
}

pub type Picture = Arc<image::RgbaImage>;

pub fn load_picture(path: &str) -> Option<Picture> {
    static C: OnceLock<Mutex<HashMap<String, Option<Picture>>>> = OnceLock::new();
    let c = C.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(p) = c.lock().unwrap().get(path) {
        return p.clone();
    }
    let img = crate::picture::open(path)
        .ok()
        .map(|i| Arc::new(i.to_rgba8()));
    c.lock().unwrap().insert(path.to_string(), img.clone());
    img
}

fn sample_picture(img: &image::RgbaImage, u: f32, v: f32) -> [f32; 4] {
    let (w, h) = img.dimensions();
    let x = ((u.rem_euclid(1.0)) * w as f32) as u32;
    let y = ((v.rem_euclid(1.0)) * h as f32) as u32;
    let p = img.get_pixel(x.min(w - 1), y.min(h - 1)).0;
    [
        p[0] as f32 / 255.0,
        p[1] as f32 / 255.0,
        p[2] as f32 / 255.0,
        p[3] as f32 / 255.0,
    ]
}

// ---------------------------------------------------------------------------------------------
// Prepared scene

struct PObj {
    kind: Kind,
    inv: Mat4,
    nmat: Mat3,
    mat: Material,
    bbox: Aabb,
    plane_y: f32,
    terrain: Option<Arc<PTerrain>>,
    bvh: Option<Arc<Bvh>>,
    picture: Option<Picture>,
    scale_y: f32,
    pos_y: f32,
}

enum LightKind {
    Point,
    Spot {
        dir: Vec3,
        cos_outer: f32,
        cos_inner: f32,
    },
    Parallel {
        dir: Vec3,
    },
}

struct PLight {
    kind: LightKind,
    pos: Vec3,
    color: Vec3,
    falloff: bool,
}

pub struct Prepared {
    objs: Vec<PObj>,
    finite: Vec<usize>,
    infinite: Vec<usize>,
    lights: Vec<PLight>,
    sky: Sky,
    sun: Vec3,
    pub view: View,
    max_depth: u32,
}

#[derive(Clone, Copy)]
pub struct Hit {
    pub t: f32,
    pub n: Vec3,
    pub obj: usize,
    pub local: Vec3,
    pub ln: Vec3,
}

fn v3(c: [f32; 3]) -> Vec3 {
    Vec3::from(c)
}

pub fn prepare(scene: &Scene, view: View, solo: Option<u64>) -> Prepared {
    prepare_with_ids(scene, view, solo).0
}

/// Also returns the scene index of every prepared object (for picking).
pub fn prepare_with_ids(scene: &Scene, view: View, solo: Option<u64>) -> (Prepared, Vec<usize>) {
    let mut objs = Vec::new();
    let mut ids = Vec::new();
    let mut lights = Vec::new();
    for (si, o) in scene.objects.iter().enumerate() {
        if o.hidden {
            continue;
        }
        if let Some(s) = solo {
            if o.id != s && !o.kind.is_light() {
                continue;
            }
        }
        if o.kind.is_light() {
            let color = v3(o.light.color) * o.light.intensity;
            let kind = match o.kind {
                Kind::SpotLight => {
                    let c = o.light.cone.clamp(1.0, 89.0).to_radians();
                    LightKind::Spot {
                        dir: o.light_dir(),
                        cos_outer: c.cos(),
                        cos_inner: (c * 0.75).cos(),
                    }
                }
                Kind::ParallelLight => LightKind::Parallel { dir: o.light_dir() },
                _ => LightKind::Point,
            };
            lights.push(PLight {
                kind,
                pos: o.pos,
                color,
                falloff: o.light.falloff,
            });
            continue;
        }
        let m = o.matrix();
        let inv = m.inverse();
        let nmat = Mat3::from_mat4(inv).transpose();
        // world bounding box of the local -1..1 cube (terrain: y 0..1)
        let mut bbox = Aabb::empty();
        let (ylo, yhi) = if o.kind == Kind::Terrain {
            (0.0, 1.0)
        } else {
            (-1.0, 1.0)
        };
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { -1.0 } else { 1.0 },
                if i & 2 == 0 { ylo } else { yhi },
                if i & 4 == 0 { -1.0 } else { 1.0 },
            );
            bbox.grow(m.transform_point3(c));
        }
        bbox.lo -= Vec3::splat(1e-3);
        bbox.hi += Vec3::splat(1e-3);
        let terrain = if o.kind == Kind::Terrain {
            o.terrain.as_ref().map(cached_terrain)
        } else {
            None
        };
        let bvh = if matches!(o.kind, Kind::Stone | Kind::Mesh) {
            o.mesh.as_ref().map(cached_bvh)
        } else {
            None
        };
        let picture = if o.material.texture == TexKind::Image {
            o.material.image.as_deref().and_then(load_picture)
        } else {
            None
        };
        objs.push(PObj {
            kind: o.kind,
            inv,
            nmat,
            mat: o.material.clone(),
            bbox,
            plane_y: o.pos.y,
            terrain,
            bvh,
            picture,
            scale_y: o.scale.y,
            pos_y: o.pos.y,
        });
        ids.push(si);
    }
    let finite = (0..objs.len())
        .filter(|&i| !objs[i].kind.is_infinite())
        .collect();
    let infinite = (0..objs.len())
        .filter(|&i| objs[i].kind.is_infinite())
        .collect();
    (
        Prepared {
            objs,
            finite,
            infinite,
            lights,
            sky: scene.sky.clone(),
            sun: scene.sky.sun_dir(),
            view,
            max_depth: scene.render.max_depth.clamp(1, 8),
        },
        ids,
    )
}

// ---------------------------------------------------------------------------------------------
// Local-space intersections. `d` is not normalised; t is the world ray parameter.

fn solve_quadratic(a: f32, b: f32, c: f32) -> Option<(f32, f32)> {
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 || a.abs() < 1e-12 {
        return None;
    }
    let s = disc.sqrt();
    let q = if b < 0.0 {
        -0.5 * (b - s)
    } else {
        -0.5 * (b + s)
    };
    let t0 = q / a;
    let t1 = if q.abs() > 1e-12 { c / q } else { t0 };
    Some((t0.min(t1), t0.max(t1)))
}

const EPS: f32 = 1e-4;

fn convex(o: Vec3, d: Vec3, planes: &[(Vec3, f32)], tmax: f32) -> Option<(f32, Vec3)> {
    let mut tn = f32::MIN;
    let mut tf = f32::MAX;
    let mut nn = Vec3::Y;
    let mut nf = Vec3::Y;
    for &(n, c) in planes {
        let denom = n.dot(d);
        let dist = c - n.dot(o);
        if denom.abs() < 1e-12 {
            if dist < 0.0 {
                return None;
            }
            continue;
        }
        let t = dist / denom;
        if denom < 0.0 {
            if t > tn {
                tn = t;
                nn = n;
            }
        } else if t < tf {
            tf = t;
            nf = n;
        }
    }
    if tn > tf {
        return None;
    }
    if tn > EPS && tn < tmax {
        Some((tn, nn))
    } else if tf > EPS && tf < tmax {
        Some((tf, nf))
    } else {
        None
    }
}

fn cube_planes() -> [(Vec3, f32); 6] {
    [
        (Vec3::X, 1.0),
        (Vec3::NEG_X, 1.0),
        (Vec3::Y, 1.0),
        (Vec3::NEG_Y, 1.0),
        (Vec3::Z, 1.0),
        (Vec3::NEG_Z, 1.0),
    ]
}

fn pyramid_planes() -> [(Vec3, f32); 5] {
    let k = 1.0 / 5f32.sqrt();
    [
        (Vec3::new(2.0, 1.0, 0.0) * k, k),
        (Vec3::new(-2.0, 1.0, 0.0) * k, k),
        (Vec3::new(0.0, 1.0, 2.0) * k, k),
        (Vec3::new(0.0, 1.0, -2.0) * k, k),
        (Vec3::NEG_Y, 1.0),
    ]
}

fn hit_cylinder(o: Vec3, d: Vec3, tmax: f32, cone: bool) -> Option<(f32, Vec3)> {
    let mut best: Option<(f32, Vec3)> = None;
    let consider = |t: f32, n: Vec3, best: &mut Option<(f32, Vec3)>| {
        if t > EPS && t < tmax && best.map(|b| t < b.0).unwrap_or(true) {
            *best = Some((t, n));
        }
    };
    if cone {
        // x^2 + z^2 = ((1 - y) / 2)^2
        let k = 0.25;
        let a = d.x * d.x + d.z * d.z - k * d.y * d.y;
        let b = 2.0 * (o.x * d.x + o.z * d.z) + 2.0 * k * (1.0 - o.y) * d.y;
        let c = o.x * o.x + o.z * o.z - k * (1.0 - o.y) * (1.0 - o.y);
        if let Some((t0, t1)) = solve_quadratic(a, b, c) {
            for t in [t0, t1] {
                let p = o + d * t;
                if (-1.0..=1.0).contains(&p.y) {
                    let n = Vec3::new(p.x, 0.5 * (1.0 - p.y) * 0.5, p.z);
                    consider(t, n.normalize_or(Vec3::Y), &mut best);
                }
            }
        }
    } else {
        let a = d.x * d.x + d.z * d.z;
        let b = 2.0 * (o.x * d.x + o.z * d.z);
        let c = o.x * o.x + o.z * o.z - 1.0;
        if let Some((t0, t1)) = solve_quadratic(a, b, c) {
            for t in [t0, t1] {
                let p = o + d * t;
                if (-1.0..=1.0).contains(&p.y) {
                    consider(t, Vec3::new(p.x, 0.0, p.z), &mut best);
                }
            }
        }
        if d.y.abs() > 1e-12 {
            let t = (1.0 - o.y) / d.y;
            let p = o + d * t;
            if p.x * p.x + p.z * p.z <= 1.0 {
                consider(t, Vec3::Y, &mut best);
            }
        }
    }
    if d.y.abs() > 1e-12 {
        let t = (-1.0 - o.y) / d.y;
        let p = o + d * t;
        if p.x * p.x + p.z * p.z <= 1.0 {
            consider(t, Vec3::NEG_Y, &mut best);
        }
    }
    best
}

fn hit_torus(o: Vec3, d: Vec3, tmax: f32) -> Option<(f32, Vec3)> {
    const R: f32 = 0.7;
    const RR: f32 = 0.3;
    let bb = Aabb {
        lo: Vec3::new(-1.0, -RR, -1.0),
        hi: Vec3::new(1.0, RR, 1.0),
    };
    let len = d.length();
    let dn = d / len;
    let t0 = bb.hit(o, Vec3::ONE / dn, tmax * len)?;
    let mut t = t0;
    for _ in 0..200 {
        let p = o + dn * t;
        let q = (p.x * p.x + p.z * p.z).sqrt() - R;
        let sd = (q * q + p.y * p.y).sqrt() - RR;
        if sd < 1e-4 {
            let rxz = (p.x * p.x + p.z * p.z).sqrt().max(1e-6);
            let c = Vec3::new(p.x / rxz * R, 0.0, p.z / rxz * R);
            let tw = t / len;
            if tw > EPS {
                return Some((tw, (p - c).normalize()));
            }
            t += 0.01;
            continue;
        }
        t += sd * 0.95;
        if t > tmax * len || t > 6.0 + t0 {
            break;
        }
    }
    None
}

fn hit_terrain(pt: &PTerrain, o: Vec3, d: Vec3, tmax: f32) -> Option<(f32, Vec3)> {
    let n = pt.map.size;
    let c = (n - 1) as f32;
    let s = 0.5 * c;
    // grid space
    let go = Vec3::new((o.x + 1.0) * s, o.y, (o.z + 1.0) * s);
    let gd = Vec3::new(d.x * s, d.y, d.z * s);
    let bb = Aabb {
        lo: Vec3::new(0.0, -1e-3, 0.0),
        hi: Vec3::new(c, 1.0 + 1e-3, c),
    };
    let inv = Vec3::ONE / gd;
    let tn = bb.hit(go, inv, tmax)?;
    let t1 = {
        let a = (bb.lo - go) * inv;
        let b = (bb.hi - go) * inv;
        a.max(b).min_element().min(tmax)
    };
    let nc = n - 1;
    let mut best: Option<(f32, Vec3)> = None;
    let fine = |ta: f32, tb: f32, best: &mut Option<(f32, Vec3)>, bx: usize, bz: usize| -> bool {
        // DDA over cells inside block (bx, bz) between ta..tb
        let x_lo = (bx * BLOCK) as i32;
        let z_lo = (bz * BLOCK) as i32;
        let x_hi = ((bx + 1) * BLOCK).min(nc) as i32 - 1;
        let z_hi = ((bz + 1) * BLOCK).min(nc) as i32 - 1;
        let mut any = false;
        dda(go, gd, ta, tb, 1.0, |cx, cz, ca, cb| {
            if cx < x_lo || cz < z_lo || cx > x_hi || cz > z_hi {
                return false;
            }
            let (cx, cz) = (cx as usize, cz as usize);
            let ya = go.y + gd.y * ca;
            let yb = go.y + gd.y * cb;
            if ya.min(yb) > pt.cellmax[cz * nc + cx] {
                return false;
            }
            let p = |i: usize, j: usize| Vec3::new(i as f32, pt.map.get(i, j), j as f32);
            let tris = [
                [(cx, cz), (cx + 1, cz), (cx, cz + 1)],
                [(cx + 1, cz), (cx + 1, cz + 1), (cx, cz + 1)],
            ];
            let mut found = false;
            for tri in tris {
                let a = p(tri[0].0, tri[0].1);
                let e1 = p(tri[1].0, tri[1].1) - a;
                let e2 = p(tri[2].0, tri[2].1) - a;
                let pv = gd.cross(e2);
                let det = e1.dot(pv);
                if det.abs() < 1e-12 {
                    continue;
                }
                let id = 1.0 / det;
                let tv = go - a;
                let u = tv.dot(pv) * id;
                if !(-1e-5..=1.00001).contains(&u) {
                    continue;
                }
                let qv = tv.cross(e1);
                let v = gd.dot(qv) * id;
                if v < -1e-5 || u + v > 1.00001 {
                    continue;
                }
                let t = e2.dot(qv) * id;
                if t > EPS && t < tmax && best.map(|b| t < b.0).unwrap_or(true) {
                    let nrm = |ij: (usize, usize)| pt.normals[ij.1 * n + ij.0];
                    let nl = nrm(tri[0]) * (1.0 - u - v) + nrm(tri[1]) * u + nrm(tri[2]) * v;
                    *best = Some((t, nl.normalize_or(Vec3::Y)));
                    found = true;
                }
            }
            any |= found;
            found
        });
        any
    };
    let bsz = BLOCK as f32;
    dda(go, gd, tn, t1, bsz, |bx, bz, ta, tb| {
        if bx < 0 || bz < 0 || bx as usize >= pt.nb || bz as usize >= pt.nb {
            return false;
        }
        let ya = go.y + gd.y * ta;
        let yb = go.y + gd.y * tb;
        if ya.min(yb) > pt.blockmax[bz as usize * pt.nb + bx as usize] {
            return false;
        }
        fine(ta, tb, &mut best, bx as usize, bz as usize)
    });
    best
}

/// 2D DDA over a grid of `cell`-sized squares in the XZ plane. The visitor returns true to stop.
fn dda(
    o: Vec3,
    d: Vec3,
    t0: f32,
    t1: f32,
    cell: f32,
    mut visit: impl FnMut(i32, i32, f32, f32) -> bool,
) {
    let p = o + d * t0;
    let mut cx = (p.x / cell).floor() as i32;
    let mut cz = (p.z / cell).floor() as i32;
    let step_x = if d.x > 0.0 { 1 } else { -1 };
    let step_z = if d.z > 0.0 { 1 } else { -1 };
    let next_bound = |c: i32, step: i32| (c + if step > 0 { 1 } else { 0 }) as f32 * cell;
    let mut tmx = if d.x.abs() > 1e-12 {
        (next_bound(cx, step_x) - o.x) / d.x
    } else {
        f32::MAX
    };
    let mut tmz = if d.z.abs() > 1e-12 {
        (next_bound(cz, step_z) - o.z) / d.z
    } else {
        f32::MAX
    };
    let dtx = if d.x.abs() > 1e-12 {
        cell / d.x.abs()
    } else {
        f32::MAX
    };
    let dtz = if d.z.abs() > 1e-12 {
        cell / d.z.abs()
    } else {
        f32::MAX
    };
    let mut t = t0;
    for _ in 0..100_000 {
        let tn = tmx.min(tmz).min(t1);
        if visit(cx, cz, t, tn) {
            return;
        }
        if tn >= t1 {
            return;
        }
        if tmx < tmz {
            cx += step_x;
            t = tmx;
            tmx += dtx;
        } else {
            cz += step_z;
            t = tmz;
            tmz += dtz;
        }
    }
}

impl Prepared {
    fn hit_obj(&self, i: usize, o: Vec3, d: Vec3, tmax: f32) -> Option<Hit> {
        let ob = &self.objs[i];
        if ob.kind.is_infinite() {
            if d.y.abs() < 1e-9 {
                return None;
            }
            let t = (ob.plane_y - o.y) / d.y;
            if t > EPS && t < tmax {
                let p = o + d * t;
                let n = if d.y < 0.0 { Vec3::Y } else { Vec3::NEG_Y };
                return Some(Hit {
                    t,
                    n,
                    obj: i,
                    local: p,
                    ln: Vec3::Y,
                });
            }
            return None;
        }
        if ob.bbox.hit(o, Vec3::ONE / d, tmax).is_none() {
            return None;
        }
        let lo = ob.inv.transform_point3(o);
        let ld = ob.inv.transform_vector3(d);
        let r = match ob.kind {
            Kind::Sphere => solve_quadratic(ld.dot(ld), 2.0 * lo.dot(ld), lo.dot(lo) - 1.0)
                .and_then(|(t0, t1)| {
                    let t = if t0 > EPS { t0 } else { t1 };
                    (t > EPS && t < tmax).then(|| (t, lo + ld * t))
                }),
            Kind::Cube => convex(lo, ld, &cube_planes(), tmax),
            Kind::Pyramid => convex(lo, ld, &pyramid_planes(), tmax),
            Kind::Cylinder => hit_cylinder(lo, ld, tmax, false),
            Kind::Cone => hit_cylinder(lo, ld, tmax, true),
            Kind::Torus => hit_torus(lo, ld, tmax),
            Kind::Disk | Kind::Square | Kind::Picture => {
                if ld.y.abs() < 1e-12 {
                    None
                } else {
                    let t = -lo.y / ld.y;
                    let p = lo + ld * t;
                    let inside = if ob.kind == Kind::Disk {
                        p.x * p.x + p.z * p.z <= 1.0
                    } else {
                        p.x.abs() <= 1.0 && p.z.abs() <= 1.0
                    };
                    let mut ok = inside && t > EPS && t < tmax;
                    if ok && ob.kind == Kind::Picture {
                        if let Some(img) = &ob.picture {
                            ok = sample_picture(img, (p.x + 1.0) * 0.5, (p.z + 1.0) * 0.5)[3] > 0.5;
                        }
                    }
                    ok.then_some((t, Vec3::Y))
                }
            }
            Kind::Terrain => ob
                .terrain
                .as_ref()
                .and_then(|pt| hit_terrain(pt, lo, ld, tmax)),
            Kind::Stone | Kind::Mesh => ob.bvh.as_ref().and_then(|b| {
                let len = ld.length();
                b.intersect(lo, ld / len, tmax * len)
                    .map(|(t, n)| (t / len, n))
            }),
            _ => None,
        }?;
        let (t, ln) = r;
        let local = lo + ld * t;
        let mut n = (ob.nmat * ln).normalize_or(Vec3::Y);
        if matches!(ob.kind, Kind::Disk | Kind::Square | Kind::Picture) && n.dot(d) > 0.0 {
            n = -n;
        }
        Some(Hit {
            t,
            n,
            obj: i,
            local,
            ln,
        })
    }

    pub fn intersect(&self, o: Vec3, d: Vec3, tmax: f32) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        let mut tm = tmax;
        for &i in &self.finite {
            if let Some(h) = self.hit_obj(i, o, d, tm) {
                tm = h.t;
                best = Some(h);
            }
        }
        // planes lose ties against objects resting on them
        let tm = if best.is_some() { tm - 2e-3 } else { tm };
        let mut tm2 = tm;
        for &i in &self.infinite {
            if let Some(h) = self.hit_obj(i, o, d, tm2) {
                tm2 = h.t;
                best = Some(h);
            }
        }
        best
    }

    /// Light that gets through to `p` along `dir` (1 = unshadowed).
    fn transmit(&self, p: Vec3, dir: Vec3, dist: f32) -> f32 {
        let mut tr = 1.0;
        for &i in self.finite.iter().chain(self.infinite.iter()) {
            let ob = &self.objs[i];
            if ob.kind == Kind::CloudPlane {
                continue;
            }
            if self.hit_obj(i, p, dir, dist).is_some() {
                tr *= ob.mat.transparency * 0.85;
                if tr < 0.01 {
                    return 0.0;
                }
            }
        }
        tr
    }

    // -----------------------------------------------------------------------------------------
    // Sky

    pub fn sky_color(&self, d: Vec3) -> Vec3 {
        sky_color(&self.sky, self.sun, d)
    }

    fn atmosphere(&self, col: Vec3, o: Vec3, d: Vec3, dist: f32) -> Vec3 {
        let s = &self.sky;
        let mut c = col;
        if s.fog > 0.0 {
            let a = 1.0 / s.fog_height.max(0.1);
            let y0 = o.y;
            let y1 = o.y + d.y * dist;
            let dens = |y: f32| (-(y.max(0.0)) * a).exp();
            let avg = if (y1 - y0).abs() < 1e-3 {
                dens(y0)
            } else {
                (dens(y0.max(0.0)) - dens(y1.max(0.0))).abs() / (a * (y1 - y0).abs()).max(1e-6)
            };
            let avg = if y0 < 0.0 && y1 < 0.0 {
                1.0
            } else {
                avg.min(1.0)
            };
            let f = 1.0 - (-dist * s.fog * avg * 0.12).exp();
            let fc = v3(s.fog_color) * (0.4 + 0.6 * self.sun.y.max(0.0).sqrt().max(0.3));
            c = c.lerp(fc, f);
        }
        if s.haze > 0.0 {
            let f = 1.0 - (-dist * s.haze * 0.012).exp();
            c = c.lerp(v3(s.haze_color) * self.daylight(), f);
        }
        c
    }

    fn daylight(&self) -> f32 {
        (self.sun.y * 3.0 + 0.6).clamp(0.25, 1.0)
    }

    // -----------------------------------------------------------------------------------------
    // Shading

    pub fn trace(&self, o: Vec3, d: Vec3, depth: u32) -> Vec3 {
        match self.intersect(o, d, 1e6) {
            None => self.sky_color(d),
            Some(h) => {
                let c = self.shade(o, d, &h, depth);
                self.atmosphere(c, o, d, h.t.min(5000.0))
            }
        }
    }

    fn shade(&self, o: Vec3, d: Vec3, h: &Hit, depth: u32) -> Vec3 {
        let ob = &self.objs[h.obj];
        let m = &ob.mat;
        let p = o + d * h.t;
        let (mut base, alpha) = texture(m, ob, p, h, ob.picture.as_deref());
        if m.texture == TexKind::Iridescent {
            base = iridescent(m, (-d).dot(h.n).abs(), p);
        }
        let mut n = h.n;
        let geo_front = n.dot(d) < 0.0;
        if m.bump > 0.0 {
            n = bump(m, ob, p, h, n);
        }
        let ns = if n.dot(d) > 0.0 && !geo_front { -n } else { n };
        let view = -d;
        let sky = &self.sky;
        let ambience = (m.ambience + 0.25).min(1.5);
        let mut col = (v3(sky.ambient) * base * 0.75 + v3(m.ambient) * 0.25) * ambience;
        let spec_col = v3(m.specular).lerp(base, m.metallicity);
        let shininess = 8.0 + 250.0 * m.specularity * m.specularity;
        let po = p + ns * 2e-3;
        let mut light = |ldir: Vec3, lcol: Vec3, dist: f32, shadow_k: f32| {
            let ndl = ns.dot(ldir);
            if ndl <= 0.0 {
                return;
            }
            let tr = self.transmit(po, ldir, dist);
            let vis = 1.0 - shadow_k * (1.0 - tr);
            if vis <= 0.0 {
                return;
            }
            let diff = base * (m.diffusion * ndl * (1.0 - m.metallicity * 0.7));
            let hv = (ldir + view).normalize();
            let sp = ns.dot(hv).max(0.0).powf(shininess) * m.specularity * 1.5;
            col += (diff + spec_col * sp) * lcol * vis;
        };
        // sun / moon
        if self.sun.y > -0.1 {
            let k = (self.sun.y * 8.0 + 0.8).clamp(0.0, 1.0);
            light(
                self.sun,
                v3(sky.sun_color) * sky.sun_intensity * k,
                1e6,
                sky.shadows,
            );
        }
        for l in &self.lights {
            match l.kind {
                LightKind::Parallel { dir } => light(-dir, l.color, 1e6, 1.0),
                LightKind::Point | LightKind::Spot { .. } => {
                    let to = l.pos - p;
                    let dist = to.length();
                    let ldir = to / dist.max(1e-6);
                    let mut c = l.color;
                    if l.falloff {
                        c *= 60.0 / (dist * dist + 6.0);
                    }
                    if let LightKind::Spot {
                        dir,
                        cos_outer,
                        cos_inner,
                    } = l.kind
                    {
                        let ca = (-ldir).dot(dir);
                        if ca < cos_outer {
                            continue;
                        }
                        let k =
                            ((ca - cos_outer) / (cos_inner - cos_outer).max(1e-4)).clamp(0.0, 1.0);
                        c *= k * k * (3.0 - 2.0 * k);
                    }
                    light(ldir, c, dist, 1.0);
                }
            }
        }
        // reflection & transparency
        if depth < self.max_depth {
            let cosi = (-d).dot(ns).clamp(0.0, 1.0);
            let mut refl = m.reflection;
            if m.transparency > 0.0 || m.reflection > 0.0 {
                refl =
                    (refl + (1.0 - refl) * (1.0 - cosi).powi(5) * (m.specularity * 0.8)).min(1.0);
            }
            let mut trans = m.transparency;
            if trans > 0.0 {
                let (eta, nn) = if geo_front {
                    (1.0 / m.refraction.max(1.0), n)
                } else {
                    (m.refraction.max(1.0), -n)
                };
                let ci = -nn.dot(d);
                let k = 1.0 - eta * eta * (1.0 - ci * ci);
                let tc = if k < 0.0 {
                    let r = d - 2.0 * d.dot(nn) * nn;
                    self.trace(p + nn * 2e-3, r.normalize(), depth + 1)
                } else {
                    let t = (d * eta + nn * (eta * ci - k.sqrt())).normalize();
                    self.trace(p - nn * 2e-3, t, depth + 1)
                };
                let tint = Vec3::ONE.lerp(base, 0.5);
                col = col.lerp(tc * tint, trans * (1.0 - refl * 0.5));
                if ob.kind == Kind::Water {
                    trans = trans.min(0.8);
                }
                let _ = trans;
            }
            if refl > 0.01 {
                let r = (d - 2.0 * d.dot(ns) * ns).normalize();
                let rc = self.trace(p + ns * 2e-3, r, depth + 1);
                let tint = if m.metallicity > 0.0 {
                    Vec3::ONE.lerp(base, m.metallicity)
                } else {
                    Vec3::ONE
                };
                col = col.lerp(rc * tint, refl);
            }
            if alpha < 0.999 {
                let behind = self.trace(p + d * 2e-3, d, depth + 1);
                col = behind.lerp(col, alpha);
            }
        } else if alpha < 0.999 {
            col = self.sky_color(d).lerp(col, alpha);
        }
        if m.glow > 0.0 {
            col += base * m.glow;
        }
        col
    }

    pub fn pixel(&self, sx: f32, sy: f32) -> Vec3 {
        let (o, d) = self.view.ray(sx, sy);
        self.trace(o, d, 0)
    }
}

/// Holographic colour that shifts with the viewing angle (thin-film look), tinted by the
/// material's colours; tex_scale sets how strongly it shimmers.
fn iridescent(m: &Material, cos_view: f32, p: Vec3) -> Vec3 {
    let k = m.tex_scale.clamp(0.0, 2.0);
    let hue = (1.0 - cos_view) * 1.4 + noise(p * 0.7) * 0.15;
    let tau = std::f32::consts::TAU;
    let rainbow = Vec3::new(
        0.5 + 0.5 * (tau * (hue + 0.0)).cos(),
        0.5 + 0.5 * (tau * (hue + 0.33)).cos(),
        0.5 + 0.5 * (tau * (hue + 0.67)).cos(),
    );
    v3(m.diffuse).lerp(rainbow * v3(m.color2), (0.35 + 0.4 * k).min(1.0))
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn tex_coord(ob: &PObj, p: Vec3, h: &Hit, scale: f32) -> Vec3 {
    if ob.kind.is_infinite() {
        p * 0.12 * scale
    } else if ob.kind == Kind::Terrain {
        h.local * Vec3::new(4.0, 2.5, 4.0) * scale
    } else {
        h.local * scale
    }
}

fn texture(
    m: &Material,
    ob: &PObj,
    p: Vec3,
    h: &Hit,
    pic: Option<&image::RgbaImage>,
) -> (Vec3, f32) {
    let c1 = v3(m.diffuse);
    let c2 = v3(m.color2);
    let q = tex_coord(ob, p, h, m.tex_scale.max(0.01));
    match m.texture {
        TexKind::None | TexKind::Iridescent => (c1, 1.0),
        TexKind::Noise => (c2.lerp(c1, fbm(q * 2.0, 5) * 0.5 + 0.5), 1.0),
        TexKind::Marble => {
            let t = (q.x * 3.0 + q.y * 1.5 + fbm(q * 1.5, 6) * 7.0).sin() * 0.5 + 0.5;
            (c2.lerp(c1, t.powf(0.4)), 1.0)
        }
        TexKind::Wood => {
            let r = (q.x * q.x + q.z * q.z).sqrt() * 7.0 + fbm(q * 1.2, 3) * 2.5;
            let t = smoothstep(0.2, 0.8, r.fract());
            (c1.lerp(c2, t), 1.0)
        }
        TexKind::Granite => {
            let n = fbm(q * 3.0, 6) * 0.5 + 0.5;
            let speck = if noise(q * 40.0) > 0.45 { 0.6 } else { 1.0 };
            (c2.lerp(c1, n) * speck, 1.0)
        }
        TexKind::Checker => {
            let s = (q * 2.0).floor();
            let k = ((s.x + s.y + s.z) as i64).rem_euclid(2);
            (if k == 0 { c1 } else { c2 }, 1.0)
        }
        TexKind::Stripes => {
            let t = (q.y * 9.0 + fbm(q, 4) * 3.0).sin() * 0.5 + 0.5;
            (c2.lerp(c1, t), 1.0)
        }
        TexKind::Altitude => {
            let hf = if ob.kind == Kind::Terrain {
                h.local.y
            } else {
                ((p.y - ob.pos_y) / (ob.scale_y * 2.0).max(1.0) + 0.5).clamp(0.0, 1.0)
            };
            let slope = h.n.y;
            let jit = fbm(q * 3.0, 4) * 0.08;
            let lowland = c1.lerp(c1 * 0.7, (fbm(q * 6.0, 3) * 0.5 + 0.5) * 0.6);
            let rock = c2.lerp(c2 * 0.75, fbm(q * 8.0, 3) * 0.5 + 0.5);
            let mut col = lowland;
            let rk = smoothstep(0.82, 0.62, slope + jit).max(smoothstep(0.42, 0.6, hf + jit));
            col = col.lerp(rock, rk);
            let snow = smoothstep(0.66, 0.74, hf + jit) * smoothstep(0.55, 0.7, slope);
            col = col.lerp(Vec3::new(0.96, 0.97, 1.0), snow);
            (col, 1.0)
        }
        TexKind::Clouds => {
            let dens = (fbm(q * 0.5, 6) * 0.5 + 0.5 - (1.0 - m.tex_scale.clamp(0.0, 1.0)) * 0.3)
                * 1.8
                - 0.4;
            let a = dens.clamp(0.0, 1.0);
            (c1.lerp(c2, (1.0 - a) * 0.5), a)
        }
        TexKind::Image => match pic {
            Some(img) => {
                let (u, v) = if ob.kind == Kind::Sphere {
                    let l = h.local.normalize_or(Vec3::Y);
                    (
                        0.5 + l.z.atan2(l.x) / std::f32::consts::TAU,
                        0.5 - l.y.asin() / std::f32::consts::PI,
                    )
                } else if matches!(ob.kind, Kind::Square | Kind::Picture | Kind::Disk) {
                    ((h.local.x + 1.0) * 0.5, (h.local.z + 1.0) * 0.5)
                } else {
                    let a = h.ln.abs();
                    let l = q * 0.5 + Vec3::splat(0.5);
                    if a.x > a.y && a.x > a.z {
                        (l.z, 1.0 - l.y)
                    } else if a.y > a.z {
                        (l.x, l.z)
                    } else {
                        (l.x, 1.0 - l.y)
                    }
                };
                let s = sample_picture(img, u, v);
                (
                    Vec3::new(s[0], s[1], s[2]),
                    if ob.kind == Kind::Picture { 1.0 } else { 1.0 },
                )
            }
            None => (c1, 1.0),
        },
    }
}

fn bump(m: &Material, ob: &PObj, p: Vec3, h: &Hit, n: Vec3) -> Vec3 {
    let q = tex_coord(ob, p, h, m.tex_scale.max(0.01)) * 3.0;
    let e = 0.05;
    let f = |v: Vec3| fbm(v, 4);
    let base = f(q);
    let g = Vec3::new(
        f(q + Vec3::X * e) - base,
        f(q + Vec3::Y * e) - base,
        f(q + Vec3::Z * e) - base,
    ) / e;
    let k = m.bump * if ob.kind == Kind::Water { 0.04 } else { 0.12 };
    let g = g - n * g.dot(n);
    (n - g * k).normalize_or(n)
}

pub fn sky_color(s: &Sky, sun: Vec3, d: Vec3) -> Vec3 {
    let el = d.y;
    let zen = v3(s.zenith);
    let hor = v3(s.horizon);
    let mut c = if el >= 0.0 {
        hor.lerp(zen, el.powf(0.5))
    } else {
        hor * (1.0 - (-el).min(1.0) * 0.4)
    };
    let cs = d.dot(sun);
    // stars
    if s.stars && el > 0.0 {
        let k = 220.0;
        let a = d.x.atan2(d.z);
        let gx = (a * k / 3.0).floor() as i32;
        let gy = (el * k).floor() as i32;
        let r = hash2(gx, gy);
        if r > 0.985 {
            let night = (1.0 - (sun.y * 4.0 + 0.5)).clamp(0.0, 1.0);
            c += Vec3::splat((r - 0.985) * 60.0 * night * el.min(0.3) / 0.3);
        }
    }
    // clouds
    if s.cloud_cover > 0.0 && el > 0.0 {
        let tpl = s.cloud_height.max(5.0) / el.max(0.02);
        let p = d * tpl;
        let q = Vec3::new(p.x, 0.0, p.z) * (0.006 * s.cloud_frequency.max(0.05));
        let n = fbm(q, 6) * 0.5 + 0.5;
        let dens =
            ((n - (1.0 - s.cloud_cover)) / (s.cloud_amplitude.max(0.05) * 0.6)).clamp(0.0, 1.0);
        let fade = smoothstep(0.0, 0.15, el);
        let lit = 0.55 + 0.45 * (sun.y.max(0.0) * 2.0).min(1.0);
        let silver = cs.max(0.0).powi(8) * 0.4;
        let cc = v3(s.cloud_color) * lit * (1.0 - dens * 0.25) + v3(s.sun_color) * silver;
        c = c.lerp(cc, dens * fade);
    }
    // sun / moon
    if s.sun_visible || s.moon {
        let col = v3(s.sun_color);
        if s.moon {
            if cs > 0.9993 {
                return Vec3::new(0.92, 0.92, 0.85);
            }
            c += Vec3::splat(cs.max(0.0).powi(400) * 0.15);
        } else {
            c += col * (cs.max(0.0).powi(6) * 0.18 + cs.max(0.0).powi(120) * 0.6) * s.sun_intensity;
            if cs > 0.99955 {
                c = c.lerp(col * 1.6 + Vec3::splat(0.5), 0.95);
            }
        }
    }
    // horizon haze
    if s.haze > 0.0 {
        let k = (1.0 - el.abs()).powi(6) * (s.haze * 1.5).min(1.0);
        c = c.lerp(v3(s.haze_color) * (sun.y * 3.0 + 0.6).clamp(0.25, 1.0), k);
    }
    if s.fog > 0.0 {
        let k = (1.0 - el.max(0.0)).powi(30) * (s.fog * 2.0).min(1.0);
        c = c.lerp(v3(s.fog_color), k);
    }
    c
}

pub fn to_rgba(c: Vec3) -> [u8; 4] {
    let f = |x: f32| (x.clamp(0.0, 1.0).powf(0.95) * 255.0 + 0.5) as u8;
    [f(c.x), f(c.y), f(c.z), 255]
}

/// Render a whole image synchronously (previews and thumbnails).
pub fn render_image(prep: &Prepared, w: usize, h: usize) -> Vec<u8> {
    let rows: Vec<Vec<u8>> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row = Vec::with_capacity(w * 4);
            for x in 0..w {
                let sx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
                let sy = 1.0 - (y as f32 + 0.5) / h as f32 * 2.0;
                row.extend_from_slice(&to_rgba(prep.pixel(sx, sy)));
            }
            row
        })
        .collect();
    rows.concat()
}

// ---------------------------------------------------------------------------------------------
// Progressive background renderer

pub struct RenderJob {
    pub width: usize,
    pub height: usize,
    pub buffer: Arc<Mutex<Vec<u8>>>,
    pub progress: Arc<AtomicU32>,
    pub version: Arc<AtomicU64>,
    pub done: Arc<AtomicBool>,
    pub cancel: Arc<AtomicBool>,
    pub started: std::time::Instant,
    pub finished_secs: Arc<Mutex<Option<f32>>>,
}

impl RenderJob {
    pub fn start(
        prep: Prepared,
        width: usize,
        height: usize,
        antialias: bool,
        on_update: impl Fn() + Send + Sync + 'static,
    ) -> RenderJob {
        let buffer = Arc::new(Mutex::new(vec![0u8; width * height * 4]));
        let progress = Arc::new(AtomicU32::new(0));
        let version = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let finished_secs = Arc::new(Mutex::new(None));
        let job = RenderJob {
            width,
            height,
            buffer: buffer.clone(),
            progress: progress.clone(),
            version: version.clone(),
            done: done.clone(),
            cancel: cancel.clone(),
            started: std::time::Instant::now(),
            finished_secs: finished_secs.clone(),
        };
        let t0 = job.started;
        std::thread::spawn(move || {
            let prep = Arc::new(prep);
            let w = width;
            let h = height;
            let mut colors = vec![Vec3::ZERO; w * h];
            let passes = [16usize, 8, 4, 2, 1];
            let total_steps = passes.len() + if antialias { 1 } else { 0 };
            let sample = |x: f32, y: f32| -> Vec3 {
                let sx = x / w as f32 * 2.0 - 1.0;
                let sy = 1.0 - y / h as f32 * 2.0;
                prep.pixel(sx, sy)
            };
            for (pi, &bs) in passes.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let rows: Vec<usize> = (0..h).step_by(bs).collect();
                let results: Vec<(usize, Vec<(usize, Vec3)>)> = rows
                    .par_iter()
                    .filter_map(|&y| {
                        if cancel.load(Ordering::Relaxed) {
                            return None;
                        }
                        let mut out = Vec::new();
                        let mut x = 0;
                        while x < w {
                            let skip = pi > 0 && x % (bs * 2) == 0 && y % (bs * 2) == 0;
                            if !skip {
                                let c = sample(x as f32 + 0.5, y as f32 + 0.5);
                                out.push((x, c));
                            }
                            x += bs;
                        }
                        // paint this row's blocks right away
                        {
                            let mut buf = buffer.lock().unwrap();
                            for &(x, c) in &out {
                                let px = to_rgba(c);
                                for yy in y..(y + bs).min(h) {
                                    for xx in x..(x + bs).min(w) {
                                        let i = (yy * w + xx) * 4;
                                        buf[i..i + 4].copy_from_slice(&px);
                                    }
                                }
                            }
                        }
                        version.fetch_add(1, Ordering::Relaxed);
                        Some((y, out))
                    })
                    .collect();
                for (y, row) in results {
                    for (x, c) in row {
                        colors[y * w + x] = c;
                    }
                }
                progress.store(((pi + 1) * 1000 / total_steps) as u32, Ordering::Relaxed);
                on_update();
            }
            if antialias && !cancel.load(Ordering::Relaxed) {
                let colors_ref = &colors;
                let rows: Vec<(usize, Vec<Vec3>)> = (0..h)
                    .into_par_iter()
                    .filter_map(|y| {
                        if cancel.load(Ordering::Relaxed) {
                            return None;
                        }
                        let mut row = Vec::with_capacity(w);
                        for x in 0..w {
                            let c = colors_ref[y * w + x];
                            let mut diff = 0.0f32;
                            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                                let xx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                                let yy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                                diff = diff.max((colors_ref[yy * w + xx] - c).abs().max_element());
                            }
                            if diff > 0.06 {
                                let mut acc = c;
                                for (ox, oy) in
                                    [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
                                {
                                    acc += sample(x as f32 + ox, y as f32 + oy);
                                }
                                row.push(acc / 5.0);
                            } else {
                                row.push(c);
                            }
                        }
                        {
                            let mut buf = buffer.lock().unwrap();
                            for (x, c) in row.iter().enumerate() {
                                let i = (y * w + x) * 4;
                                buf[i..i + 4].copy_from_slice(&to_rgba(*c));
                            }
                        }
                        version.fetch_add(1, Ordering::Relaxed);
                        Some((y, row))
                    })
                    .collect();
                drop(rows);
                progress.store(1000, Ordering::Relaxed);
            }
            if !cancel.load(Ordering::Relaxed) {
                progress.store(1000, Ordering::Relaxed);
                *finished_secs.lock().unwrap() = Some(t0.elapsed().as_secs_f32());
            }
            done.store(true, Ordering::Relaxed);
            version.fetch_add(1, Ordering::Relaxed);
            on_update();
        });
        job
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Render the camera view of `scene` at full resolution synchronously (animation frames).
pub fn render_frame(scene: &Scene) -> image::RgbaImage {
    let w = scene.render.width as usize;
    let h = scene.render.height as usize;
    let view = View::from_camera(&scene.camera, w as f32 / h as f32);
    let prep = prepare(scene, view, None);
    if !scene.render.antialias {
        let buf = render_image(&prep, w, h);
        return image::RgbaImage::from_raw(w as u32, h as u32, buf).unwrap();
    }
    // 2 x 2 samples per pixel, averaged in linear values
    let offs = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
    let rows: Vec<Vec<u8>> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row = Vec::with_capacity(w * 4);
            for x in 0..w {
                let mut c = Vec3::ZERO;
                for (ox, oy) in offs {
                    let sx = (x as f32 + ox) / w as f32 * 2.0 - 1.0;
                    let sy = 1.0 - (y as f32 + oy) / h as f32 * 2.0;
                    c += prep.pixel(sx, sy).clamp(Vec3::ZERO, Vec3::ONE);
                }
                row.extend_from_slice(&to_rgba(c / 4.0));
            }
            row
        })
        .collect();
    image::RgbaImage::from_raw(w as u32, h as u32, rows.concat()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{find_material, Kind, Scene};

    #[test]
    fn all_kinds_render() {
        let mut s = Scene::empty();
        let kinds = [
            Kind::Sphere,
            Kind::Cube,
            Kind::Cylinder,
            Kind::Cone,
            Kind::Torus,
            Kind::Pyramid,
            Kind::Disk,
            Kind::Square,
            Kind::Stone,
        ];
        for (n, k) in kinds.iter().enumerate() {
            let i = s.add(*k);
            let o = &mut s.objects[i];
            o.pos = Vec3::new(
                -16.0 + n as f32 * 4.0,
                if matches!(k, Kind::Disk | Kind::Square) {
                    0.05
                } else {
                    1.5
                },
                0.0,
            );
            o.scale = Vec3::splat(1.5);
            o.rot.y = 25.0;
            if n % 3 == 1 {
                o.material = find_material("Marmor").unwrap();
            } else if n % 3 == 2 {
                o.material = find_material("Glas").unwrap();
            }
        }
        let l = s.add(Kind::SpotLight);
        s.objects[l].pos = Vec3::new(0.0, 8.0, 2.0);
        s.objects[l].light.color = [1.0, 0.3, 0.2];
        s.objects[l].light.intensity = 4.0;
        let p = s.add(Kind::RadialLight);
        s.objects[p].pos = Vec3::new(-10.0, 4.0, 4.0);
        s.objects[p].light.color = [0.3, 0.5, 1.0];
        s.objects[p].light.intensity = 3.0;
        s.sky = crate::scene::sky_presets()
            .into_iter()
            .find(|k| k.name == "Sonnenuntergang")
            .unwrap();
        s.camera = crate::scene::Camera {
            pos: Vec3::new(0.0, 6.0, 22.0),
            yaw: 0.0,
            pitch: -14.0,
            fov: 60.0,
        };
        s.render.width = 900;
        s.render.height = 450;
        let img = render_frame(&s);
        std::fs::create_dir_all("shots").unwrap();
        img.save("shots/kinds.png").unwrap();
        s.sky = crate::scene::sky_presets()
            .into_iter()
            .find(|k| k.name == "Mondnacht")
            .unwrap();
        s.sky.fog = 0.3;
        render_frame(&s).save("shots/night.png").unwrap();
    }
}
