//! Online library: free (CC0) textures and 3D models from Poly Haven and ambientCG, browsed,
//! downloaded and cached in the portable data folder next to the program.

use crate::i18n::tr;
use crate::mesh::TriMesh;
use egui::{Color32, RichText, Sense, TextureHandle, TextureOptions, Vec2};
use glam::{Mat4, Vec3};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Source {
    PolyHaven,
    AmbientCg,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AssetKind {
    Texture,
    Model,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub source: Source,
    pub kind: AssetKind,
    pub id: String,
    pub name: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub thumb: String,
}

/// Words that point to the glossy, metallic, synthetic look of Y2K-era 3D.
const Y2K_WORDS: [&str; 16] = [
    "metal",
    "chrome",
    "steel",
    "alumin",
    "plastic",
    "glass",
    "terrazzo",
    "rubber",
    "foil",
    "carbon",
    "neon",
    "holo",
    "iridescent",
    "lacquer",
    "glossy",
    "mirror",
];

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|c| {
            c.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_lowercase()))
                .collect()
        })
        .unwrap_or_default()
}

impl Asset {
    fn is_y2k(&self) -> bool {
        let hay = format!(
            "{} {} {}",
            self.name,
            self.categories.join(" "),
            self.tags.join(" ")
        )
        .to_lowercase();
        Y2K_WORDS.iter().any(|w| hay.contains(w))
    }
}

impl Asset {
    fn key(&self) -> String {
        format!("{:?}/{}", self.source, self.id)
    }
}

/// What the window asks the main program to do.
pub enum LibAction {
    ApplyTexture {
        diffuse: PathBuf,
        name: String,
    },
    NewTerrain {
        height: PathBuf,
        diffuse: Option<PathBuf>,
        name: String,
    },
    AddModel {
        mesh: TriMesh,
        color: [f32; 3],
        name: String,
    },
}

pub fn library_dir() -> PathBuf {
    crate::settings::data_dir().join("Bibliothek")
}

// ---------------------------------------------------------------------------------------------
// HTTP

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    ureq::config::Config::builder()
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .user_agent("Y2K-3D (https://github.com/Bratwurstexe/Y2K-3D)")
        .build()
        .into()
}

fn get_json(url: &str) -> Result<Value, String> {
    agent()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())
}

fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    agent()
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(400 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

fn download_to(url: &str, path: &Path) -> Result<(), String> {
    if path.is_file() {
        return Ok(());
    }
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let b = get_bytes(url)?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, b).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Catalogues

fn polyhaven_index(kind: AssetKind) -> Result<Vec<Asset>, String> {
    let t = if kind == AssetKind::Texture {
        "textures"
    } else {
        "models"
    };
    let v = get_json(&format!("https://api.polyhaven.com/assets?t={t}"))?;
    let obj = v.as_object().ok_or("unexpected answer")?;
    let mut out: Vec<Asset> = obj
        .iter()
        .map(|(id, a)| Asset {
            source: Source::PolyHaven,
            kind,
            id: id.clone(),
            name: a["name"].as_str().unwrap_or(id).to_string(),
            tags: strings(&a["tags"]),
            categories: a["categories"]
                .as_array()
                .map(|c| {
                    c.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            thumb: format!(
                "https://cdn.polyhaven.com/asset_img/thumbs/{id}.png?width=128&height=128"
            ),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn ambientcg_index() -> Result<Vec<Asset>, String> {
    let mut out = Vec::new();
    for offset in [0, 250, 500, 750] {
        let v = get_json(&format!(
            "https://ambientcg.com/api/v2/full_json?type=Material&sort=Popular&limit=250&offset={offset}&include=previewData,tagData"
        ))?;
        let Some(list) = v["foundAssets"].as_array() else {
            break;
        };
        if list.is_empty() {
            break;
        }
        for a in list {
            let id = a["assetId"].as_str().unwrap_or_default().to_string();
            if id.is_empty() {
                continue;
            }
            out.push(Asset {
                source: Source::AmbientCg,
                kind: AssetKind::Texture,
                name: a["displayName"].as_str().unwrap_or(&id).to_string(),
                tags: strings(&a["tags"]),
                categories: a["displayCategory"]
                    .as_str()
                    .map(|c| vec![c.to_lowercase()])
                    .unwrap_or_default(),
                thumb: a["previewImage"]["128-PNG"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                id,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Downloads

/// Diffuse colour map and (if there is one) height map of a texture.
fn fetch_texture(a: &Asset) -> Result<(PathBuf, Option<PathBuf>), String> {
    let dir = library_dir().join("Texturen");
    match a.source {
        Source::PolyHaven => {
            let diff = dir.join(format!("ph_{}_farbe.jpg", a.id));
            let disp = dir.join(format!("ph_{}_hoehe.jpg", a.id));
            if !diff.is_file() {
                let v = get_json(&format!("https://api.polyhaven.com/files/{}", a.id))?;
                let url = v["Diffuse"]["1k"]["jpg"]["url"]
                    .as_str()
                    .ok_or("no colour map")?;
                download_to(url, &diff)?;
                if let Some(u) = v["Displacement"]["1k"]["jpg"]["url"].as_str() {
                    let _ = download_to(u, &disp);
                }
            }
            Ok((diff, disp.is_file().then_some(disp)))
        }
        Source::AmbientCg => {
            let diff = dir.join(format!("acg_{}_farbe.jpg", a.id));
            let disp = dir.join(format!("acg_{}_hoehe.jpg", a.id));
            if !diff.is_file() {
                let bytes = get_bytes(&format!(
                    "https://ambientcg.com/get?file={}_1K-JPG.zip",
                    a.id
                ))?;
                let mut zip =
                    zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                for i in 0..zip.len() {
                    let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
                    let n = f.name().map(|n| n.to_lowercase()).unwrap_or_default();
                    let target = if n.ends_with("_color.jpg") {
                        Some(&diff)
                    } else if n.ends_with("_displacement.jpg") {
                        Some(&disp)
                    } else {
                        None
                    };
                    if let Some(t) = target {
                        let mut buf = Vec::new();
                        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                        std::fs::write(t, buf).map_err(|e| e.to_string())?;
                    }
                }
                if !diff.is_file() {
                    return Err("no colour map in the download".into());
                }
            }
            Ok((diff, disp.is_file().then_some(disp)))
        }
    }
}

/// Downloads a Poly Haven model (glTF with its buffers and textures) and loads it.
fn fetch_model(a: &Asset) -> Result<(TriMesh, [f32; 3]), String> {
    let dir = library_dir().join("Modelle").join(&a.id);
    let main = dir.join(format!("{}.gltf", a.id));
    if !main.is_file() {
        let v = get_json(&format!("https://api.polyhaven.com/files/{}", a.id))?;
        let g = &v["gltf"]["1k"]["gltf"];
        let url = g["url"].as_str().ok_or("no glTF file")?;
        if let Some(inc) = g["include"].as_object() {
            for (rel, f) in inc {
                if rel.contains("..") {
                    continue;
                }
                if let Some(u) = f["url"].as_str() {
                    download_to(u, &dir.join(rel))?;
                }
            }
        }
        download_to(url, &main)?;
    }
    let mesh = load_gltf(&main)?;
    // average colour of the colour map, if there is one
    let mut color = [0.7, 0.7, 0.7];
    if let Ok(rd) = std::fs::read_dir(dir.join("textures")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_lowercase();
            if n.contains("_diff") {
                if let Ok(img) = crate::picture::open(e.path()) {
                    let small = img.thumbnail(32, 32).to_rgb8();
                    let mut sum = [0f32; 3];
                    for p in small.pixels() {
                        for k in 0..3 {
                            sum[k] += p.0[k] as f32 / 255.0;
                        }
                    }
                    let n = (small.width() * small.height()).max(1) as f32;
                    color = [sum[0] / n, sum[1] / n, sum[2] / n];
                }
                break;
            }
        }
    }
    Ok((mesh, color))
}

/// Reads every triangle of a glTF / GLB file (node transforms applied) into one mesh.
pub fn load_gltf(path: &Path) -> Result<TriMesh, String> {
    let (doc, buffers, _) = gltf::import(path).map_err(|e| e.to_string())?;
    let mut m = TriMesh {
        smooth: true,
        ..Default::default()
    };
    fn walk(node: gltf::Node, parent: Mat4, buffers: &[gltf::buffer::Data], m: &mut TriMesh) {
        let local = Mat4::from_cols_array_2d(&node.transform().matrix());
        let world = parent * local;
        if let Some(mesh) = node.mesh() {
            for prim in mesh.primitives() {
                if prim.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }
                let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
                let Some(pos) = reader.read_positions() else {
                    continue;
                };
                let base = m.verts.len() as u32;
                let count0 = m.verts.len();
                m.verts
                    .extend(pos.map(|p| world.transform_point3(Vec3::from(p))));
                let n = (m.verts.len() - count0) as u32;
                match reader.read_indices() {
                    Some(ix) => {
                        let ix: Vec<u32> = ix.into_u32().collect();
                        for t in ix.chunks_exact(3) {
                            m.tris.push([base + t[0], base + t[1], base + t[2]]);
                        }
                    }
                    None => {
                        for k in (0..n.saturating_sub(2)).step_by(3) {
                            m.tris.push([base + k, base + k + 1, base + k + 2]);
                        }
                    }
                }
            }
        }
        for c in node.children() {
            walk(c, world, buffers, m);
        }
    }
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .ok_or("no scene in the file")?;
    for n in scene.nodes() {
        walk(n, Mat4::IDENTITY, &buffers, &mut m);
    }
    if m.tris.is_empty() {
        return Err("no triangles in the file".into());
    }
    m.fit_unit();
    Ok(m)
}

/// Reads an STL file (binary or text); Z-up is turned into Y-up.
pub fn load_stl(bytes: &[u8]) -> Result<TriMesh, String> {
    let mut m = TriMesh::default();
    let push = |p: [f32; 3], m: &mut TriMesh| m.verts.push(Vec3::new(p[0], p[2], -p[1]));
    let binary = bytes.len() >= 84 && {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        84 + n * 50 == bytes.len()
    };
    if binary {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        for i in 0..n {
            let o = 84 + i * 50 + 12;
            let f = |k: usize| {
                f32::from_le_bytes([
                    bytes[o + k * 4],
                    bytes[o + k * 4 + 1],
                    bytes[o + k * 4 + 2],
                    bytes[o + k * 4 + 3],
                ])
            };
            let b = m.verts.len() as u32;
            for v in 0..3 {
                push([f(v * 3), f(v * 3 + 1), f(v * 3 + 2)], &mut m);
            }
            m.tris.push([b, b + 1, b + 2]);
        }
    } else {
        let text = String::from_utf8_lossy(bytes);
        let mut pending = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if it.next() == Some("vertex") {
                let c: Vec<f32> = it.take(3).filter_map(|s| s.parse().ok()).collect();
                if c.len() == 3 {
                    pending.push([c[0], c[1], c[2]]);
                }
                if pending.len() == 3 {
                    let b = m.verts.len() as u32;
                    for p in pending.drain(..) {
                        push(p, &mut m);
                    }
                    m.tris.push([b, b + 1, b + 2]);
                }
            }
        }
    }
    if m.tris.is_empty() {
        return Err("no triangles in the file".into());
    }
    m.fit_unit();
    Ok(m)
}

// ---------------------------------------------------------------------------------------------
// Thumbnails (a few background workers)

struct Thumbs {
    queue: Mutex<VecDeque<(String, String)>>,
    done: Mutex<HashMap<String, Option<egui::ColorImage>>>,
    started: AtomicBool,
}

impl Thumbs {
    fn new() -> Arc<Thumbs> {
        Arc::new(Thumbs {
            queue: Mutex::new(VecDeque::new()),
            done: Mutex::new(HashMap::new()),
            started: AtomicBool::new(false),
        })
    }

    fn start(self: &Arc<Self>, ctx: &egui::Context) {
        if self.started.swap(true, Ordering::Relaxed) {
            return;
        }
        for _ in 0..6 {
            let me = self.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let job = me.queue.lock().unwrap().pop_back();
                match job {
                    Some((key, url)) => {
                        let img = get_bytes(&url)
                            .ok()
                            .and_then(|b| image::load_from_memory(&b).ok())
                            .map(|i| {
                                let i = i.thumbnail(128, 128).to_rgba8();
                                egui::ColorImage::from_rgba_unmultiplied(
                                    [i.width() as usize, i.height() as usize],
                                    i.as_raw(),
                                )
                            });
                        me.done.lock().unwrap().insert(key, img);
                        ctx.request_repaint();
                    }
                    None => std::thread::sleep(std::time::Duration::from_millis(120)),
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The window

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    PolyHavenTextures,
    AmbientCgTextures,
    Models,
    Links,
}

type Pending<T> = Arc<Mutex<Option<Result<T, String>>>>;

enum Job {
    Texture(Pending<(PathBuf, Option<PathBuf>)>, Asset, bool),
    Model(Pending<(TriMesh, [f32; 3])>, Asset),
}

pub struct LibraryWindow {
    pub open: bool,
    tab: Tab,
    search: String,
    category: String,
    y2k_only: bool,
    indexes: HashMap<u8, Pending<Vec<Asset>>>,
    thumbs: Arc<Thumbs>,
    textures: HashMap<String, TextureHandle>,
    requested: HashSet<String>,
    selected: Option<Asset>,
    job: Option<Job>,
    status: String,
}

fn spawn<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Pending<T> {
    let p: Pending<T> = Arc::new(Mutex::new(None));
    let p2 = p.clone();
    std::thread::spawn(move || {
        let r = f();
        *p2.lock().unwrap() = Some(r);
    });
    p
}

impl LibraryWindow {
    pub fn new() -> LibraryWindow {
        LibraryWindow {
            open: false,
            tab: Tab::PolyHavenTextures,
            search: String::new(),
            category: String::new(),
            y2k_only: true,
            indexes: HashMap::new(),
            thumbs: Thumbs::new(),
            textures: HashMap::new(),
            requested: HashSet::new(),
            selected: None,
            job: None,
            status: String::new(),
        }
    }

    fn index(&mut self, tab: Tab) -> &Pending<Vec<Asset>> {
        let key = tab as u8;
        self.indexes.entry(key).or_insert_with(|| match tab {
            Tab::PolyHavenTextures => spawn(|| polyhaven_index(AssetKind::Texture)),
            Tab::AmbientCgTextures => spawn(ambientcg_index),
            _ => spawn(|| polyhaven_index(AssetKind::Model)),
        })
    }

    pub fn show(&mut self, ctx: &egui::Context, has_selection: bool) -> Option<LibAction> {
        if !self.open {
            return None;
        }
        self.thumbs.start(ctx);
        let mut action = self.poll_job();
        let mut open = self.open;
        egui::Window::new(tr("Online-Bibliothek"))
            .open(&mut open)
            .default_size([860.0, 600.0])
            .min_width(560.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (t, label) in [
                        (Tab::PolyHavenTextures, tr("Texturen · Poly Haven")),
                        (Tab::AmbientCgTextures, tr("Texturen · ambientCG")),
                        (Tab::Models, tr("3D-Modelle · Poly Haven")),
                        (Tab::Links, tr("Weitere freie Quellen")),
                    ] {
                        if ui.selectable_label(self.tab == t, RichText::new(label).strong()).clicked() && self.tab != t {
                            self.tab = t;
                            self.category.clear();
                            self.selected = None;
                        }
                    }
                });
                ui.label(
                    RichText::new(tr("Alle Inhalte stehen unter CC0 (gemeinfrei): frei nutzbar, auch kommerziell, ohne Namensnennung. Heruntergeladenes wird im Ordner „Daten\\Bibliothek“ neben dem Programm gespeichert."))
                        .small()
                        .weak(),
                );
                ui.separator();
                if self.tab == Tab::Links {
                    links(ui);
                    return;
                }
                if let Some(a) = self.details(ui, has_selection) {
                    action = Some(a);
                }
                ui.separator();
                self.grid(ui, ctx);
            });
        self.open = open;
        action
    }

    fn details(&mut self, ui: &mut egui::Ui, has_selection: bool) -> Option<LibAction> {
        let mut start: Option<(Asset, bool, bool)> = None; // (asset, model?, as terrain?)
        ui.horizontal(|ui| {
            ui.label(tr("Suchen"));
            ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(180.0).hint_text(tr("z. B. rock, sand, grass, tree")));
            if !self.category.is_empty() && ui.button(format!("✖ {}", self.category)).clicked() {
                self.category.clear();
            }
            ui.separator();
            let busy = self.job.is_some();
            match self.selected.clone() {
                None => {
                    ui.label(RichText::new(tr("Eintrag anklicken, um ihn zu laden")).weak());
                }
                Some(a) => {
                    ui.label(RichText::new(&a.name).strong());
                    if a.kind == AssetKind::Model {
                        if ui.add_enabled(!busy, egui::Button::new(tr("In die Szene laden"))).clicked() {
                            start = Some((a, true, false));
                        }
                    } else {
                        if ui
                            .add_enabled(!busy && has_selection, egui::Button::new(tr("Auf Auswahl anwenden")))
                            .on_disabled_hover_text(tr("Erst ein Objekt in der Szene auswählen"))
                            .clicked()
                        {
                            start = Some((a.clone(), false, false));
                        }
                        if ui
                            .add_enabled(!busy, egui::Button::new(tr("Als Gelände (Höhenkarte)")))
                            .on_hover_text(tr("Erzeugt ein neues Gelände aus der Höhenkarte der Textur und färbt es damit ein"))
                            .clicked()
                        {
                            start = Some((a, false, true));
                        }
                    }
                }
            }
            if busy {
                ui.spinner();
            }
        });
        if !self.status.is_empty() {
            ui.label(
                RichText::new(&self.status)
                    .small()
                    .color(Color32::from_rgb(230, 180, 120)),
            );
        }
        if let Some((a, model, terrain)) = start {
            self.status = crate::trf!("„{}“ wird geladen…", a.name);
            if model {
                let a2 = a.clone();
                self.job = Some(Job::Model(spawn(move || fetch_model(&a2)), a));
            } else {
                let a2 = a.clone();
                self.job = Some(Job::Texture(spawn(move || fetch_texture(&a2)), a, terrain));
            }
        }
        None
    }

    fn poll_job(&mut self) -> Option<LibAction> {
        let job = self.job.take()?;
        match job {
            Job::Texture(p, a, terrain) => {
                let r = p.lock().unwrap().take();
                match r {
                    None => {
                        self.job = Some(Job::Texture(p, a, terrain));
                        None
                    }
                    Some(Err(e)) => {
                        self.status = crate::trf!("Laden fehlgeschlagen: {}", e);
                        None
                    }
                    Some(Ok((diff, disp))) => {
                        if terrain {
                            match disp {
                                Some(h) => {
                                    self.status = crate::trf!("Gelände aus „{}“ erzeugt", a.name);
                                    Some(LibAction::NewTerrain {
                                        height: h,
                                        diffuse: Some(diff),
                                        name: a.name,
                                    })
                                }
                                None => {
                                    self.status = tr("Diese Textur hat keine Höhenkarte – sie wird stattdessen als Graustufen-Höhenkarte genutzt.").into();
                                    Some(LibAction::NewTerrain {
                                        height: diff.clone(),
                                        diffuse: Some(diff),
                                        name: a.name,
                                    })
                                }
                            }
                        } else {
                            self.status = crate::trf!("„{}“ angewendet", a.name);
                            Some(LibAction::ApplyTexture {
                                diffuse: diff,
                                name: a.name,
                            })
                        }
                    }
                }
            }
            Job::Model(p, a) => {
                let r = p.lock().unwrap().take();
                match r {
                    None => {
                        self.job = Some(Job::Model(p, a));
                        None
                    }
                    Some(Err(e)) => {
                        self.status = crate::trf!("Laden fehlgeschlagen: {}", e);
                        None
                    }
                    Some(Ok((mesh, color))) => {
                        self.status = crate::trf!(
                            "„{}“ in die Szene geladen ({} Dreiecke)",
                            a.name,
                            mesh.tris.len()
                        );
                        Some(LibAction::AddModel {
                            mesh,
                            color,
                            name: a.name,
                        })
                    }
                }
            }
        }
    }

    fn grid(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let tab = self.tab;
        let state = self.index(tab).lock().unwrap().as_ref().map(|r| r.clone());
        let assets = match state {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr("Katalog wird geladen…"));
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
            Some(Err(e)) => {
                ui.label(
                    RichText::new(crate::trf!("Keine Verbindung zur Bibliothek: {}", e))
                        .color(Color32::from_rgb(240, 150, 110)),
                );
                if ui.button(tr("Erneut versuchen")).clicked() {
                    self.indexes.remove(&(tab as u8));
                }
                return;
            }
            Some(Ok(a)) => a,
        };
        // categories of this catalogue
        let mut cats: Vec<(String, usize)> = {
            let mut m: HashMap<&str, usize> = HashMap::new();
            for a in &assets {
                for c in &a.categories {
                    *m.entry(c.as_str()).or_default() += 1;
                }
            }
            m.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
        };
        cats.sort_by(|a, b| b.1.cmp(&a.1));
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(
                    self.y2k_only,
                    RichText::new(tr("Y2K-Look")).strong().color(Color32::from_rgb(255, 150, 220)),
                )
                .on_hover_text(tr(
                    "Nur Glänzendes, Metallisches, Glas, Kunststoff und Fliesen – passend zur Y2K-Ästhetik",
                ))
                .clicked()
            {
                self.y2k_only = !self.y2k_only;
            }
            ui.label(RichText::new(tr("Kategorien:")).small().weak());
            for (c, n) in cats.iter().take(18) {
                if ui
                    .selectable_label(
                        &self.category == c,
                        RichText::new(format!("{c} ({n})")).small(),
                    )
                    .clicked()
                {
                    self.category = if &self.category == c {
                        String::new()
                    } else {
                        c.clone()
                    };
                }
            }
        });
        let q = self.search.to_lowercase();
        let shown: Vec<&Asset> = assets
            .iter()
            .filter(|a| !self.y2k_only || a.kind == AssetKind::Model || a.is_y2k())
            .filter(|a| {
                self.category.is_empty() || a.categories.iter().any(|c| c == &self.category)
            })
            .filter(|a| {
                q.is_empty()
                    || a.name.to_lowercase().contains(&q)
                    || a.id.to_lowercase().contains(&q)
                    || a.categories.iter().any(|c| c.contains(&q))
            })
            .collect();
        ui.label(
            RichText::new(crate::trf!("{} Einträge", shown.len()))
                .small()
                .weak(),
        );
        // pick up finished thumbnails
        {
            let mut done = self.thumbs.done.lock().unwrap();
            for (k, img) in done.drain() {
                if let Some(img) = img {
                    let t = ctx.load_texture(format!("lib-{k}"), img, TextureOptions::LINEAR);
                    self.textures.insert(k, t);
                }
            }
        }
        let cell = 112.0;
        let per_row = ((ui.available_width() / (cell + 8.0)).floor() as usize).max(1);
        let rows = shown.len().div_ceil(per_row);
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, cell + 30.0, rows, |ui, range| {
                for r in range {
                    ui.horizontal(|ui| {
                        for a in shown.iter().skip(r * per_row).take(per_row) {
                            let key = a.key();
                            if !self.requested.contains(&key) && !a.thumb.is_empty() {
                                self.requested.insert(key.clone());
                                self.thumbs
                                    .queue
                                    .lock()
                                    .unwrap()
                                    .push_back((key.clone(), a.thumb.clone()));
                            }
                            let sel = self
                                .selected
                                .as_ref()
                                .map(|s| s.key() == key)
                                .unwrap_or(false);
                            let (rect, resp) = ui
                                .allocate_exact_size(Vec2::new(cell, cell + 24.0), Sense::click());
                            let p = ui.painter_at(rect);
                            let bg = if sel {
                                Color32::from_rgb(120, 78, 44)
                            } else if resp.hovered() {
                                Color32::from_gray(62)
                            } else {
                                Color32::from_gray(44)
                            };
                            p.rect_filled(rect, 6.0, bg);
                            let img_rect = egui::Rect::from_min_size(
                                rect.min + Vec2::splat(6.0),
                                Vec2::splat(cell - 12.0),
                            );
                            match self.textures.get(&key) {
                                Some(t) => {
                                    p.image(
                                        t.id(),
                                        img_rect,
                                        egui::Rect::from_min_max(
                                            egui::Pos2::ZERO,
                                            egui::Pos2::new(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                }
                                None => {
                                    p.rect_filled(img_rect, 4.0, Color32::from_gray(30));
                                }
                            }
                            let name: String = a.name.chars().take(17).collect();
                            p.text(
                                egui::Pos2::new(rect.center().x, rect.bottom() - 12.0),
                                egui::Align2::CENTER_CENTER,
                                name,
                                egui::FontId::proportional(11.0),
                                Color32::from_gray(220),
                            );
                            let resp = resp.on_hover_text(format!(
                                "{}\n{}",
                                a.name,
                                a.categories.join(", ")
                            ));
                            if resp.clicked() {
                                clicked = Some((*a).clone());
                            }
                        }
                    });
                }
            });
        if let Some(a) = clicked {
            self.selected = Some(a);
            self.status.clear();
        }
    }
}

fn links(ui: &mut egui::Ui) {
    ui.label(tr("Weitere kostenlose Quellen, deren Dateien Y2K-3D öffnen kann (im Browser herunterladen, dann über „Datei“ bzw. die Labore laden):"));
    ui.add_space(6.0);
    let rows: [(&str, &str, &str); 6] = [
        ("Poly Haven", "https://polyhaven.com", "Texturen, 3D-Modelle (glTF) – CC0"),
        ("ambientCG", "https://ambientcg.com", "über 2000 Materialien und Texturen – CC0"),
        ("Kenney", "https://kenney.nl/assets", "Low-Poly-Modellpakete (OBJ, GLB) – CC0"),
        ("Heightmapper", "https://tangrams.github.io/heightmapper/", "Höhenkarten echter Landschaften als PNG (Geländedaten: OpenStreetMap-Mitwirkende / Nextzen) – für den Gelände-Editor, Reiter „Bild“"),
        ("Smithsonian 3D", "https://3d.si.edu", "Museumsobjekte, viele als CC0 (glTF)"),
        ("NASA 3D Resources", "https://science.nasa.gov/3d-resources/", "Modelle von Raumfahrzeugen und Planeten (STL, glTF), frei nutzbar"),
    ];
    egui::Grid::new("links")
        .num_columns(2)
        .spacing([16.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            for (name, url, what) in rows {
                ui.hyperlink_to(RichText::new(name).strong(), url);
                ui.label(tr(what));
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.label(
        RichText::new(tr(
            "Lizenzen der einzelnen Dateien bitte auf der jeweiligen Seite prüfen.",
        ))
        .small()
        .weak(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn online_sources_work() {
        let tex = polyhaven_index(AssetKind::Texture).unwrap();
        let models = polyhaven_index(AssetKind::Model).unwrap();
        let acg = ambientcg_index().unwrap();
        eprintln!(
            "poly haven textures {}, models {}, ambientcg {}",
            tex.len(),
            models.len(),
            acg.len()
        );
        let beach = tex.iter().find(|a| a.id == "aerial_beach_01").unwrap();
        let (d, h) = fetch_texture(beach).unwrap();
        eprintln!("texture {} height {:?}", d.display(), h);
        assert!(h.is_some());
        let rock = acg
            .iter()
            .find(|a| a.categories.iter().any(|c| c.contains("rock")))
            .unwrap_or(&acg[0]);
        let (d2, _) = fetch_texture(rock).unwrap();
        eprintln!("ambientcg {} -> {}", rock.id, d2.display());
        let m = models.iter().find(|a| a.id == "rock_moss_set_01").unwrap();
        let (mesh, col) = fetch_model(m).unwrap();
        eprintln!("model {} triangles, colour {:?}", mesh.tris.len(), col);
        assert!(mesh.tris.len() > 100);
        let img = image::load_from_memory(&get_bytes(&tex[0].thumb).unwrap()).unwrap();
        eprintln!("thumb {}x{}", img.width(), img.height());
        // render: terrain from the height map painted with the colour map, plus the model
        use crate::scene::{Kind, Scene, TexKind};
        let mut s = Scene::empty();
        let t = s.add(Kind::Terrain);
        let mut hm =
            crate::terrain::HeightMap::from_image(&crate::picture::open(h.unwrap()).unwrap(), 256);
        hm.normalize();
        hm.smooth(4);
        hm.normalize();
        s.objects[t].terrain = Some(Arc::new(hm));
        s.objects[t].scale = Vec3::new(24.0, 3.0, 24.0);
        s.objects[t].material.texture = TexKind::Image;
        s.objects[t].material.image = Some(d.to_string_lossy().to_string());
        s.objects[t].material.tex_scale = 0.25;
        let r = s.add(Kind::Mesh);
        s.objects[r].mesh = Some(Arc::new(mesh));
        s.objects[r].pos = Vec3::new(0.0, 6.0, 8.0);
        s.objects[r].scale = Vec3::splat(4.0);
        s.objects[r].material.diffuse = col;
        let c = s.add(Kind::Cube);
        s.objects[c].pos = Vec3::new(-9.0, 5.0, 6.0);
        s.objects[c].material.texture = TexKind::Image;
        s.objects[c].material.image = Some(d2.to_string_lossy().to_string());
        s.camera = crate::scene::Camera {
            pos: Vec3::new(0.0, 16.0, 34.0),
            yaw: 0.0,
            pitch: -20.0,
            fov: 60.0,
        };
        s.render.width = 900;
        s.render.height = 506;
        std::fs::create_dir_all("shots").unwrap();
        crate::render::render_frame(&s)
            .save("shots/library.png")
            .unwrap();
    }
}
