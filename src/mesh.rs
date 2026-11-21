//! Triangle meshes: imported models (OBJ, DXF), generated stones, and a BVH for ray tests.

use crate::noise::{fbm, Rng};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct TriMesh {
    pub verts: Vec<Vec3>,
    pub tris: Vec<[u32; 3]>,
    #[serde(default)]
    pub smooth: bool,
}

impl TriMesh {
    /// Center the mesh and scale it to fit the -1..1 cube.
    pub fn fit_unit(&mut self) {
        if self.verts.is_empty() {
            return;
        }
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for v in &self.verts {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
        let c = (lo + hi) * 0.5;
        let s = ((hi - lo).max_element() * 0.5).max(1e-6);
        for v in &mut self.verts {
            *v = (*v - c) / s;
        }
    }

    pub fn vertex_normals(&self) -> Vec<Vec3> {
        let mut n = vec![Vec3::ZERO; self.verts.len()];
        for t in &self.tris {
            let a = self.verts[t[0] as usize];
            let b = self.verts[t[1] as usize];
            let c = self.verts[t[2] as usize];
            let f = (b - a).cross(c - a);
            for &i in t {
                n[i as usize] += f;
            }
        }
        for v in &mut n {
            *v = v.normalize_or(Vec3::Y);
        }
        n
    }

    pub fn load_obj(text: &str) -> TriMesh {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut m = TriMesh::default();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("v") => {
                    let c: Vec<f32> = it.take(3).filter_map(|s| s.parse().ok()).collect();
                    if c.len() == 3 {
                        m.verts.push(Vec3::new(c[0], c[1], c[2]));
                    }
                }
                Some("f") => {
                    let n = m.verts.len() as i64;
                    let idx: Vec<u32> = it
                        .filter_map(|s| s.split('/').next().and_then(|a| a.parse::<i64>().ok()))
                        .map(|i| {
                            if i < 0 {
                                (n + i) as u32
                            } else {
                                (i - 1) as u32
                            }
                        })
                        .collect();
                    for k in 1..idx.len().saturating_sub(1) {
                        m.tris.push([idx[0], idx[k], idx[k + 1]]);
                    }
                }
                _ => {}
            }
        }
        let nv = m.verts.len() as u32;
        m.tris.retain(|t| t.iter().all(|&i| i < nv));
        m.fit_unit();
        m
    }

    /// Reads 3DFACE entities of an ASCII DXF file (Z up is turned into Y up).
    pub fn load_dxf(text: &str) -> TriMesh {
        let mut m = TriMesh::default();
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
        let mut i = 0;
        while i + 1 < lines.len() {
            if lines[i] == "0" && lines[i + 1] == "3DFACE" {
                let mut pts = [[0.0f32; 3]; 4];
                i += 2;
                while i + 1 < lines.len() && lines[i] != "0" {
                    if let Ok(code) = lines[i].parse::<i32>() {
                        if let Ok(val) = lines[i + 1].parse::<f32>() {
                            let (pi, ci) = match code {
                                10..=13 => ((code - 10) as usize, 0),
                                20..=23 => ((code - 20) as usize, 1),
                                30..=33 => ((code - 30) as usize, 2),
                                _ => (9, 0),
                            };
                            if pi < 4 {
                                pts[pi][ci] = val;
                            }
                        }
                    }
                    i += 2;
                }
                let b = m.verts.len() as u32;
                for p in pts {
                    m.verts.push(Vec3::new(p[0], p[2], -p[1]));
                }
                m.tris.push([b, b + 1, b + 2]);
                if pts[3] != pts[2] {
                    m.tris.push([b, b + 2, b + 3]);
                }
            } else {
                i += 1;
            }
        }
        m.fit_unit();
        m
    }

    pub fn icosphere(subdiv: usize) -> TriMesh {
        let t = (1.0 + 5f32.sqrt()) / 2.0;
        let mut verts: Vec<Vec3> = [
            [-1.0, t, 0.0],
            [1.0, t, 0.0],
            [-1.0, -t, 0.0],
            [1.0, -t, 0.0],
            [0.0, -1.0, t],
            [0.0, 1.0, t],
            [0.0, -1.0, -t],
            [0.0, 1.0, -t],
            [t, 0.0, -1.0],
            [t, 0.0, 1.0],
            [-t, 0.0, -1.0],
            [-t, 0.0, 1.0],
        ]
        .iter()
        .map(|p| Vec3::from(*p).normalize())
        .collect();
        let mut tris: Vec<[u32; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        for _ in 0..subdiv {
            let mut cache = std::collections::HashMap::new();
            let mut mid = |a: u32, b: u32, verts: &mut Vec<Vec3>| -> u32 {
                let key = (a.min(b), a.max(b));
                *cache.entry(key).or_insert_with(|| {
                    verts.push(((verts[a as usize] + verts[b as usize]) * 0.5).normalize());
                    (verts.len() - 1) as u32
                })
            };
            let mut nt = Vec::with_capacity(tris.len() * 4);
            for t in &tris {
                let a = mid(t[0], t[1], &mut verts);
                let b = mid(t[1], t[2], &mut verts);
                let c = mid(t[2], t[0], &mut verts);
                nt.push([t[0], a, c]);
                nt.push([t[1], b, a]);
                nt.push([t[2], c, b]);
                nt.push([a, b, c]);
            }
            tris = nt;
        }
        TriMesh {
            verts,
            tris,
            smooth: true,
        }
    }

    /// A lumpy stone.
    pub fn stone(seed: u64) -> TriMesh {
        let mut m = TriMesh::icosphere(3);
        let mut rng = Rng::new(seed);
        let off = Vec3::new(
            rng.range(0.0, 100.0),
            rng.range(0.0, 100.0),
            rng.range(0.0, 100.0),
        );
        let squash = Vec3::new(
            rng.range(0.8, 1.2),
            rng.range(0.55, 0.9),
            rng.range(0.8, 1.2),
        );
        for v in &mut m.verts {
            let d = 1.0 + fbm(*v * 1.6 + off, 4) * 0.35;
            *v = *v * d * squash;
        }
        m.fit_unit();
        m.smooth = true;
        m
    }
}

#[derive(Clone, Copy)]
pub struct Aabb {
    pub lo: Vec3,
    pub hi: Vec3,
}

impl Aabb {
    pub fn empty() -> Self {
        Aabb {
            lo: Vec3::splat(f32::MAX),
            hi: Vec3::splat(f32::MIN),
        }
    }
    pub fn grow(&mut self, p: Vec3) {
        self.lo = self.lo.min(p);
        self.hi = self.hi.max(p);
    }
    pub fn merge(&mut self, o: &Aabb) {
        self.lo = self.lo.min(o.lo);
        self.hi = self.hi.max(o.hi);
    }
    #[inline]
    pub fn hit(&self, o: Vec3, inv: Vec3, tmax: f32) -> Option<f32> {
        let t0 = (self.lo - o) * inv;
        let t1 = (self.hi - o) * inv;
        let tn = t0.min(t1).max_element().max(0.0);
        let tf = t0.max(t1).min_element().min(tmax);
        if tn <= tf {
            Some(tn)
        } else {
            None
        }
    }
}

struct Node {
    bb: Aabb,
    left: u32,
    count: u32, // 0 = inner node (children left, left+1)
}

pub struct Bvh {
    nodes: Vec<Node>,
    tri_idx: Vec<u32>,
    v0: Vec<Vec3>,
    e1: Vec<Vec3>,
    e2: Vec<Vec3>,
    fnorm: Vec<Vec3>,
    vnorm: Option<(Vec<Vec3>, Vec<[u32; 3]>)>,
}

impl Bvh {
    pub fn build(m: &TriMesh) -> Bvh {
        let n = m.tris.len();
        let mut v0 = Vec::with_capacity(n);
        let mut e1 = Vec::with_capacity(n);
        let mut e2 = Vec::with_capacity(n);
        let mut fnorm = Vec::with_capacity(n);
        let mut cent = Vec::with_capacity(n);
        let mut boxes = Vec::with_capacity(n);
        for t in &m.tris {
            let a = m.verts[t[0] as usize];
            let b = m.verts[t[1] as usize];
            let c = m.verts[t[2] as usize];
            v0.push(a);
            e1.push(b - a);
            e2.push(c - a);
            fnorm.push((b - a).cross(c - a).normalize_or(Vec3::Y));
            cent.push((a + b + c) / 3.0);
            let mut bb = Aabb::empty();
            bb.grow(a);
            bb.grow(b);
            bb.grow(c);
            boxes.push(bb);
        }
        let mut bvh = Bvh {
            nodes: Vec::new(),
            tri_idx: (0..n as u32).collect(),
            v0,
            e1,
            e2,
            fnorm,
            vnorm: if m.smooth {
                Some((m.vertex_normals(), m.tris.clone()))
            } else {
                None
            },
        };
        if n > 0 {
            bvh.nodes.push(Node {
                bb: Aabb::empty(),
                left: 0,
                count: 0,
            });
            bvh.split(0, 0, n, &cent, &boxes);
        }
        bvh
    }

    fn split(&mut self, node: usize, start: usize, end: usize, cent: &[Vec3], boxes: &[Aabb]) {
        let mut bb = Aabb::empty();
        let mut cb = Aabb::empty();
        for &i in &self.tri_idx[start..end] {
            bb.merge(&boxes[i as usize]);
            cb.grow(cent[i as usize]);
        }
        self.nodes[node].bb = bb;
        let count = end - start;
        if count <= 4 {
            self.nodes[node].left = start as u32;
            self.nodes[node].count = count as u32;
            return;
        }
        let ext = cb.hi - cb.lo;
        let axis = if ext.x > ext.y && ext.x > ext.z {
            0
        } else if ext.y > ext.z {
            1
        } else {
            2
        };
        let mid = start + count / 2;
        self.tri_idx[start..end].select_nth_unstable_by(count / 2, |a, b| {
            cent[*a as usize][axis]
                .partial_cmp(&cent[*b as usize][axis])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let l = self.nodes.len();
        self.nodes.push(Node {
            bb: Aabb::empty(),
            left: 0,
            count: 0,
        });
        self.nodes.push(Node {
            bb: Aabb::empty(),
            left: 0,
            count: 0,
        });
        self.nodes[node].left = l as u32;
        self.nodes[node].count = 0;
        self.split(l, start, mid, cent, boxes);
        self.split(l + 1, mid, end, cent, boxes);
    }

    /// Returns (t, normal).
    pub fn intersect(&self, o: Vec3, d: Vec3, tmax: f32) -> Option<(f32, Vec3)> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = Vec3::ONE / d;
        let mut best = tmax;
        let mut hit: Option<(u32, f32, f32)> = None;
        let mut stack = [0u32; 64];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let nd = &self.nodes[stack[sp] as usize];
            if nd.bb.hit(o, inv, best).is_none() {
                continue;
            }
            if nd.count > 0 {
                for k in nd.left..nd.left + nd.count {
                    let ti = self.tri_idx[k as usize] as usize;
                    let e1 = self.e1[ti];
                    let e2 = self.e2[ti];
                    let pv = d.cross(e2);
                    let det = e1.dot(pv);
                    if det.abs() < 1e-9 {
                        continue;
                    }
                    let id = 1.0 / det;
                    let tv = o - self.v0[ti];
                    let u = tv.dot(pv) * id;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let qv = tv.cross(e1);
                    let v = d.dot(qv) * id;
                    if v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let t = e2.dot(qv) * id;
                    if t > 1e-5 && t < best {
                        best = t;
                        hit = Some((ti as u32, u, v));
                    }
                }
            } else if sp + 2 <= stack.len() {
                stack[sp] = nd.left;
                stack[sp + 1] = nd.left + 1;
                sp += 2;
            }
        }
        hit.map(|(ti, u, v)| {
            let n = match &self.vnorm {
                Some((vn, tris)) => {
                    let t = tris[ti as usize];
                    (vn[t[0] as usize] * (1.0 - u - v)
                        + vn[t[1] as usize] * u
                        + vn[t[2] as usize] * v)
                        .normalize_or(self.fnorm[ti as usize])
                }
                None => self.fnorm[ti as usize],
            };
            (best, n)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Folder with classic CD content, from the Y2K3D_LIBRARY environment variable.
    fn root() -> String {
        std::env::var("Y2K3D_LIBRARY")
            .expect("set Y2K3D_LIBRARY to a folder with the old library files")
    }

    #[test]
    #[ignore]
    fn reads_the_old_cd_files() {
        let mut ok_tif = 0;
        let mut bad = Vec::new();
        for dir in [
            "Terrain  Maps",
            "Picture Textures",
            "Picture Objects",
            "Light Gels",
        ] {
            for e in std::fs::read_dir(format!(r"{}\{dir}", root())).unwrap() {
                let p = e.unwrap().path();
                if p.extension()
                    .map(|x| x.eq_ignore_ascii_case("tif"))
                    .unwrap_or(false)
                {
                    match crate::picture::open(&p) {
                        Ok(_) => ok_tif += 1,
                        Err(err) => bad.push(format!("{}: {err}", p.display())),
                    }
                }
            }
        }
        eprintln!("TIF ok: {ok_tif}, unreadable: {}", bad.len());
        for b in bad.iter().take(10) {
            eprintln!("  {b}");
        }
        let mut objs = 0;
        for e in std::fs::read_dir(format!(r"{}\OBJ Models", root())).unwrap() {
            let p = e.unwrap().path();
            if p.extension()
                .map(|x| x.eq_ignore_ascii_case("obj"))
                .unwrap_or(false)
            {
                let m = TriMesh::load_obj(&String::from_utf8_lossy(&std::fs::read(&p).unwrap()));
                eprintln!(
                    "OBJ {}: {} tris",
                    p.file_name().unwrap().to_string_lossy(),
                    m.tris.len()
                );
                objs += (m.tris.len() > 0) as usize;
            }
        }
        for f in [
            r"Viewpoint DXF's\DXF Models\aheart1l.dxf",
            r"ONYX\DRYMOPHL.DXF",
        ] {
            let m = TriMesh::load_dxf(&String::from_utf8_lossy(
                &std::fs::read(format!(r"{}\{f}", root())).unwrap(),
            ));
            eprintln!("DXF {f}: {} tris", m.tris.len());
        }
        assert!(objs > 0);
    }
}
