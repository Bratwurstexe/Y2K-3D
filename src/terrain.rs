//! Height maps for terrain objects and the operations of the terrain editor.

use crate::noise::{fbm, hash2, ridged, Rng};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct HeightMap {
    pub size: usize,
    pub h: Vec<f32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FractalKind {
    Mountains,
    Hills,
    Ridges,
    Canyons,
    Dunes,
    Crater,
}

impl FractalKind {
    pub const ALL: [FractalKind; 6] = [
        FractalKind::Mountains,
        FractalKind::Hills,
        FractalKind::Ridges,
        FractalKind::Canyons,
        FractalKind::Dunes,
        FractalKind::Crater,
    ];
    pub fn label(self) -> &'static str {
        match self {
            FractalKind::Mountains => crate::i18n::tr("Gebirge"),
            FractalKind::Hills => crate::i18n::tr("Hügel"),
            FractalKind::Ridges => crate::i18n::tr("Grate"),
            FractalKind::Canyons => crate::i18n::tr("Schluchten"),
            FractalKind::Dunes => crate::i18n::tr("Dünen"),
            FractalKind::Crater => crate::i18n::tr("Krater"),
        }
    }
}

impl HeightMap {
    pub fn flat(size: usize) -> Self {
        HeightMap {
            size,
            h: vec![0.0; size * size],
        }
    }

    #[inline]
    pub fn get(&self, x: usize, z: usize) -> f32 {
        self.h[z * self.size + x]
    }

    pub fn get_clamped(&self, x: i32, z: i32) -> f32 {
        let n = self.size as i32 - 1;
        self.h[(z.clamp(0, n) * self.size as i32 + x.clamp(0, n)) as usize]
    }

    /// Bilinear sample with u,v in 0..1.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let n = (self.size - 1) as f32;
        let x = (u.clamp(0.0, 1.0)) * n;
        let z = (v.clamp(0.0, 1.0)) * n;
        let x0 = x.floor() as i32;
        let z0 = z.floor() as i32;
        let fx = x - x0 as f32;
        let fz = z - z0 as f32;
        let a = self.get_clamped(x0, z0);
        let b = self.get_clamped(x0 + 1, z0);
        let c = self.get_clamped(x0, z0 + 1);
        let d = self.get_clamped(x0 + 1, z0 + 1);
        (a * (1.0 - fx) + b * fx) * (1.0 - fz) + (c * (1.0 - fx) + d * fx) * fz
    }

    pub fn fractal(size: usize, kind: FractalKind, seed: u64) -> Self {
        let mut m = HeightMap::flat(size);
        let mut rng = Rng::new(seed);
        let off = Vec3::new(
            rng.range(0.0, 500.0),
            rng.range(0.0, 500.0),
            rng.range(0.0, 500.0),
        );
        for z in 0..size {
            for x in 0..size {
                let u = x as f32 / (size - 1) as f32;
                let v = z as f32 / (size - 1) as f32;
                let p = Vec3::new(u * 4.0, 0.0, v * 4.0) + off;
                let dx = u - 0.5;
                let dz = v - 0.5;
                let r = (dx * dx + dz * dz).sqrt() * 2.0;
                let falloff = (1.0 - r * r).max(0.0);
                let val = match kind {
                    FractalKind::Mountains => {
                        (ridged(p * 0.8, 7) * 0.7 + (fbm(p, 6) * 0.5 + 0.5) * 0.3)
                            * falloff.powf(0.6)
                    }
                    FractalKind::Hills => (fbm(p * 0.6, 5) * 0.5 + 0.5) * (0.4 + 0.6 * falloff),
                    FractalKind::Ridges => ridged(p * 1.3, 8).powf(1.6),
                    FractalKind::Canyons => {
                        let n = fbm(p * 0.7, 6) * 0.5 + 0.5;
                        let c = (1.0 - (fbm(p * 0.35 + 3.0, 4)).abs() * 3.0).clamp(0.0, 1.0);
                        (n * 0.4 + 0.6) * (1.0 - c * 0.8)
                    }
                    FractalKind::Dunes => {
                        let w = fbm(p * 0.5, 3);
                        ((p.x * 2.5 + w * 3.0).sin() * 0.5 + 0.5).powf(1.5) * 0.6
                            + (fbm(p * 1.5, 4) * 0.5 + 0.5) * 0.2
                    }
                    FractalKind::Crater => {
                        let rim = (-(r - 0.55).powi(2) * 60.0).exp();
                        let bowl = if r < 0.55 {
                            0.25 + r * 0.4
                        } else {
                            0.25 * falloff
                        };
                        bowl + rim * 0.6 + fbm(p * 1.5, 5) * 0.08
                    }
                };
                m.h[z * size + x] = val;
            }
        }
        m.normalize();
        m
    }

    pub fn normalize(&mut self) {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for &v in &self.h {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let span = (hi - lo).max(1e-6);
        for v in &mut self.h {
            *v = (*v - lo) / span;
        }
    }

    pub fn clamp01(&mut self) {
        for v in &mut self.h {
            *v = v.clamp(0.0, 1.0);
        }
    }

    pub fn smooth(&mut self, passes: usize) {
        let n = self.size as i32;
        for _ in 0..passes {
            let old = self.h.clone();
            for z in 0..n {
                for x in 0..n {
                    let mut s = 0.0;
                    let mut c = 0.0;
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            let xx = (x + dx).clamp(0, n - 1);
                            let zz = (z + dz).clamp(0, n - 1);
                            let w = if dx == 0 && dz == 0 { 2.0 } else { 1.0 };
                            s += old[(zz * n + xx) as usize] * w;
                            c += w;
                        }
                    }
                    self.h[(z * n + x) as usize] = s / c;
                }
            }
        }
    }

    pub fn sharpen(&mut self) {
        let orig = self.h.clone();
        self.smooth(1);
        for (v, o) in self.h.iter_mut().zip(orig) {
            *v = (o + (o - *v) * 1.5).clamp(0.0, 1.0);
        }
    }

    /// Simple hydraulic + thermal erosion.
    pub fn erode(&mut self, iterations: usize, seed: u64) {
        let n = self.size;
        let mut rng = Rng::new(seed);
        let drops = iterations * n * 2;
        for _ in 0..drops {
            let mut x = rng.range(1.0, (n - 2) as f32);
            let mut z = rng.range(1.0, (n - 2) as f32);
            let mut sediment = 0.0f32;
            let mut water = 1.0f32;
            for _ in 0..48 {
                let xi = x as usize;
                let zi = z as usize;
                if xi < 1 || zi < 1 || xi >= n - 1 || zi >= n - 1 {
                    break;
                }
                let h0 = self.get(xi, zi);
                // find lowest neighbour
                let mut best = (0i32, 0i32, h0);
                for dz in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let hh = self.get((xi as i32 + dx) as usize, (zi as i32 + dz) as usize);
                        if hh < best.2 {
                            best = (dx, dz, hh);
                        }
                    }
                }
                let idx = zi * n + xi;
                if best.0 == 0 && best.1 == 0 {
                    self.h[idx] += sediment * 0.5;
                    break;
                }
                let drop = h0 - best.2;
                let capacity = drop * water * 4.0;
                if sediment > capacity {
                    let dep = (sediment - capacity) * 0.3;
                    self.h[idx] += dep;
                    sediment -= dep;
                } else {
                    let take = ((capacity - sediment) * 0.3).min(drop * 0.5);
                    self.h[idx] -= take;
                    sediment += take;
                }
                x += best.0 as f32;
                z += best.1 as f32;
                water *= 0.98;
            }
        }
        // thermal pass
        let talus = 1.5 / n as f32;
        for _ in 0..iterations.min(10) {
            for z in 1..n - 1 {
                for x in 1..n - 1 {
                    let i = z * n + x;
                    for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                        let j = ((z as i32 + dz) as usize) * n + (x as i32 + dx) as usize;
                        let d = self.h[i] - self.h[j];
                        if d > talus {
                            let mv = (d - talus) * 0.25;
                            self.h[i] -= mv;
                            self.h[j] += mv;
                        }
                    }
                }
            }
        }
        self.clamp01();
    }

    pub fn raise(&mut self, amount: f32) {
        for v in &mut self.h {
            *v = (*v + amount).clamp(0.0, 1.0);
        }
    }

    pub fn amplify(&mut self, factor: f32) {
        for v in &mut self.h {
            *v = ((*v - 0.5) * factor + 0.5).clamp(0.0, 1.0);
        }
    }

    pub fn invert(&mut self) {
        for v in &mut self.h {
            *v = 1.0 - *v;
        }
    }

    pub fn terraces(&mut self, steps: usize) {
        let s = steps.max(2) as f32;
        for v in &mut self.h {
            let t = *v * s;
            let f = t.fract();
            *v = ((t.floor() + f.powf(6.0)) / s).clamp(0.0, 1.0);
        }
    }

    pub fn spikes(&mut self, seed: u64) {
        let n = self.size;
        let mut rng = Rng::new(seed);
        for _ in 0..(n / 4) {
            let cx = rng.range(0.0, n as f32);
            let cz = rng.range(0.0, n as f32);
            let r = rng.range(2.0, n as f32 / 18.0);
            let hgt = rng.range(0.1, 0.4);
            self.add_bump(cx, cz, r, hgt, 2.5);
        }
        self.clamp01();
    }

    pub fn add_bump(&mut self, cx: f32, cz: f32, r: f32, height: f32, power: f32) {
        let n = self.size as i32;
        let x0 = ((cx - r).floor() as i32).max(0);
        let x1 = ((cx + r).ceil() as i32).min(n - 1);
        let z0 = ((cz - r).floor() as i32).max(0);
        let z1 = ((cz + r).ceil() as i32).min(n - 1);
        for z in z0..=z1 {
            for x in x0..=x1 {
                let d = ((x as f32 - cx).powi(2) + (z as f32 - cz).powi(2)).sqrt() / r;
                if d < 1.0 {
                    let w = (1.0 - d).powf(power);
                    self.h[(z * n + x) as usize] += height * w;
                }
            }
        }
    }

    pub fn add_noise(&mut self, amount: f32, seed: u64) {
        let n = self.size;
        let s = (seed % 1000) as i32;
        for z in 0..n {
            for x in 0..n {
                let r = hash2(x as i32 + s * 31, z as i32 - s * 17) - 0.5;
                let i = z * n + x;
                self.h[i] = (self.h[i] + r * amount).clamp(0.0, 1.0);
            }
        }
    }

    /// Fade the edges into a round island.
    pub fn round_edges(&mut self) {
        let n = self.size;
        for z in 0..n {
            for x in 0..n {
                let dx = x as f32 / (n - 1) as f32 - 0.5;
                let dz = z as f32 / (n - 1) as f32 - 0.5;
                let r = (dx * dx + dz * dz).sqrt() * 2.0;
                let f = ((1.0 - r) * 4.0).clamp(0.0, 1.0);
                let f = f * f * (3.0 - 2.0 * f);
                self.h[z * n + x] *= f;
            }
        }
    }

    pub fn apply_curve(&mut self, curve: &[[f32; 2]]) {
        for v in &mut self.h {
            *v = eval_curve(curve, *v);
        }
    }

    pub fn resample(&self, size: usize) -> HeightMap {
        let mut m = HeightMap::flat(size);
        for z in 0..size {
            for x in 0..size {
                m.h[z * size + x] =
                    self.sample(x as f32 / (size - 1) as f32, z as f32 / (size - 1) as f32);
            }
        }
        m
    }

    pub fn from_image(img: &image::DynamicImage, size: usize) -> HeightMap {
        let g = img.to_luma32f();
        let (w, h) = g.dimensions();
        let mut m = HeightMap::flat(size);
        for z in 0..size {
            for x in 0..size {
                let px = ((x as f32 / (size - 1) as f32) * (w - 1) as f32) as u32;
                let pz = ((z as f32 / (size - 1) as f32) * (h - 1) as f32) as u32;
                m.h[z * size + x] = g.get_pixel(px, pz).0[0];
            }
        }
        m
    }

    /// Blend another map: mode 0 replace, 1 add, 2 multiply, 3 max.
    pub fn blend(&mut self, other: &HeightMap, mode: usize, amount: f32) {
        let o = if other.size == self.size {
            other.clone()
        } else {
            other.resample(self.size)
        };
        for (v, w) in self.h.iter_mut().zip(o.h) {
            let t = match mode {
                0 => w,
                1 => *v + w * 0.5,
                2 => *v * w,
                _ => v.max(w),
            };
            *v = (*v * (1.0 - amount) + t * amount).clamp(0.0, 1.0);
        }
    }

    /// Paint with a soft brush. mode: 0 raise, 1 lower, 2 smooth, 3 flatten to `level`.
    pub fn brush(&mut self, cx: f32, cz: f32, radius: f32, strength: f32, mode: usize, level: f32) {
        let n = self.size as i32;
        let x0 = ((cx - radius).floor() as i32).max(0);
        let x1 = ((cx + radius).ceil() as i32).min(n - 1);
        let z0 = ((cz - radius).floor() as i32).max(0);
        let z1 = ((cz + radius).ceil() as i32).min(n - 1);
        let old = if mode == 2 {
            Some(self.h.clone())
        } else {
            None
        };
        for z in z0..=z1 {
            for x in x0..=x1 {
                let d = ((x as f32 - cx).powi(2) + (z as f32 - cz).powi(2)).sqrt() / radius;
                if d >= 1.0 {
                    continue;
                }
                let w = (1.0 - d * d).powi(2) * strength;
                let i = (z * n + x) as usize;
                match mode {
                    0 => self.h[i] += w * 0.05,
                    1 => self.h[i] -= w * 0.05,
                    2 => {
                        let o = old.as_ref().unwrap();
                        let mut s = 0.0;
                        for dz in -1..=1 {
                            for dx in -1..=1 {
                                let xx = (x + dx).clamp(0, n - 1);
                                let zz = (z + dz).clamp(0, n - 1);
                                s += o[(zz * n + xx) as usize];
                            }
                        }
                        self.h[i] += (s / 9.0 - self.h[i]) * w.min(1.0);
                    }
                    _ => self.h[i] += (level - self.h[i]) * (w * 0.3).min(1.0),
                }
                self.h[i] = self.h[i].clamp(0.0, 1.0);
            }
        }
    }
}

pub fn eval_curve(curve: &[[f32; 2]], x: f32) -> f32 {
    if curve.is_empty() {
        return x;
    }
    if x <= curve[0][0] {
        return curve[0][1];
    }
    for w in curve.windows(2) {
        if x <= w[1][0] {
            let t = (x - w[0][0]) / (w[1][0] - w[0][0]).max(1e-6);
            return w[0][1] + (w[1][1] - w[0][1]) * t;
        }
    }
    curve[curve.len() - 1][1]
}
