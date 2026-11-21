//! Opening pictures, including the colour-palette TIFF files of the old CD, which the
//! general image library refuses.

use image::{DynamicImage, RgbaImage};
use std::path::Path;

pub fn open(path: impl AsRef<Path>) -> Result<DynamicImage, String> {
    let path = path.as_ref();
    match image::open(path) {
        Ok(i) => Ok(i),
        Err(e) => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            palette_tiff(&bytes)
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| e.to_string())
        }
    }
}

struct Rd<'a> {
    b: &'a [u8],
    le: bool,
}

impl Rd<'_> {
    fn u16(&self, o: usize) -> Option<u32> {
        let s = self.b.get(o..o + 2)?;
        Some(if self.le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        } as u32)
    }
    fn u32(&self, o: usize) -> Option<u32> {
        let s = self.b.get(o..o + 4)?;
        Some(if self.le {
            u32::from_le_bytes([s[0], s[1], s[2], s[3]])
        } else {
            u32::from_be_bytes([s[0], s[1], s[2], s[3]])
        })
    }
    /// Values of an IFD entry (SHORT or LONG).
    fn values(&self, entry: usize) -> Option<Vec<u32>> {
        let typ = self.u16(entry + 2)?;
        let count = self.u32(entry + 4)? as usize;
        let size = match typ {
            3 => 2,
            4 => 4,
            1 => 1,
            _ => return None,
        };
        let data = if size * count <= 4 {
            entry + 8
        } else {
            self.u32(entry + 8)? as usize
        };
        (0..count)
            .map(|i| match size {
                2 => self.u16(data + i * 2),
                4 => self.u32(data + i * 4),
                _ => self.b.get(data + i).map(|v| *v as u32),
            })
            .collect()
    }
}

fn packbits(src: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < src.len() {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            let c = n as usize + 1;
            out.extend_from_slice(&src[i..(i + c).min(src.len())]);
            i += c;
        } else if n != -128 {
            if let Some(&v) = src.get(i) {
                out.extend(std::iter::repeat_n(v, (1 - n as i32) as usize));
            }
            i += 1;
        }
    }
}

fn lzw(src: &[u8], out: &mut Vec<u8>) {
    let mut table: Vec<Vec<u8>> = (0..256).map(|i| vec![i as u8]).collect();
    table.push(Vec::new());
    table.push(Vec::new());
    let mut bits = 9;
    let mut pos = 0usize;
    let mut prev: Option<Vec<u8>> = None;
    let read = |pos: &mut usize, bits: usize| -> Option<usize> {
        let mut v = 0usize;
        for _ in 0..bits {
            let byte = *src.get(*pos / 8)?;
            let bit = (byte >> (7 - (*pos % 8))) & 1;
            v = (v << 1) | bit as usize;
            *pos += 1;
        }
        Some(v)
    };
    while let Some(code) = read(&mut pos, bits) {
        if code == 256 {
            table.truncate(258);
            bits = 9;
            prev = None;
            continue;
        }
        if code == 257 {
            break;
        }
        let entry = if code < table.len() {
            table[code].clone()
        } else if let Some(p) = &prev {
            let mut e = p.clone();
            e.push(p[0]);
            e
        } else {
            break;
        };
        out.extend_from_slice(&entry);
        if let Some(p) = prev {
            let mut n = p;
            n.push(entry[0]);
            table.push(n);
        }
        prev = Some(entry);
        if table.len() + 1 >= (1 << bits) && bits < 12 {
            bits += 1;
        }
    }
}

fn palette_tiff(b: &[u8]) -> Option<RgbaImage> {
    let le = match b.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let r = Rd { b, le };
    let ifd = r.u32(4)? as usize;
    let n = r.u16(ifd)? as usize;
    let (mut w, mut h, mut bps, mut comp, mut rps) = (0u32, 0u32, 8u32, 1u32, u32::MAX);
    let mut offsets = Vec::new();
    let mut counts = Vec::new();
    let mut cmap = Vec::new();
    let mut predictor = 1;
    for k in 0..n {
        let e = ifd + 2 + k * 12;
        let tag = r.u16(e)?;
        let v = r.values(e);
        let first = v.as_ref().and_then(|v| v.first().copied()).unwrap_or(0);
        match tag {
            256 => w = first,
            257 => h = first,
            258 => bps = first,
            259 => comp = first,
            273 => offsets = v.unwrap_or_default(),
            278 => rps = first,
            279 => counts = v.unwrap_or_default(),
            317 => predictor = first,
            320 => cmap = v.unwrap_or_default(),
            _ => {}
        }
    }
    if w == 0 || h == 0 || !(bps == 8 || bps == 4) || offsets.is_empty() {
        return None;
    }
    let _ = rps;
    let mut data = Vec::new();
    for (i, &o) in offsets.iter().enumerate() {
        let o = o as usize;
        let c = counts.get(i).copied().unwrap_or((b.len() - o) as u32) as usize;
        let strip = b.get(o..(o + c).min(b.len()))?;
        match comp {
            1 => data.extend_from_slice(strip),
            32773 => packbits(strip, &mut data),
            5 => lzw(strip, &mut data),
            _ => return None,
        }
    }
    let row_bytes = if bps == 8 {
        w as usize
    } else {
        (w as usize).div_ceil(2)
    };
    if predictor == 2 && bps == 8 {
        for row in data.chunks_mut(row_bytes) {
            for i in 1..row.len() {
                row[i] = row[i].wrapping_add(row[i - 1]);
            }
        }
    }
    let colors = 1usize << bps;
    let pal = |i: usize| -> [u8; 4] {
        if cmap.len() >= colors * 3 {
            [
                (cmap[i] >> 8) as u8,
                (cmap[colors + i] >> 8) as u8,
                (cmap[2 * colors + i] >> 8) as u8,
                255,
            ]
        } else {
            let g = (i * 255 / (colors - 1)) as u8;
            [g, g, g, 255]
        }
    };
    let mut img = RgbaImage::new(w, h);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let idx = if bps == 8 {
                *data.get(y * row_bytes + x).unwrap_or(&0) as usize
            } else {
                let byte = *data.get(y * row_bytes + x / 2).unwrap_or(&0);
                (if x % 2 == 0 { byte >> 4 } else { byte & 15 }) as usize
            };
            img.put_pixel(x as u32, y as u32, image::Rgba(pal(idx.min(colors - 1))));
        }
    }
    Some(img)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn converts_palette_tiffs() {
        let a = super::open(format!(
            r"{}\Terrain  Maps\Crater Lake.TIF",
            std::env::var("Y2K3D_LIBRARY").unwrap()
        ))
        .unwrap();
        a.to_rgba8().save("shots/tif_terrain.png").unwrap();
        let b = super::open(format!(
            r"{}\Picture Textures\Brick Wall.TIF",
            std::env::var("Y2K3D_LIBRARY").unwrap()
        ))
        .unwrap();
        b.thumbnail(256, 256)
            .to_rgba8()
            .save("shots/tif_texture.png")
            .unwrap();
    }
}
