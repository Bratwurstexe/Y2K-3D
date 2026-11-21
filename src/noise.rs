//! Gradient noise (improved Perlin) and fractal sums used by terrain, textures and clouds.

use glam::Vec3;

const PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
    142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
    203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230,
    220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76,
    132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173,
    186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
    59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163,
    70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232,
    178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162,
    241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184, 84, 204,
    176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141,
    128, 195, 78, 66, 215, 61, 156, 180,
];

#[inline]
fn p(i: i32) -> i32 {
    PERM[(i & 255) as usize] as i32
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn grad(hash: i32, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

/// Noise in roughly -1..1.
pub fn noise3(x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let zf = z - z.floor();
    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);
    let a = p(xi) + yi;
    let aa = p(a) + zi;
    let ab = p(a + 1) + zi;
    let b = p(xi + 1) + yi;
    let ba = p(b) + zi;
    let bb = p(b + 1) + zi;
    lerp(
        lerp(
            lerp(grad(p(aa), xf, yf, zf), grad(p(ba), xf - 1.0, yf, zf), u),
            lerp(
                grad(p(ab), xf, yf - 1.0, zf),
                grad(p(bb), xf - 1.0, yf - 1.0, zf),
                u,
            ),
            v,
        ),
        lerp(
            lerp(
                grad(p(aa + 1), xf, yf, zf - 1.0),
                grad(p(ba + 1), xf - 1.0, yf, zf - 1.0),
                u,
            ),
            lerp(
                grad(p(ab + 1), xf, yf - 1.0, zf - 1.0),
                grad(p(bb + 1), xf - 1.0, yf - 1.0, zf - 1.0),
                u,
            ),
            v,
        ),
        w,
    )
}

pub fn noise(v: Vec3) -> f32 {
    noise3(v.x, v.y, v.z)
}

/// Fractal sum, result roughly -1..1.
pub fn fbm(v: Vec3, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut q = v;
    for _ in 0..octaves {
        sum += amp * noise(q);
        q = q * 2.03 + Vec3::new(1.7, 9.2, 3.1);
        amp *= 0.5;
    }
    sum * 1.6
}

/// Ridged fractal, result roughly 0..1.
pub fn ridged(v: Vec3, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut q = v;
    let mut norm = 0.0;
    for _ in 0..octaves {
        let n = 1.0 - noise(q).abs();
        sum += amp * n * n;
        norm += amp;
        q = q * 2.07 + Vec3::new(5.3, 1.3, 7.7);
        amp *= 0.5;
    }
    sum / norm
}

/// Cheap hash for per-cell randomness.
pub fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h ^ (h >> 16)) as f32 / u32::MAX as f32
}

/// Small deterministic random generator.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407)
            | 1)
    }
    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    pub fn f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
}

pub fn time_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345)
}
