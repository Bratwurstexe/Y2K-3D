//! The main window: menus, palettes, working window, properties, animation and the assistant.

use crate::claude::{self, ChatMsg, Pending};
use crate::icons::{draw_kind_icon, icon_button};
use crate::labs::TerrainEditor;
use crate::mesh::TriMesh;
use crate::render::{self, prepare, prepare_with_ids, RenderJob, View};
use crate::scene::{
    material_presets, sky_presets, Camera, Kind, Material, Object, Scene, Sky, TexKind,
};
use crate::settings::Settings;
use crate::wire;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use glam::Vec3;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Camera,
    Top,
    Front,
    Back,
    Left,
    Right,
}

impl ViewMode {
    const ALL: [ViewMode; 6] = [
        ViewMode::Camera,
        ViewMode::Top,
        ViewMode::Front,
        ViewMode::Back,
        ViewMode::Left,
        ViewMode::Right,
    ];
    fn label(self) -> &'static str {
        match self {
            ViewMode::Camera => crate::i18n::tr("Kamera-Ansicht"),
            ViewMode::Top => crate::i18n::tr("Von oben"),
            ViewMode::Front => crate::i18n::tr("Von vorne"),
            ViewMode::Back => crate::i18n::tr("Von hinten"),
            ViewMode::Left => crate::i18n::tr("Von links"),
            ViewMode::Right => crate::i18n::tr("Von rechts"),
        }
    }
    fn dir(self) -> Vec3 {
        match self {
            ViewMode::Top => Vec3::NEG_Y,
            ViewMode::Front | ViewMode::Camera => Vec3::NEG_Z,
            ViewMode::Back => Vec3::Z,
            ViewMode::Left => Vec3::X,
            ViewMode::Right => Vec3::NEG_X,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    Create,
    Edit,
    Sky,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SideTab {
    Props,
    Claude,
}

struct AnimExport {
    progress: Arc<AtomicU32>,
    total: u32,
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    folder: PathBuf,
}

pub struct Screenshot {
    pub path: PathBuf,
    pub frames: u32,
    pub requested: bool,
    pub saved: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
enum DragKind {
    None,
    MoveObject,
    Orbit,
    Pan,
}

pub struct App {
    pub scene: Scene,
    pub selected: Option<u64>,
    undo: Vec<Scene>,
    redo: Vec<Scene>,
    frame_snapshot: Option<Scene>,
    changed: bool,
    in_edit: bool,
    pub rev: u64,
    file_path: Option<PathBuf>,
    dirty: bool,
    view_mode: ViewMode,
    ortho_center: Vec3,
    ortho_half: f32,
    show_render: bool,
    render_job: Option<RenderJob>,
    render_tex: Option<TextureHandle>,
    render_seen: u64,
    last_render: Option<(usize, usize, Vec<u8>)>,
    nano_tex: Option<TextureHandle>,
    nano_rev: u64,
    nano_pending: Option<(u64, [usize; 2], Arc<Mutex<Option<Vec<u8>>>>)>,
    nano_size: [usize; 2],
    palette: Palette,
    side_tab: SideTab,
    bottom_selection: bool,
    solo: bool,
    pub status: String,
    material_lab: bool,
    sky_lab: bool,
    render_settings: bool,
    help_open: bool,
    about_open: bool,
    pub terrain_editor: Option<TerrainEditor>,
    cam_memory: [Option<Camera>; 6],
    sky_memory: [Option<Sky>; 6],
    mat_thumbs: Vec<(Material, TextureHandle)>,
    sky_thumbs: Vec<(Sky, TextureHandle)>,
    mat_preview: Option<(Material, TextureHandle)>,
    anim_time: f32,
    playing: bool,
    anim_export: Option<AnimExport>,
    chat: Vec<ChatMsg>,
    chat_input: String,
    chat_pending: Option<Pending>,
    login: Arc<Mutex<Option<claude::Login>>>,
    login_checked: bool,
    login_next_check: f64,
    login_announced: bool,
    chat_born: Vec<f64>,
    claude_opened_at: f64,
    claude_phase: f32,
    claude_bottom_h: f32,
    pub settings: Settings,
    drag: DragKind,
    pub screenshot: Option<Screenshot>,
    frame_no: u64,
}

fn color32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> App {
        setup_style(&cc.egui_ctx);
        let settings = Settings::load();
        crate::i18n::set_english(
            settings.language == "en"
                || std::env::var("Y2K3D_LANG")
                    .map(|l| l == "en")
                    .unwrap_or(false),
        );
        App {
            scene: Scene::demo(),
            selected: None,
            undo: Vec::new(),
            redo: Vec::new(),
            frame_snapshot: None,
            changed: false,
            in_edit: false,
            rev: 1,
            file_path: None,
            dirty: false,
            view_mode: ViewMode::Camera,
            ortho_center: Vec3::new(0.0, 0.0, 0.0),
            ortho_half: 40.0,
            show_render: false,
            render_job: None,
            render_tex: None,
            render_seen: 0,
            last_render: None,
            nano_tex: None,
            nano_rev: 0,
            nano_pending: None,
            nano_size: [NANO_W, NANO_H],
            palette: Palette::Create,
            side_tab: SideTab::Props,
            bottom_selection: false,
            solo: false,
            status: crate::i18n::tr(
                "Willkommen! Objekte oben erzeugen, im Arbeitsfenster anklicken und ziehen.",
            )
            .into(),
            material_lab: false,
            sky_lab: false,
            render_settings: false,
            help_open: false,
            about_open: false,
            terrain_editor: None,
            cam_memory: [None; 6],
            sky_memory: Default::default(),
            mat_thumbs: Vec::new(),
            sky_thumbs: Vec::new(),
            mat_preview: None,
            anim_time: 0.0,
            playing: false,
            anim_export: None,
            chat: Vec::new(),
            chat_input: String::new(),
            chat_pending: None,
            login: Arc::new(Mutex::new(None)),
            login_checked: false,
            login_next_check: 0.0,
            login_announced: false,
            chat_born: Vec::new(),
            claude_opened_at: -1.0,
            claude_phase: 0.0,
            claude_bottom_h: 330.0,
            settings,
            drag: DragKind::None,
            screenshot: screenshot.map(|path| Screenshot {
                path,
                frames: 0,
                requested: false,
                saved: Arc::new(AtomicBool::new(false)),
            }),
            frame_no: 0,
        }
    }

    fn set_language(&mut self, english: bool) {
        crate::i18n::set_english(english);
        self.settings.language = if english { "en" } else { "de" }.into();
        self.settings.save();
        self.status = crate::i18n::tr("Sprache").to_string()
            + ": "
            + if english { "English" } else { "Deutsch" };
        // thumbnails and window titles follow on the next frame
        self.claude_opened_at = -1.0;
    }

    /// For checking the editors from the command line (LS3D_OPEN=terrain|material|sky|claude|render).
    pub fn open_for_check(&mut self, what: &str, ctx: &egui::Context) {
        let terrain = self
            .scene
            .objects
            .iter()
            .find(|o| o.kind == Kind::Terrain)
            .map(|o| o.id);
        let ball = self
            .scene
            .objects
            .iter()
            .find(|o| o.kind == Kind::Sphere)
            .map(|o| o.id);
        match what {
            "terrain" => {
                self.selected = terrain;
                self.open_terrain_editor();
            }
            "material" => {
                self.selected = ball;
                self.material_lab = true;
                self.palette = Palette::Edit;
            }
            "sky" => {
                self.sky_lab = true;
                self.palette = Palette::Sky;
                self.selected = ball;
            }
            "claude" => {
                self.side_tab = SideTab::Claude;
                self.chat.push(ChatMsg {
                    user: true,
                    text: crate::i18n::tr("Stell eine goldene Kugel auf den Berg").into(),
                });
                self.chat.push(ChatMsg {
                    user: false,
                    text: if crate::i18n::is_english() {
                        "Sure! I put a golden sphere on the summit and let the evening sun catch it.

✔ “Gold sphere” added"
                            .into()
                    } else {
                        "Gern! Ich habe eine Goldkugel auf den Gipfel gesetzt.

✔ „Goldkugel“ hinzugefügt"
                            .into()
                    },
                });
            }
            "render" => self.start_render(ctx),
            "top" => self.set_view(ViewMode::Top),
            "english" => crate::i18n::set_english(true),
            _ => {}
        }
    }

    // -----------------------------------------------------------------------------------------
    // Change tracking & undo

    pub fn mark(&mut self) {
        self.changed = true;
        self.dirty = true;
        self.rev += 1;
    }

    fn touch_view(&mut self) {
        self.rev += 1;
        self.show_render = false;
    }

    fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.scene, s));
            self.rev += 1;
            self.status = crate::i18n::tr("Rückgängig gemacht").into();
        }
    }

    fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.scene, s));
            self.rev += 1;
            self.status = crate::i18n::tr("Wiederhergestellt").into();
        }
    }

    fn sel_index(&self) -> Option<usize> {
        self.selected.and_then(|id| self.scene.index_of(id))
    }

    pub fn add_object(&mut self, kind: Kind) -> usize {
        let i = self.scene.add(kind);
        // place new objects in front of the camera, on the ground
        if !kind.is_infinite() && kind != Kind::Terrain {
            let c = &self.scene.camera;
            let f = c.forward();
            let fl = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::NEG_Z);
            let p = c.pos + fl * 22.0;
            let o = &mut self.scene.objects[i];
            o.pos.x = p.x;
            o.pos.z = p.z;
        }
        self.selected = Some(self.scene.objects[i].id);
        self.status = crate::trf!("{} erzeugt", self.scene.objects[i].kind.label());
        self.mark();
        i
    }

    fn delete_selected(&mut self) {
        if let Some(i) = self.sel_index() {
            let o = self.scene.objects.remove(i);
            self.status = crate::trf!("„{}“ gelöscht", o.name);
            self.selected = None;
            self.mark();
        }
    }

    fn duplicate_selected(&mut self) {
        if let Some(i) = self.sel_index() {
            let mut o = self.scene.objects[i].clone();
            o.id = self.scene.next_id;
            self.scene.next_id += 1;
            o.name = crate::trf!("{} Kopie", o.name);
            if !o.kind.is_infinite() {
                o.pos += Vec3::new(o.scale.x.max(1.0) * 1.2, 0.0, 0.0);
            }
            self.selected = Some(o.id);
            self.scene.objects.push(o);
            self.status = crate::i18n::tr("Objekt dupliziert").into();
            self.mark();
        }
    }

    fn drop_to_ground(&mut self) {
        let Some(i) = self.sel_index() else { return };
        let o = &self.scene.objects[i];
        if o.kind.is_infinite() || o.kind.is_light() {
            return;
        }
        // find lowest point of the object and what's beneath it
        let segs = wire::world_segments(o, self.scene.camera.pos);
        let low = segs
            .iter()
            .map(|s| s.0.y.min(s.1.y))
            .fold(f32::MAX, f32::min);
        let mut sc = self.scene.clone();
        sc.objects[i].hidden = true;
        let prep = prepare(&sc, View::from_camera(&sc.camera, 1.0), None);
        let start = Vec3::new(o.pos.x, low.max(o.pos.y) + 200.0, o.pos.z);
        let ground = prep
            .intersect(start, Vec3::NEG_Y, 1e5)
            .map(|h| start.y - h.t)
            .unwrap_or(0.0);
        let dy = ground - low;
        self.scene.objects[i].pos.y += dy;
        self.status = crate::i18n::tr("Auf den Boden gesetzt").into();
        self.mark();
    }

    fn randomize_selected(&mut self) {
        let Some(i) = self.sel_index() else { return };
        let mut rng = crate::noise::Rng::new(crate::noise::time_seed());
        let o = &mut self.scene.objects[i];
        if o.kind.is_infinite() {
            return;
        }
        o.rot.y = rng.range(0.0, 360.0);
        let k = rng.range(0.7, 1.4);
        o.scale *= k;
        o.pos.x += rng.range(-4.0, 4.0);
        o.pos.z += rng.range(-4.0, 4.0);
        if o.kind == Kind::Stone {
            o.mesh = Some(Arc::new(TriMesh::stone(rng.next_u32() as u64)));
        }
        self.status = crate::i18n::tr("Zufällig verändert").into();
        self.mark();
    }

    // -----------------------------------------------------------------------------------------
    // Views

    fn current_view(&self, aspect: f32) -> View {
        match self.view_mode {
            ViewMode::Camera => View::from_camera(&self.scene.camera, aspect),
            m => View::ortho(self.ortho_center, m.dir(), self.ortho_half, aspect),
        }
    }

    fn doc_aspect(&self) -> f32 {
        self.scene.render.width as f32 / self.scene.render.height.max(1) as f32
    }

    fn orbit(&mut self, dyaw: f32, dpitch: f32) {
        let pivot = match self.sel_index() {
            Some(i) if !self.scene.objects[i].kind.is_infinite() => self.scene.objects[i].pos,
            _ => {
                let c = &self.scene.camera;
                let f = c.forward();
                let d = if f.y < -0.05 {
                    (c.pos.y / -f.y).min(60.0)
                } else {
                    30.0
                };
                c.pos + f * d
            }
        };
        let cam = &mut self.scene.camera;
        let off = cam.pos - pivot;
        let r = off.length().max(0.5);
        let mut yaw = off.x.atan2(off.z);
        let mut pitch = (off.y / r).clamp(-1.0, 1.0).asin();
        yaw -= dyaw.to_radians();
        pitch = (pitch + dpitch.to_radians()).clamp(-1.45, 1.45);
        cam.pos = pivot
            + Vec3::new(
                pitch.cos() * yaw.sin(),
                pitch.sin(),
                pitch.cos() * yaw.cos(),
            ) * r;
        cam.look_at(pivot);
        self.touch_view();
    }

    fn move_camera(&mut self, right: f32, up: f32, fwd: f32) {
        let cam = &mut self.scene.camera;
        let (f, r, _) = cam.basis();
        let fl = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::NEG_Z);
        cam.pos += r * right + Vec3::Y * up + fl * fwd;
        self.touch_view();
    }

    // -----------------------------------------------------------------------------------------
    // Rendering

    pub fn start_render(&mut self, ctx: &egui::Context) {
        if let Some(j) = &self.render_job {
            j.stop();
        }
        let w = self.scene.render.width as usize;
        let h = self.scene.render.height as usize;
        let view = self.current_view(w as f32 / h as f32);
        let solo = if self.solo { self.selected } else { None };
        let prep = prepare(&self.scene, view, solo);
        let c2 = ctx.clone();
        self.render_job = Some(RenderJob::start(
            prep,
            w,
            h,
            self.scene.render.antialias,
            move || c2.request_repaint(),
        ));
        self.show_render = true;
        self.render_seen = u64::MAX;
        self.status = crate::i18n::tr("Rendern läuft…").into();
    }

    fn stop_render(&mut self) {
        if let Some(j) = &self.render_job {
            j.stop();
            self.status = crate::i18n::tr("Rendern angehalten").into();
        }
    }

    fn poll_render(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.render_job else { return };
        let v = job.version.load(Ordering::Relaxed);
        if v != self.render_seen {
            self.render_seen = v;
            let buf = job.buffer.lock().unwrap().clone();
            let img = egui::ColorImage::from_rgba_unmultiplied([job.width, job.height], &buf);
            match &mut self.render_tex {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => {
                    self.render_tex = Some(ctx.load_texture("render", img, TextureOptions::LINEAR))
                }
            }
            if job.is_done() {
                self.last_render = Some((job.width, job.height, buf));
                if let Some(s) = *job.finished_secs.lock().unwrap() {
                    self.status = crate::trf!("Fertig gerendert in {:.1} s", s);
                }
            }
        }
        if !job.is_done() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }

    fn poll_nano(&mut self, ctx: &egui::Context) {
        if let Some((rev, size, slot)) = &self.nano_pending {
            let done = slot.lock().unwrap().take();
            if let Some(buf) = done {
                let img = egui::ColorImage::from_rgba_unmultiplied(*size, &buf);
                match &mut self.nano_tex {
                    Some(t) => t.set(img, TextureOptions::LINEAR),
                    None => {
                        self.nano_tex = Some(ctx.load_texture("nano", img, TextureOptions::LINEAR))
                    }
                }
                self.nano_rev = *rev;
                self.nano_pending = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
                return;
            }
        }
        if self.nano_rev != self.rev && self.drag_is_idle() {
            let size = self.nano_size;
            let view = self.current_view(size[0] as f32 / size[1] as f32);
            let solo = if self.solo { self.selected } else { None };
            let prep = prepare(&self.scene, view, solo);
            let slot = Arc::new(Mutex::new(None));
            let s2 = slot.clone();
            let c2 = ctx.clone();
            std::thread::spawn(move || {
                let buf = render::render_image(&prep, size[0], size[1]);
                *s2.lock().unwrap() = Some(buf);
                c2.request_repaint();
            });
            self.nano_pending = Some((self.rev, size, slot));
        }
    }

    fn drag_is_idle(&self) -> bool {
        matches!(self.drag, DragKind::None)
    }

    // -----------------------------------------------------------------------------------------
    // Files

    fn lib_dir(&self, sub: &str) -> Option<PathBuf> {
        self.settings
            .library_root()
            .map(|r| r.join(sub))
            .filter(|p| p.is_dir())
    }

    fn new_scene(&mut self) {
        self.undo.push(self.scene.clone());
        self.scene = Scene::empty();
        self.selected = None;
        self.file_path = None;
        self.dirty = false;
        self.rev += 1;
        self.status = crate::i18n::tr("Neue Szene").into();
    }

    fn open_scene(&mut self) {
        let mut d = rfd::FileDialog::new()
            .add_filter(crate::i18n::tr("Szene"), &["l3d"])
            .set_title(crate::i18n::tr("Szene öffnen"));
        if let Some(f) = &self.settings.last_folder {
            d = d.set_directory(f);
        }
        if let Some(p) = d.pick_file() {
            match std::fs::read_to_string(&p)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<Scene>(&s).map_err(|e| e.to_string()))
            {
                Ok(s) => {
                    self.undo.push(std::mem::replace(&mut self.scene, s));
                    self.selected = None;
                    self.file_path = Some(p.clone());
                    self.dirty = false;
                    self.rev += 1;
                    self.status = crate::trf!(
                        "Geöffnet: {}",
                        p.file_name().unwrap_or_default().to_string_lossy()
                    );
                    self.remember_folder(&p);
                }
                Err(e) => self.status = crate::trf!("Konnte die Szene nicht öffnen: {}", e),
            }
        }
    }

    fn remember_folder(&mut self, p: &std::path::Path) {
        if let Some(d) = p.parent() {
            self.settings.last_folder = Some(d.to_string_lossy().to_string());
            self.settings.save();
        }
    }

    fn save_scene(&mut self, ask: bool) {
        let path = if ask || self.file_path.is_none() {
            let mut d = rfd::FileDialog::new()
                .add_filter(crate::i18n::tr("Szene"), &["l3d"])
                .set_file_name("Szene.l3d")
                .set_title(crate::i18n::tr("Szene speichern"));
            if let Some(f) = &self.settings.last_folder {
                d = d.set_directory(f);
            }
            d.save_file()
        } else {
            self.file_path.clone()
        };
        if let Some(p) = path {
            match serde_json::to_string(&self.scene)
                .map_err(|e| e.to_string())
                .and_then(|s| std::fs::write(&p, s).map_err(|e| e.to_string()))
            {
                Ok(()) => {
                    self.file_path = Some(p.clone());
                    self.dirty = false;
                    self.status = crate::trf!(
                        "Gespeichert: {}",
                        p.file_name().unwrap_or_default().to_string_lossy()
                    );
                    self.remember_folder(&p);
                }
                Err(e) => self.status = crate::trf!("Speichern fehlgeschlagen: {}", e),
            }
        }
    }

    fn save_image(&mut self) {
        let Some((w, h, buf)) = &self.last_render else {
            self.status = crate::i18n::tr("Erst rendern, dann speichern.").into();
            return;
        };
        let mut d = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .add_filter("JPEG", &["jpg"])
            .add_filter("BMP", &["bmp"])
            .set_file_name("Bild.png")
            .set_title(crate::i18n::tr("Bild speichern"));
        if let Some(f) = &self.settings.last_folder {
            d = d.set_directory(f);
        }
        if let Some(p) = d.save_file() {
            let img = image::RgbaImage::from_raw(*w as u32, *h as u32, buf.clone()).unwrap();
            let res = if p
                .extension()
                .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
                .unwrap_or(false)
            {
                image::DynamicImage::ImageRgba8(img).to_rgb8().save(&p)
            } else {
                img.save(&p)
            };
            self.status = match res {
                Ok(()) => crate::trf!(
                    "Bild gespeichert: {}",
                    p.file_name().unwrap_or_default().to_string_lossy()
                ),
                Err(e) => crate::trf!("Bild konnte nicht gespeichert werden: {}", e),
            };
            self.remember_folder(&p);
        }
    }

    fn import_model(&mut self) {
        let mut d = rfd::FileDialog::new()
            .add_filter(crate::i18n::tr("3D-Modell"), &["obj", "dxf"])
            .set_title(crate::i18n::tr("3D-Modell importieren"));
        if let Some(dir) = self.lib_dir("OBJ Models") {
            d = d.set_directory(dir);
        }
        if let Some(p) = d.pick_file() {
            let text = match std::fs::read(&p) {
                Ok(b) => String::from_utf8_lossy(&b).to_string(),
                Err(e) => {
                    self.status = crate::trf!("Datei nicht lesbar: {}", e);
                    return;
                }
            };
            let is_dxf = p
                .extension()
                .map(|e| e.eq_ignore_ascii_case("dxf"))
                .unwrap_or(false);
            let mesh = if is_dxf {
                TriMesh::load_dxf(&text)
            } else {
                TriMesh::load_obj(&text)
            };
            if mesh.tris.is_empty() {
                self.status = crate::i18n::tr("In der Datei wurden keine Flächen gefunden.").into();
                return;
            }
            let n = mesh.tris.len();
            let i = self.add_object(Kind::Mesh);
            let o = &mut self.scene.objects[i];
            o.mesh = Some(Arc::new(mesh));
            o.name = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            o.scale = Vec3::splat(3.0);
            o.pos.y = 3.0;
            self.status = crate::trf!("Modell importiert ({} Dreiecke)", n);
            self.remember_folder(&p);
            self.mark();
        }
    }

    fn add_picture_object(&mut self) {
        let mut d = rfd::FileDialog::new()
            .add_filter(
                crate::i18n::tr("Bild"),
                &["tif", "tiff", "png", "jpg", "jpeg", "bmp"],
            )
            .set_title(crate::i18n::tr("Bild für Bildobjekt wählen"));
        if let Some(dir) = self.lib_dir("Picture Objects") {
            d = d.set_directory(dir);
        }
        if let Some(p) = d.pick_file() {
            let ps = p.to_string_lossy().to_string();
            let Some(img) = render::load_picture(&ps) else {
                self.status = crate::i18n::tr("Bild konnte nicht gelesen werden.").into();
                return;
            };
            let (w, h) = img.dimensions();
            let i = self.add_object(Kind::Picture);
            let o = &mut self.scene.objects[i];
            o.material.image = Some(ps);
            o.material.texture = TexKind::Image;
            o.material.name = crate::i18n::tr("Bild").into();
            o.material.specularity = 0.0;
            let a = w as f32 / h.max(1) as f32;
            o.scale = Vec3::new(3.0 * a, 1.0, 3.0);
            o.pos.y = 3.0;
            o.name = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            self.mark();
        }
    }

    // -----------------------------------------------------------------------------------------
    // Animation

    fn start_anim_export(&mut self) {
        if self.scene.anim.keys.len() < 2 {
            self.status =
                crate::i18n::tr("Für eine Animation mindestens zwei Schlüsselbilder setzen.")
                    .into();
            return;
        }
        let Some(folder) = rfd::FileDialog::new()
            .set_title(crate::i18n::tr("Ordner für die Einzelbilder wählen"))
            .pick_folder()
        else {
            return;
        };
        let scene = self.scene.clone();
        let total = ((scene.anim.duration * scene.anim.fps as f32).round() as u32).max(1);
        let progress = Arc::new(AtomicU32::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let (p2, c2, d2, f2) = (
            progress.clone(),
            cancel.clone(),
            done.clone(),
            folder.clone(),
        );
        std::thread::spawn(move || {
            let mut s = scene;
            for f in 0..total {
                if c2.load(Ordering::Relaxed) {
                    break;
                }
                let t = f as f32 / s.anim.fps.max(1) as f32;
                s.apply_time(t);
                let img = render::render_frame(&s);
                let _ = img.save(f2.join(format!("bild_{:04}.png", f + 1)));
                p2.store(f + 1, Ordering::Relaxed);
            }
            d2.store(true, Ordering::Relaxed);
        });
        self.anim_export = Some(AnimExport {
            progress,
            total,
            cancel,
            done,
            folder,
        });
    }

    // -----------------------------------------------------------------------------------------
    // Claude

    fn send_prompt(&mut self) {
        let prompt = self.chat_input.trim().to_string();
        if prompt.is_empty() || self.chat_pending.is_some() {
            return;
        }
        let summary = claude::scene_summary(&self.scene, self.selected);
        let history = self.chat.clone();
        self.chat.push(ChatMsg {
            user: true,
            text: prompt.clone(),
        });
        self.chat_input.clear();
        self.chat_pending = Some(claude::ask(
            self.settings.model.clone(),
            history,
            summary,
            prompt,
        ));
    }

    /// Connects to the Claude sign-in that's already active on this computer, automatically at
    /// start, and keeps looking (every 15 s) while there isn't one yet.
    fn check_login(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let current = self.login.lock().unwrap().clone();
        let ok = matches!(current, Some(claude::Login::Subscription(_)));
        if let (Some(claude::Login::Subscription(plan)), false) = (&current, self.login_announced) {
            self.login_announced = true;
            self.status = crate::trf!("Automatisch mit deinem {} verbunden", plan);
        }
        let due = !self.login_checked || (!ok && current.is_some() && now > self.login_next_check);
        if !due {
            return;
        }
        self.login_checked = true;
        self.login_next_check = now + 15.0;
        let slot = self.login.clone();
        let c2 = ctx.clone();
        std::thread::spawn(move || {
            let st = claude::login_status();
            *slot.lock().unwrap() = Some(st);
            c2.request_repaint();
        });
        ctx.request_repaint_after(std::time::Duration::from_secs(16));
    }

    fn poll_claude(&mut self, ctx: &egui::Context) {
        let Some(p) = &self.chat_pending else { return };
        let res = p.lock().unwrap().take();
        match res {
            None => ctx.request_repaint_after(std::time::Duration::from_millis(150)),
            Some(Err(e)) => {
                self.chat.push(ChatMsg {
                    user: false,
                    text: format!("⚠ {e}"),
                });
                self.chat_pending = None;
            }
            Some(Ok(text)) => {
                self.chat_pending = None;
                let (reply, actions) = claude::parse_answer(&text);
                let mut msg = reply;
                if !actions.is_empty() {
                    let applied = claude::apply_actions(&mut self.scene, &actions, self.selected);
                    if let Some(s) = applied.select {
                        self.selected = Some(s);
                    }
                    if self
                        .selected
                        .and_then(|id| self.scene.index_of(id))
                        .is_none()
                    {
                        self.selected = None;
                    }
                    self.mark();
                    self.show_render = false;
                    if !applied.log.is_empty() {
                        msg.push_str(&format!("\n\n✔ {}", applied.log.join(" · ")));
                    }
                    if applied.render {
                        self.start_render(ctx);
                    }
                }
                self.chat.push(ChatMsg {
                    user: false,
                    text: msg,
                });
            }
        }
    }
}

pub const NANO_W: usize = 176;
pub const NANO_H: usize = 132;

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(38, 38, 42);
    v.window_fill = Color32::from_rgb(44, 44, 49);
    v.extreme_bg_color = Color32::from_rgb(24, 24, 27);
    v.selection.bg_fill = Color32::from_rgb(150, 95, 45);
    v.hyperlink_color = Color32::from_rgb(230, 160, 90);
    v.widgets.hovered.bg_fill = Color32::from_rgb(80, 72, 66);
    v.widgets.active.bg_fill = Color32::from_rgb(150, 95, 45);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(6.0, 5.0);
        s.spacing.button_padding = Vec2::new(7.0, 3.0);
    });
}

// ---------------------------------------------------------------------------------------------
// The frame

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_no += 1;
        if self.frame_no == 3 && std::env::var("LS3D_OPEN").is_err() {
            // open with the demo scene rendered
            self.start_render(&ctx);
            self.status = crate::i18n::tr("Demoszene – klicke ins Bild für das Drahtgitter").into();
        }
        if self.frame_no == 2 {
            if let Ok(w) = std::env::var("LS3D_OPEN") {
                for part in w.split(',') {
                    self.open_for_check(part.trim(), &ctx);
                }
            }
        }
        self.frame_snapshot = Some(self.scene.clone());
        self.changed = false;

        self.poll_render(&ctx);
        self.poll_claude(&ctx);
        self.check_login(&ctx);
        if self.terrain_editor.is_none() {
            self.poll_nano(&ctx);
        }
        self.ensure_thumbs(&ctx);
        self.keyboard(&ctx);
        self.animate(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));

        if self.terrain_editor.is_some() {
            let mut te = self.terrain_editor.take().unwrap();
            let mut close = None;
            egui::CentralPanel::default().show(ui, |ui| {
                close = te.ui(ui, &self.settings);
            });
            match close {
                Some(true) => {
                    if let Some(i) = self.scene.index_of(te.target) {
                        self.scene.objects[i].terrain = Some(Arc::new(te.map.clone()));
                        self.status = crate::i18n::tr("Gelände übernommen").into();
                        self.mark();
                    }
                }
                Some(false) => {
                    self.status = crate::i18n::tr("Gelände-Editor ohne Änderung verlassen").into()
                }
                None => self.terrain_editor = Some(te),
            }
        } else {
            egui::Panel::top("palette")
                .exact_size(66.0)
                .show(ui, |ui| self.palette_bar(ui));
            egui::Panel::bottom("bottom")
                .exact_size(58.0)
                .show(ui, |ui| self.bottom_bar(ui));
            egui::Panel::left("controls")
                .default_size(200.0)
                .size_range(200.0..=900.0)
                .resizable(true)
                .show(ui, |ui| self.control_palette(ui));
            egui::Panel::right("side")
                .default_size(300.0)
                .size_range(220.0..=1100.0)
                .resizable(true)
                .show(ui, |ui| self.side_panel(ui));
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(Color32::from_rgb(30, 30, 33)))
                .show(ui, |ui| self.working_window(ui));
        }

        self.windows(&ctx);

        // undo bookkeeping: one undo step per continuous edit
        if self.changed {
            if !self.in_edit {
                if let Some(s) = self.frame_snapshot.take() {
                    self.undo.push(s);
                    if self.undo.len() > 60 {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                }
            }
            self.in_edit = true;
        } else if !ctx.input(|i| i.pointer.any_down()) {
            self.in_edit = false;
        }

        self.screenshot_step(&ctx);
    }
}

impl App {
    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(s) = &mut self.screenshot else {
            return;
        };
        ctx.request_repaint();
        s.frames += 1;
        if s.saved.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if s.frames >= 120 && !s.requested {
            s.requested = true;
            let path = s.path.clone();
            let saved = s.saved.clone();
            let c2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    buf.extend_from_slice(&p.to_array());
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(im) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                    let _ = im.save(&path);
                }
                saved.store(true, Ordering::Relaxed);
                c2.request_repaint();
            });
        }
        if s.frames > 600 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some()) && ctx.egui_wants_keyboard_input();
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        let pressed = |k: egui::Key| ctx.input(|i| i.key_pressed(k));
        if ctrl && pressed(egui::Key::S) {
            self.save_scene(shift);
        }
        if ctrl && pressed(egui::Key::O) {
            self.open_scene();
        }
        if ctrl && pressed(egui::Key::R) {
            self.start_render(ctx);
        }
        if typing || self.terrain_editor.is_some() {
            return;
        }
        if ctrl && pressed(egui::Key::Z) {
            self.undo();
        }
        if ctrl && pressed(egui::Key::Y) {
            self.redo();
        }
        if ctrl && pressed(egui::Key::D) {
            self.duplicate_selected();
        }
        if pressed(egui::Key::Delete) {
            self.delete_selected();
        }
        if pressed(egui::Key::Escape) {
            if self
                .render_job
                .as_ref()
                .map(|j| !j.is_done())
                .unwrap_or(false)
            {
                self.stop_render();
            } else {
                self.selected = None;
            }
        }
        if pressed(egui::Key::Tab) {
            self.cycle_selection(if shift { -1 } else { 1 });
        }
        // nudge with arrow keys
        if let Some(i) = self.sel_index() {
            let step = if shift { 1.0 } else { 0.25 };
            let mut d = Vec3::ZERO;
            if pressed(egui::Key::ArrowLeft) {
                d.x -= step;
            }
            if pressed(egui::Key::ArrowRight) {
                d.x += step;
            }
            if pressed(egui::Key::ArrowUp) {
                d.z -= step;
            }
            if pressed(egui::Key::ArrowDown) {
                d.z += step;
            }
            if pressed(egui::Key::PageUp) {
                d.y += step;
            }
            if pressed(egui::Key::PageDown) {
                d.y -= step;
            }
            if d != Vec3::ZERO {
                self.scene.objects[i].pos += d;
                self.mark();
            }
        }
    }

    fn cycle_selection(&mut self, dir: i32) {
        let n = self.scene.objects.len();
        if n == 0 {
            return;
        }
        let cur = self.sel_index().map(|i| i as i32).unwrap_or(-1);
        let next = (cur + dir).rem_euclid(n as i32) as usize;
        self.selected = Some(self.scene.objects[next].id);
        self.status = crate::trf!("Ausgewählt: {}", self.scene.objects[next].name);
    }

    fn animate(&mut self, ctx: &egui::Context) {
        if self.playing {
            let dt = ctx.input(|i| i.stable_dt).min(0.1);
            self.anim_time += dt;
            if self.anim_time > self.scene.anim.duration {
                self.anim_time = 0.0;
            }
            self.scene.apply_time(self.anim_time);
            self.touch_view();
            ctx.request_repaint();
        }
        if let Some(e) = &self.anim_export {
            if e.done.load(Ordering::Relaxed) {
                let n = e.progress.load(Ordering::Relaxed);
                self.status =
                    crate::trf!("{} Einzelbilder gespeichert in {}", n, e.folder.display());
                self.anim_export = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Menu

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(crate::i18n::tr("Datei"), |ui| {
                if ui.button(crate::i18n::tr("Neue Szene")).clicked() {
                    self.new_scene();
                    ui.close();
                }
                if ui
                    .button(crate::i18n::tr("Szene öffnen…    Strg+O"))
                    .clicked()
                {
                    self.open_scene();
                    ui.close();
                }
                if ui.button(crate::i18n::tr("Speichern    Strg+S")).clicked() {
                    self.save_scene(false);
                    ui.close();
                }
                if ui.button(crate::i18n::tr("Speichern unter…")).clicked() {
                    self.save_scene(true);
                    ui.close();
                }
                ui.separator();
                if ui
                    .button(crate::i18n::tr("3D-Modell importieren (OBJ/DXF)…"))
                    .clicked()
                {
                    ui.close();
                    self.import_model();
                }
                if ui.button(crate::i18n::tr("Bild speichern…")).clicked() {
                    ui.close();
                    self.save_image();
                }
                if ui
                    .button(crate::i18n::tr("Animation als Einzelbilder rendern…"))
                    .clicked()
                {
                    ui.close();
                    self.start_anim_export();
                }
                ui.separator();
                if ui.button(crate::i18n::tr("Beenden")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(crate::i18n::tr("Bearbeiten"), |ui| {
                if ui
                    .add_enabled(
                        !self.undo.is_empty(),
                        egui::Button::new(crate::i18n::tr("Rückgängig    Strg+Z")),
                    )
                    .clicked()
                {
                    self.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !self.redo.is_empty(),
                        egui::Button::new(crate::i18n::tr("Wiederholen    Strg+Y")),
                    )
                    .clicked()
                {
                    self.redo();
                    ui.close();
                }
                ui.separator();
                let has = self.selected.is_some();
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(crate::i18n::tr("Duplizieren    Strg+D")),
                    )
                    .clicked()
                {
                    self.duplicate_selected();
                    ui.close();
                }
                if ui
                    .add_enabled(has, egui::Button::new(crate::i18n::tr("Löschen    Entf")))
                    .clicked()
                {
                    self.delete_selected();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(crate::i18n::tr("Auf den Boden setzen")),
                    )
                    .clicked()
                {
                    self.drop_to_ground();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(crate::i18n::tr("Zufällig verändern")),
                    )
                    .clicked()
                {
                    self.randomize_selected();
                    ui.close();
                }
                ui.separator();
                if ui.button(crate::i18n::tr("Material-Labor…")).clicked() {
                    self.material_lab = true;
                    ui.close();
                }
            });
            ui.menu_button(crate::i18n::tr("Objekte"), |ui| {
                for k in [
                    Kind::Terrain,
                    Kind::Stone,
                    Kind::Sphere,
                    Kind::Cube,
                    Kind::Cylinder,
                    Kind::Cone,
                    Kind::Torus,
                    Kind::Pyramid,
                    Kind::Disk,
                    Kind::Square,
                    Kind::Ground,
                    Kind::Water,
                    Kind::CloudPlane,
                    Kind::RadialLight,
                    Kind::SpotLight,
                    Kind::ParallelLight,
                ] {
                    if ui.button(crate::trf!("{} erzeugen", k.label())).clicked() {
                        self.add_object(k);
                        ui.close();
                    }
                }
                if ui.button(crate::i18n::tr("Bildobjekt erzeugen…")).clicked() {
                    ui.close();
                    self.add_picture_object();
                }
                ui.separator();
                let terr = self
                    .sel_index()
                    .map(|i| self.scene.objects[i].kind == Kind::Terrain)
                    .unwrap_or(false);
                if ui
                    .add_enabled(
                        terr,
                        egui::Button::new(crate::i18n::tr("Gelände bearbeiten…")),
                    )
                    .clicked()
                {
                    self.open_terrain_editor();
                    ui.close();
                }
            });
            ui.menu_button(crate::i18n::tr("Ansicht"), |ui| {
                for m in ViewMode::ALL {
                    if ui
                        .selectable_label(self.view_mode == m, m.label())
                        .clicked()
                    {
                        self.set_view(m);
                        ui.close();
                    }
                }
                ui.separator();
                if ui
                    .checkbox(&mut self.solo, crate::i18n::tr("Solo-Modus (nur Auswahl)"))
                    .changed()
                {
                    self.rev += 1;
                }
                if ui.button(crate::i18n::tr("Kamera zurücksetzen")).clicked() {
                    self.scene.camera = Camera::default();
                    self.touch_view();
                    ui.close();
                }
            });
            ui.menu_button(crate::i18n::tr("Rendern"), |ui| {
                if ui
                    .button(crate::i18n::tr("Rendern starten    Strg+R"))
                    .clicked()
                {
                    self.start_render(&ctx);
                    ui.close();
                }
                if ui
                    .button(crate::i18n::tr("Rendern anhalten    Esc"))
                    .clicked()
                {
                    self.stop_render();
                    ui.close();
                }
                if ui
                    .button(crate::i18n::tr("Render-Einstellungen…"))
                    .clicked()
                {
                    self.render_settings = true;
                    ui.close();
                }
            });
            ui.menu_button("Claude", |ui| {
                if ui
                    .button(crate::i18n::tr("Claude-Assistent öffnen"))
                    .clicked()
                {
                    self.side_tab = SideTab::Claude;
                    ui.close();
                }
            });
            ui.menu_button(crate::i18n::tr("Hilfe"), |ui| {
                if ui.button(crate::i18n::tr("Bedienung")).clicked() {
                    self.help_open = true;
                    ui.close();
                }
                if ui.button(crate::i18n::tr("Über dieses Programm")).clicked() {
                    self.about_open = true;
                    ui.close();
                }
                ui.separator();
                ui.label(RichText::new("Sprache / Language").small().weak());
                let en = crate::i18n::is_english();
                if ui.selectable_label(!en, "Deutsch").clicked() {
                    self.set_language(false);
                    ui.close();
                }
                if ui.selectable_label(en, "English").clicked() {
                    self.set_language(true);
                    ui.close();
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // quick language switch
                let en = crate::i18n::is_english();
                if ui
                    .selectable_label(en, "EN")
                    .on_hover_text("English")
                    .clicked()
                    && !en
                {
                    self.set_language(true);
                }
                if ui
                    .selectable_label(!en, "DE")
                    .on_hover_text("Deutsch")
                    .clicked()
                    && en
                {
                    self.set_language(false);
                }
                ui.separator();
                let name = self
                    .file_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| crate::i18n::tr("Unbenannte Szene").into());
                ui.label(
                    RichText::new(format!("{}{}", name, if self.dirty { " •" } else { "" })).weak(),
                );
            });
        });
    }

    fn set_view(&mut self, m: ViewMode) {
        if m != ViewMode::Camera && self.view_mode == ViewMode::Camera {
            let c = &self.scene.camera;
            let f = c.forward();
            let d = if f.y < -0.05 {
                (c.pos.y / -f.y).min(60.0)
            } else {
                25.0
            };
            self.ortho_center = c.pos + f * d;
            self.ortho_center.y = self.ortho_center.y.max(0.0);
        }
        self.view_mode = m;
        self.touch_view();
    }

    // -----------------------------------------------------------------------------------------
    // Top palette

    fn palette_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (p, name) in [
                (Palette::Create, crate::i18n::tr("Erstellen")),
                (Palette::Edit, crate::i18n::tr("Bearbeiten")),
                (Palette::Sky, crate::i18n::tr("Himmel & Nebel")),
            ] {
                let txt = RichText::new(name).strong().color(if self.palette == p {
                    Color32::from_rgb(240, 175, 100)
                } else {
                    Color32::from_gray(170)
                });
                if ui.add(egui::Button::new(txt).frame(false)).clicked() {
                    self.palette = p;
                }
            }
        });
        ui.horizontal(|ui| match self.palette {
            Palette::Create => {
                let kinds = [
                    Kind::Water,
                    Kind::CloudPlane,
                    Kind::Ground,
                    Kind::Terrain,
                    Kind::Stone,
                    Kind::Sphere,
                    Kind::Cube,
                    Kind::Cylinder,
                    Kind::Cone,
                    Kind::Torus,
                    Kind::Pyramid,
                    Kind::Disk,
                    Kind::Square,
                ];
                for k in kinds {
                    if icon_button(
                        ui,
                        36.0,
                        false,
                        &crate::trf!("{} erzeugen", k.label()),
                        |p, r| draw_kind_icon(p, r, k),
                    )
                    .clicked()
                    {
                        self.add_object(k);
                    }
                }
                ui.separator();
                if icon_button(
                    ui,
                    36.0,
                    false,
                    crate::i18n::tr("Bildobjekt aus einem Bild erzeugen"),
                    |p, r| draw_kind_icon(p, r, Kind::Picture),
                )
                .clicked()
                {
                    self.add_picture_object();
                }
                if icon_button(
                    ui,
                    36.0,
                    false,
                    crate::i18n::tr("3D-Modell importieren (OBJ / DXF)"),
                    |p, r| draw_kind_icon(p, r, Kind::Mesh),
                )
                .clicked()
                {
                    self.import_model();
                }
                ui.separator();
                for k in [Kind::RadialLight, Kind::SpotLight, Kind::ParallelLight] {
                    if icon_button(
                        ui,
                        36.0,
                        false,
                        &crate::trf!("{} erzeugen", k.label()),
                        |p, r| draw_kind_icon(p, r, k),
                    )
                    .clicked()
                    {
                        self.add_object(k);
                    }
                }
            }
            Palette::Edit => {
                let has = self.selected.is_some();
                ui.add_enabled_ui(has, |ui| {
                    if ui.button(crate::i18n::tr("🎨 Material-Labor")).clicked() {
                        self.material_lab = true;
                    }
                    ui.menu_button(crate::i18n::tr("Material ▾"), |ui| {
                        for m in material_presets() {
                            if ui.button(crate::i18n::tr_name(&m.name)).clicked() {
                                if let Some(i) = self.sel_index() {
                                    self.scene.objects[i].material = m;
                                    self.mark();
                                }
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    if ui
                        .button(crate::i18n::tr("➕ Größer"))
                        .on_hover_text(crate::i18n::tr("Um 10 % vergrößern"))
                        .clicked()
                    {
                        if let Some(i) = self.sel_index() {
                            self.scene.objects[i].scale *= 1.1;
                            self.mark();
                        }
                    }
                    if ui
                        .button(crate::i18n::tr("➖ Kleiner"))
                        .on_hover_text(crate::i18n::tr("Um 10 % verkleinern"))
                        .clicked()
                    {
                        if let Some(i) = self.sel_index() {
                            self.scene.objects[i].scale /= 1.1;
                            self.mark();
                        }
                    }
                    if ui
                        .button(crate::i18n::tr("⟳ Drehen"))
                        .on_hover_text(crate::i18n::tr("Um 15° um die Hochachse drehen"))
                        .clicked()
                    {
                        if let Some(i) = self.sel_index() {
                            self.scene.objects[i].rot.y =
                                (self.scene.objects[i].rot.y + 15.0) % 360.0;
                            self.mark();
                        }
                    }
                    if ui.button(crate::i18n::tr("⬇ Auf Boden")).clicked() {
                        self.drop_to_ground();
                    }
                    if ui
                        .button(crate::i18n::tr("⌖ Zentrieren"))
                        .on_hover_text(crate::i18n::tr("In die Mitte der Szene (X/Z = 0)"))
                        .clicked()
                    {
                        if let Some(i) = self.sel_index() {
                            self.scene.objects[i].pos.x = 0.0;
                            self.scene.objects[i].pos.z = 0.0;
                            self.mark();
                        }
                    }
                    if ui.button(crate::i18n::tr("🎲 Zufall")).clicked() {
                        self.randomize_selected();
                    }
                    if ui.button(crate::i18n::tr("Duplizieren")).clicked() {
                        self.duplicate_selected();
                    }
                    let terr = self
                        .sel_index()
                        .map(|i| self.scene.objects[i].kind == Kind::Terrain)
                        .unwrap_or(false);
                    if ui
                        .add_enabled(terr, egui::Button::new(crate::i18n::tr("Gelände-Editor")))
                        .clicked()
                    {
                        self.open_terrain_editor();
                    }
                });
                if !has {
                    ui.label(RichText::new(crate::i18n::tr("Erst ein Objekt auswählen")).weak());
                }
            }
            Palette::Sky => {
                let thumbs: Vec<(Sky, egui::TextureId)> = self
                    .sky_thumbs
                    .iter()
                    .map(|(s, t)| (s.clone(), t.id()))
                    .collect();
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (s, tid) in thumbs {
                            let sel = self.scene.sky.name == s.name;
                            let resp = ui
                                .add(
                                    egui::Button::image(egui::Image::new((
                                        tid,
                                        Vec2::new(54.0, 34.0),
                                    )))
                                    .selected(sel),
                                )
                                .on_hover_text(crate::i18n::tr_name(&s.name));
                            if resp.clicked() {
                                self.scene.sky = s;
                                self.mark();
                            }
                        }
                        ui.separator();
                        if ui.button(crate::i18n::tr("☀ Himmel-Labor…")).clicked() {
                            self.sky_lab = true;
                        }
                    });
                });
            }
        });
    }

    // -----------------------------------------------------------------------------------------
    // Left control palette

    fn control_palette(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.add_space(4.0);
        ui.label(
            RichText::new(crate::i18n::tr("Nano-Vorschau"))
                .small()
                .weak(),
        );
        // the preview grows with the panel (its right edge can be dragged)
        let w = ui.available_width().floor().clamp(120.0, 900.0);
        let h = (w * 0.75).floor();
        let want = [w as usize, h as usize];
        if want != self.nano_size && !ctx.input(|i| i.pointer.any_down()) {
            self.nano_size = want;
            self.nano_rev = 0;
        }
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        match &self.nano_tex {
            Some(t) => {
                ui.painter().image(
                    t.id(),
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                ui.painter().rect_filled(r, 2.0, Color32::from_gray(20));
            }
        }
        if self.nano_pending.is_some() {
            ui.painter().circle_filled(
                r.right_top() + Vec2::new(-6.0, 6.0),
                3.0,
                Color32::from_rgb(240, 170, 80),
            );
        }
        // camera memory dots
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(crate::i18n::tr("Kamera-Speicher"))
                    .small()
                    .weak(),
            );
            for i in 0..6 {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
                let filled = self.cam_memory[i].is_some();
                ui.painter().circle(
                    r.center(),
                    5.0,
                    if filled {
                        Color32::from_rgb(220, 150, 80)
                    } else {
                        Color32::from_gray(60)
                    },
                    Stroke::new(1.0, Color32::from_gray(120)),
                );
                let resp = resp.on_hover_text(crate::i18n::tr(
                    "Klick: Kamera speichern / abrufen · Rechtsklick: löschen",
                ));
                if resp.clicked() {
                    match self.cam_memory[i] {
                        Some(c) => {
                            self.scene.camera = c;
                            self.touch_view();
                        }
                        None => self.cam_memory[i] = Some(self.scene.camera),
                    }
                }
                if resp.secondary_clicked() {
                    self.cam_memory[i] = None;
                }
            }
        });
        ui.separator();
        egui::ComboBox::from_id_salt("viewmode")
            .selected_text(self.view_mode.label())
            .width(180.0)
            .show_ui(ui, |ui| {
                for m in ViewMode::ALL {
                    if ui
                        .selectable_label(self.view_mode == m, m.label())
                        .clicked()
                    {
                        self.set_view(m);
                    }
                }
            });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            // trackball
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(78.0), Sense::drag());
            let p = ui.painter();
            for i in 0..10 {
                let k = i as f32 / 10.0;
                p.circle_filled(
                    r.center() + Vec2::new(-10.0, -10.0) * k,
                    36.0 * (1.0 - k * 0.8),
                    Color32::from_gray((60.0 + k * 150.0) as u8),
                );
            }
            let resp = resp.on_hover_text(crate::i18n::tr(
                "Kamera-Trackball: ziehen, um die Kamera um die Szene zu drehen",
            ));
            if resp.dragged() {
                let d = resp.drag_delta();
                if self.view_mode == ViewMode::Camera {
                    self.orbit(d.x * 0.6, d.y * 0.4);
                }
            }
            ui.vertical(|ui| {
                // XZ cross
                let (r, resp) = ui.allocate_exact_size(Vec2::new(64.0, 50.0), Sense::drag());
                let p = ui.painter();
                p.rect_filled(r, 6.0, Color32::from_gray(55));
                let c = r.center();
                let col = Color32::from_rgb(220, 160, 90);
                p.line_segment(
                    [c - Vec2::new(22.0, 0.0), c + Vec2::new(22.0, 0.0)],
                    Stroke::new(3.0, col),
                );
                p.line_segment(
                    [c - Vec2::new(0.0, 18.0), c + Vec2::new(0.0, 18.0)],
                    Stroke::new(3.0, col),
                );
                let resp = resp.on_hover_text(crate::i18n::tr(
                    "Kamera-Kreuz: ziehen, um die Kamera seitlich und vor/zurück zu bewegen",
                ));
                if resp.dragged() {
                    let d = resp.drag_delta();
                    if self.view_mode == ViewMode::Camera {
                        self.move_camera(d.x * 0.15, 0.0, -d.y * 0.25);
                    } else {
                        self.ortho_center += Vec3::new(d.x * 0.1, 0.0, d.y * 0.1);
                        self.touch_view();
                    }
                }
                // Y bar
                let (r, resp) = ui.allocate_exact_size(Vec2::new(64.0, 22.0), Sense::drag());
                ui.painter().rect_filled(r, 6.0, Color32::from_gray(55));
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    crate::i18n::tr("↕ Höhe"),
                    egui::FontId::proportional(11.0),
                    Color32::from_gray(200),
                );
                let resp = resp.on_hover_text(crate::i18n::tr("Ziehen: Kamera heben oder senken"));
                if resp.dragged() {
                    self.move_camera(0.0, -resp.drag_delta().y * 0.1, 0.0);
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Brennweite"));
            if ui
                .add(
                    egui::Slider::new(&mut self.scene.camera.fov, 15.0..=120.0)
                        .suffix("°")
                        .show_value(true),
                )
                .changed()
            {
                self.touch_view();
            }
        });
        ui.separator();
        // render controls
        let rendering = self
            .render_job
            .as_ref()
            .map(|j| !j.is_done())
            .unwrap_or(false);
        ui.horizontal(|ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(50.0), Sense::click());
            let p = ui.painter();
            let hot = resp.hovered();
            for i in 0..8 {
                let k = i as f32 / 8.0;
                let base = if rendering {
                    [210.0, 120.0, 60.0]
                } else {
                    [150.0, 150.0, 160.0]
                };
                let f = 0.5 + k * 0.8 + if hot { 0.15 } else { 0.0 };
                p.circle_filled(
                    r.center() + Vec2::new(-6.0, -6.0) * k,
                    23.0 * (1.0 - k * 0.75),
                    Color32::from_rgb(
                        (base[0] * f).min(255.0) as u8,
                        (base[1] * f).min(255.0) as u8,
                        (base[2] * f).min(255.0) as u8,
                    ),
                );
            }
            if resp
                .on_hover_text(crate::i18n::tr("Rendern (Strg+R)"))
                .clicked()
            {
                self.start_render(&ctx);
            }
            ui.vertical(|ui| {
                if ui
                    .add_enabled(rendering, egui::Button::new(crate::i18n::tr("⏹ Anhalten")))
                    .clicked()
                {
                    self.stop_render();
                }
                let can_show = self.render_tex.is_some();
                if ui
                    .add_enabled(
                        can_show,
                        egui::Button::new(if self.show_render {
                            crate::i18n::tr("Drahtgitter")
                        } else {
                            crate::i18n::tr("🖼 Render zeigen")
                        }),
                    )
                    .clicked()
                {
                    self.show_render = !self.show_render;
                }
            });
        });
        if let Some(j) = &self.render_job {
            let pr = j.progress.load(Ordering::Relaxed) as f32 / 1000.0;
            if !j.is_done() {
                ui.add(
                    egui::ProgressBar::new(pr)
                        .desired_width(180.0)
                        .text(format!("{:.0} %", pr * 100.0)),
                );
            }
        }
        if ui.button(crate::i18n::tr("💾 Bild speichern…")).clicked() {
            self.save_image();
        }
        ui.separator();
        ui.label(
            RichText::new(crate::trf!(
                "{} Objekte\n{} Polygone",
                self.scene.objects.len(),
                self.scene.polygon_count()
            ))
            .color(Color32::from_gray(200)),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(&self.status)
                .small()
                .color(Color32::from_rgb(230, 180, 120)),
        );
    }

    // -----------------------------------------------------------------------------------------
    // Bottom: animation / selection

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            if ui
                .button("↔")
                .on_hover_text(crate::i18n::tr(
                    "Zwischen Animation und Auswahl-Palette wechseln",
                ))
                .clicked()
            {
                self.bottom_selection = !self.bottom_selection;
            }
            if self.bottom_selection {
                self.selection_palette(ui);
            } else {
                self.animation_controls(ui);
            }
        });
    }

    fn selection_palette(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(crate::i18n::tr("Auswahl")).strong());
        let kinds: Vec<Kind> = {
            let mut v: Vec<Kind> = Vec::new();
            for o in &self.scene.objects {
                if !v.contains(&o.kind) {
                    v.push(o.kind);
                }
            }
            v
        };
        for k in kinds {
            if icon_button(
                ui,
                30.0,
                self.sel_index()
                    .map(|i| self.scene.objects[i].kind == k)
                    .unwrap_or(false),
                &crate::trf!("Nächstes Objekt der Art „{}“ auswählen", k.label()),
                |p, r| draw_kind_icon(p, r, k),
            )
            .clicked()
            {
                let start = self.sel_index().map(|i| i + 1).unwrap_or(0);
                let n = self.scene.objects.len();
                for j in 0..n {
                    let idx = (start + j) % n;
                    if self.scene.objects[idx].kind == k {
                        self.selected = Some(self.scene.objects[idx].id);
                        break;
                    }
                }
            }
        }
        ui.separator();
        if ui
            .button("◀")
            .on_hover_text(crate::i18n::tr("Vorheriges Objekt"))
            .clicked()
        {
            self.cycle_selection(-1);
        }
        if ui
            .selectable_label(self.solo, crate::i18n::tr("● Solo"))
            .on_hover_text(crate::i18n::tr(
                "Nur das ausgewählte Objekt zeigen und rendern",
            ))
            .clicked()
        {
            self.solo = !self.solo;
            self.rev += 1;
        }
        if ui
            .button("▶")
            .on_hover_text(crate::i18n::tr("Nächstes Objekt"))
            .clicked()
        {
            self.cycle_selection(1);
        }
    }

    fn animation_controls(&mut self, ui: &mut egui::Ui) {
        let dur = self.scene.anim.duration.max(0.1);
        if ui
            .button("⏮")
            .on_hover_text(crate::i18n::tr("Zum Anfang"))
            .clicked()
        {
            self.anim_time = 0.0;
            self.scene.apply_time(0.0);
            self.touch_view();
        }
        if ui
            .button(if self.playing { "⏸" } else { "▶" })
            .on_hover_text(crate::i18n::tr("Animation im Drahtgitter abspielen"))
            .clicked()
        {
            self.playing = !self.playing && self.scene.anim.keys.len() >= 2;
            if !self.playing && self.scene.anim.keys.len() < 2 {
                self.status = crate::i18n::tr("Mindestens zwei Schlüsselbilder setzen.").into();
            }
        }
        if ui
            .button("⏭")
            .on_hover_text(crate::i18n::tr("Zum Ende"))
            .clicked()
        {
            self.anim_time = dur;
            self.scene.apply_time(dur);
            self.touch_view();
        }
        if ui
            .button(crate::i18n::tr("✚ Schlüssel"))
            .on_hover_text(crate::i18n::tr(
                "Schlüsselbild an dieser Zeit setzen (merkt sich Kamera, Objekte und Sonne)",
            ))
            .clicked()
        {
            self.scene.set_key(self.anim_time);
            self.status = crate::trf!("Schlüsselbild bei {:.2} s gesetzt", self.anim_time);
            self.mark();
        }
        if ui
            .button(crate::i18n::tr("✖ Schlüssel"))
            .on_hover_text(crate::i18n::tr("Schlüsselbild an dieser Zeit entfernen"))
            .clicked()
        {
            let t = self.anim_time;
            let before = self.scene.anim.keys.len();
            let tol = dur / 60.0;
            self.scene.anim.keys.retain(|k| (k.time - t).abs() > tol);
            if self.scene.anim.keys.len() != before {
                self.mark();
            }
        }
        // timeline
        let w = (ui.available_width() - 260.0).max(120.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(r, 4.0, Color32::from_gray(26));
        let secs = dur.ceil() as i32;
        for s in 0..=secs {
            let x = r.left() + r.width() * (s as f32 / dur).min(1.0);
            p.line_segment(
                [Pos2::new(x, r.bottom() - 8.0), Pos2::new(x, r.bottom())],
                Stroke::new(1.0, Color32::from_gray(110)),
            );
            p.text(
                Pos2::new(x + 2.0, r.top() + 1.0),
                egui::Align2::LEFT_TOP,
                format!("{s}s"),
                egui::FontId::proportional(9.0),
                Color32::from_gray(120),
            );
        }
        for k in &self.scene.anim.keys {
            let x = r.left() + r.width() * (k.time / dur).clamp(0.0, 1.0);
            let c = Pos2::new(x, r.center().y + 4.0);
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(0.0, -6.0),
                    c + Vec2::new(5.0, 0.0),
                    c + Vec2::new(0.0, 6.0),
                    c + Vec2::new(-5.0, 0.0),
                ],
                Color32::from_rgb(240, 170, 80),
                Stroke::NONE,
            ));
        }
        let x = r.left() + r.width() * (self.anim_time / dur).clamp(0.0, 1.0);
        p.line_segment(
            [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
            Stroke::new(2.0, Color32::from_rgb(255, 90, 60)),
        );
        if resp.dragged() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                self.anim_time = ((pos.x - r.left()) / r.width()).clamp(0.0, 1.0) * dur;
                self.scene.apply_time(self.anim_time);
                self.touch_view();
            }
        }
        ui.label(format!("{:.2} s", self.anim_time));
        ui.add(
            egui::DragValue::new(&mut self.scene.anim.duration)
                .range(0.5..=600.0)
                .speed(0.1)
                .suffix(" s"),
        )
        .on_hover_text(crate::i18n::tr("Länge der Animation"));
        ui.add(
            egui::DragValue::new(&mut self.scene.anim.fps)
                .range(1..=60)
                .suffix(" B/s"),
        )
        .on_hover_text(crate::i18n::tr("Bilder pro Sekunde"));
        if let Some(e) = &self.anim_export {
            let n = e.progress.load(Ordering::Relaxed);
            ui.add(
                egui::ProgressBar::new(n as f32 / e.total as f32)
                    .desired_width(70.0)
                    .text(format!("{n}/{}", e.total)),
            );
            if ui
                .button("✖")
                .on_hover_text(crate::i18n::tr("Animations-Export abbrechen"))
                .clicked()
            {
                e.cancel.store(true, Ordering::Relaxed);
            }
        } else if ui
            .button("🎞")
            .on_hover_text(crate::i18n::tr("Animation als Einzelbilder (PNG) rendern"))
            .clicked()
        {
            self.start_anim_export();
        }
    }

    // -----------------------------------------------------------------------------------------
    // Working window

    fn working_window(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        let aspect = self.doc_aspect();
        let avail = full.shrink(14.0);
        let (w, h) = if avail.width() / avail.height() > aspect {
            (avail.height() * aspect, avail.height())
        } else {
            (avail.width(), avail.width() / aspect)
        };
        let doc = Rect::from_center_size(avail.center(), Vec2::new(w.max(10.0), h.max(10.0)));
        let resp = ui.allocate_rect(full, Sense::click_and_drag());
        let painter = ui.painter_at(full);
        painter.rect_filled(doc, 0.0, Color32::from_rgb(236, 234, 228));
        let view = self.current_view(aspect);

        let showing_render = self.show_render && self.render_tex.is_some();
        if showing_render {
            let t = self.render_tex.as_ref().unwrap();
            painter.image(
                t.id(),
                doc,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            // horizon line and ground grid feel
            if self.view_mode == ViewMode::Camera {
                let far = self.scene.camera.pos
                    + Vec3::new(
                        self.scene.camera.forward().x,
                        0.0,
                        self.scene.camera.forward().z,
                    )
                    .normalize_or(Vec3::NEG_Z)
                        * 5000.0;
                if let Some(hp) = wire::project_point(&view, doc, Vec3::new(far.x, 0.0, far.z)) {
                    if hp.y > doc.top() && hp.y < doc.bottom() {
                        painter.rect_filled(
                            Rect::from_min_max(doc.min, Pos2::new(doc.right(), hp.y)),
                            0.0,
                            Color32::from_rgb(222, 228, 236),
                        );
                        painter.line_segment(
                            [Pos2::new(doc.left(), hp.y), Pos2::new(doc.right(), hp.y)],
                            Stroke::new(1.0, Color32::from_gray(160)),
                        );
                    }
                }
            }
        }
        let doc_painter = painter.with_clip_rect(doc);
        let cam_pos = self.scene.camera.pos;
        let mut picked_light: Option<(u64, f32)> = None;
        let hover = resp.hover_pos();
        for o in &self.scene.objects {
            if o.hidden {
                continue;
            }
            let sel = Some(o.id) == self.selected;
            if showing_render && !sel {
                continue;
            }
            if self.solo && !sel && self.selected.is_some() {
                continue;
            }
            let base = o.kind.wire_color();
            let col = if sel {
                Color32::from_rgb(230, 60, 40)
            } else if o.kind.is_infinite() {
                Color32::from_rgba_unmultiplied(
                    (base[0] as f32 * 0.6) as u8,
                    (base[1] as f32 * 0.6) as u8,
                    (base[2] as f32 * 0.6) as u8,
                    120,
                )
            } else {
                let c = color32(base);
                Color32::from_rgb(c.r() / 2, c.g() / 2, c.b() / 2)
            };
            let width = if sel { 1.4 } else { 0.8 };
            for (a, b) in wire::world_segments(o, cam_pos) {
                if let Some((pa, pb)) = wire::project_segment(&view, doc, a, b) {
                    doc_painter.line_segment([pa, pb], Stroke::new(width, col));
                }
            }
            if o.kind.is_light() {
                if let (Some(pp), Some(h)) = (wire::project_point(&view, doc, o.pos), hover) {
                    let d = pp.distance(h);
                    if d < 14.0 && picked_light.map(|p| d < p.1).unwrap_or(true) {
                        picked_light = Some((o.id, d));
                    }
                }
            }
            if sel && !o.kind.is_infinite() {
                if let Some(pp) = wire::project_point(&view, doc, o.pos) {
                    doc_painter.circle_stroke(
                        pp,
                        4.0,
                        Stroke::new(1.5, Color32::from_rgb(230, 60, 40)),
                    );
                }
            }
        }
        painter.rect_stroke(
            doc,
            0.0,
            Stroke::new(1.0, Color32::from_gray(90)),
            egui::StrokeKind::Outside,
        );
        let caption = if showing_render {
            crate::i18n::tr("Gerendertes Bild – Klick ins Bild kehrt zum Drahtgitter zurück")
        } else {
            self.view_mode.label()
        };
        painter.text(
            doc.left_top() + Vec2::new(4.0, -14.0),
            egui::Align2::LEFT_TOP,
            caption,
            egui::FontId::proportional(11.0),
            Color32::from_gray(150),
        );

        // --- interaction
        let to_s = |p: Pos2| {
            (
                (p.x - doc.center().x) / (doc.width() * 0.5),
                -(p.y - doc.center().y) / (doc.height() * 0.5),
            )
        };
        if resp.clicked() {
            if showing_render {
                self.show_render = false;
            } else if let Some(pos) = resp.interact_pointer_pos() {
                if !doc.contains(pos) {
                    self.selected = None;
                } else if let Some((id, _)) = picked_light {
                    self.selected = Some(id);
                } else {
                    let (sx, sy) = to_s(pos);
                    let (prep, ids) = prepare_with_ids(&self.scene, view, None);
                    let (o, d) = view.ray(sx, sy);
                    self.selected = prep
                        .intersect(o, d, 1e6)
                        .map(|h| self.scene.objects[ids[h.obj]].id);
                }
                if let Some(i) = self.sel_index() {
                    self.status = crate::trf!("Ausgewählt: {}", self.scene.objects[i].name);
                }
            }
        }
        if resp.double_clicked() {
            if let Some(i) = self.sel_index() {
                if self.scene.objects[i].kind == Kind::Terrain {
                    self.open_terrain_editor();
                } else {
                    self.material_lab = true;
                }
            }
        }
        if resp.drag_started() {
            let ptr = ctx_pointer_button(ui);
            self.drag = match ptr {
                Some(egui::PointerButton::Primary) => {
                    let pos = resp.interact_pointer_pos().unwrap_or_default();
                    let on_sel = self.selected.is_some() && {
                        if let Some(i) = self.sel_index() {
                            let o = &self.scene.objects[i];
                            if o.kind.is_light() {
                                wire::project_point(&view, doc, o.pos)
                                    .map(|p| p.distance(pos) < 16.0)
                                    .unwrap_or(false)
                            } else {
                                let (sx, sy) = to_s(pos);
                                let (prep, ids) = prepare_with_ids(&self.scene, view, None);
                                let (ro, rd) = view.ray(sx, sy);
                                prep.intersect(ro, rd, 1e6)
                                    .map(|h| ids[h.obj] == i)
                                    .unwrap_or(false)
                            }
                        } else {
                            false
                        }
                    };
                    if on_sel {
                        DragKind::MoveObject
                    } else if self.view_mode == ViewMode::Camera {
                        DragKind::Orbit
                    } else {
                        DragKind::Pan
                    }
                }
                Some(egui::PointerButton::Secondary) => {
                    if self.view_mode == ViewMode::Camera {
                        DragKind::Orbit
                    } else {
                        DragKind::Pan
                    }
                }
                Some(egui::PointerButton::Middle) => DragKind::Pan,
                _ => DragKind::None,
            };
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            let shift = ui.input(|i| i.modifiers.shift);
            match self.drag {
                DragKind::MoveObject => {
                    if let (Some(i), Some(pos)) = (self.sel_index(), resp.interact_pointer_pos()) {
                        let prev = pos - d;
                        let o = &self.scene.objects[i];
                        let delta = if self.view_mode != ViewMode::Camera {
                            let wpp = self.ortho_half * 2.0 / doc.width();
                            view.r * d.x * wpp - view.u * d.y * wpp
                        } else if shift || o.kind.is_infinite() {
                            let dist = (o.pos - self.scene.camera.pos).length();
                            Vec3::Y * (-d.y * dist * 0.0025)
                        } else {
                            let plane = |p: Pos2| {
                                let (sx, sy) = to_s(p);
                                let (ro, rd) = view.ray(sx, sy);
                                if rd.y.abs() < 1e-4 {
                                    return None;
                                }
                                let t = (o.pos.y - ro.y) / rd.y;
                                (t > 0.0).then(|| ro + rd * t)
                            };
                            match (plane(prev), plane(pos)) {
                                (Some(a), Some(b)) => {
                                    let v = b - a;
                                    if v.length() > 50.0 {
                                        Vec3::ZERO
                                    } else {
                                        v
                                    }
                                }
                                _ => Vec3::ZERO,
                            }
                        };
                        self.scene.objects[i].pos += delta;
                        self.mark();
                        self.show_render = false;
                    }
                }
                DragKind::Orbit => self.orbit(d.x * 0.3, d.y * 0.25),
                DragKind::Pan => {
                    if self.view_mode == ViewMode::Camera {
                        self.move_camera(-d.x * 0.05, d.y * 0.05, 0.0);
                    } else {
                        let wpp = self.ortho_half * 2.0 / doc.width();
                        self.ortho_center += -view.r * d.x * wpp + view.u * d.y * wpp;
                        self.touch_view();
                    }
                }
                DragKind::None => {}
            }
        }
        if resp.drag_stopped() {
            self.drag = DragKind::None;
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                if self.view_mode == ViewMode::Camera {
                    let f = self.scene.camera.forward();
                    self.scene.camera.pos += f * scroll * 0.05;
                    self.touch_view();
                } else {
                    self.ortho_half = (self.ortho_half * (1.0 - scroll * 0.002)).clamp(2.0, 2000.0);
                    self.touch_view();
                }
            }
        }
    }

    fn open_terrain_editor(&mut self) {
        if let Some(i) = self.sel_index() {
            let o = &self.scene.objects[i];
            if let (Kind::Terrain, Some(t)) = (o.kind, &o.terrain) {
                self.terrain_editor = Some(TerrainEditor::new(o.id, (**t).clone()));
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Right side: properties / Claude

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.side_tab,
                SideTab::Props,
                RichText::new(crate::i18n::tr("Eigenschaften")).strong(),
            );
            let connected = matches!(
                *self.login.lock().unwrap(),
                Some(claude::Login::Subscription(_))
            );
            let tab = if connected {
                RichText::new(crate::i18n::tr("✨ Claude ✔")).strong()
            } else {
                RichText::new(crate::i18n::tr("✨ Claude")).strong()
            };
            if ui
                .selectable_value(&mut self.side_tab, SideTab::Claude, tab)
                .on_hover_text(if connected {
                    crate::i18n::tr("Mit deinem Claude-Abo verbunden")
                } else {
                    crate::i18n::tr("Claude-Assistent")
                })
                .clicked()
            {
                self.claude_opened_at = -1.0;
            }
        });
        ui.separator();
        match self.side_tab {
            SideTab::Props => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.properties(ui);
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new(crate::i18n::tr("Alle Objekte"))
                        .default_open(true)
                        .show(ui, |ui| self.object_list(ui));
                });
            }
            SideTab::Claude => self.claude_panel(ui),
        }
    }

    fn object_list(&mut self, ui: &mut egui::Ui) {
        let mut toggle_hide = None;
        for o in &self.scene.objects {
            ui.horizontal(|ui| {
                let eye = if o.hidden { "◌" } else { "👁" };
                if ui
                    .small_button(eye)
                    .on_hover_text(crate::i18n::tr("Ein-/Ausblenden"))
                    .clicked()
                {
                    toggle_hide = Some(o.id);
                }
                let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                draw_kind_icon(ui.painter(), r, o.kind);
                if ui
                    .selectable_label(Some(o.id) == self.selected, &o.name)
                    .clicked()
                {
                    self.selected = Some(o.id);
                }
            });
        }
        if let Some(id) = toggle_hide {
            if let Some(i) = self.scene.index_of(id) {
                self.scene.objects[i].hidden = !self.scene.objects[i].hidden;
                self.mark();
            }
        }
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.sel_index() else {
            ui.label(RichText::new(crate::i18n::tr("Kein Objekt ausgewählt.")).weak());
            ui.label(
                RichText::new(crate::i18n::tr(
                    "Klicke im Arbeitsfenster auf ein Objekt oder wähle es unten aus der Liste.",
                ))
                .small()
                .weak(),
            );
            return;
        };
        let mut changed = false;
        let mut open_lab = false;
        let mut open_terrain = false;
        let mut delete = false;
        let mut duplicate = false;
        let mut ground = false;
        let preview = self.mat_preview_tex(ui.ctx(), i);
        {
            let o = &mut self.scene.objects[i];
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                draw_kind_icon(ui.painter(), r, o.kind);
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut o.name).desired_width(200.0))
                    .changed();
            });
            ui.label(RichText::new(o.kind.label()).small().weak());
            let v3 = |ui: &mut egui::Ui, label: &str, v: &mut Vec3, speed: f32| -> bool {
                let mut c = false;
                ui.horizontal(|ui| {
                    ui.add_sized([62.0, 18.0], egui::Label::new(label));
                    c |= ui
                        .add(
                            egui::DragValue::new(&mut v.x)
                                .speed(speed)
                                .max_decimals(2)
                                .prefix("X "),
                        )
                        .changed();
                    c |= ui
                        .add(
                            egui::DragValue::new(&mut v.y)
                                .speed(speed)
                                .max_decimals(2)
                                .prefix("Y "),
                        )
                        .changed();
                    c |= ui
                        .add(
                            egui::DragValue::new(&mut v.z)
                                .speed(speed)
                                .max_decimals(2)
                                .prefix("Z "),
                        )
                        .changed();
                });
                c
            };
            changed |= v3(ui, crate::i18n::tr("Position"), &mut o.pos, 0.1);
            if !o.kind.is_infinite() {
                changed |= v3(ui, crate::i18n::tr("Drehung °"), &mut o.rot, 1.0);
                if !o.kind.is_light() {
                    changed |= v3(ui, crate::i18n::tr("Größe"), &mut o.scale, 0.05);
                    o.scale = o.scale.max(Vec3::splat(0.01));
                }
            }
            if o.kind.is_light() {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Lichtfarbe"));
                    changed |= ui.color_edit_button_rgb(&mut o.light.color).changed();
                });
                changed |= ui
                    .add(
                        egui::Slider::new(&mut o.light.intensity, 0.0..=10.0)
                            .text(crate::i18n::tr("Stärke")),
                    )
                    .changed();
                if o.kind == Kind::SpotLight {
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut o.light.cone, 2.0..=85.0)
                                .text(crate::i18n::tr("Kegel °")),
                        )
                        .changed();
                }
                if o.kind != Kind::ParallelLight {
                    changed |= ui
                        .checkbox(
                            &mut o.light.falloff,
                            crate::i18n::tr("Nimmt mit der Entfernung ab"),
                        )
                        .changed();
                }
            } else {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if let Some(t) = &preview {
                        ui.image((t.id(), Vec2::splat(64.0)));
                    }
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(crate::trf!(
                                "Material: {}",
                                crate::i18n::tr_name(&o.material.name)
                            ))
                            .strong(),
                        );
                        if ui.button(crate::i18n::tr("🎨 Material-Labor…")).clicked() {
                            open_lab = true;
                        }
                        egui::ComboBox::from_id_salt("matpreset")
                            .selected_text(crate::i18n::tr("Vorlage wählen"))
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for m in material_presets() {
                                    if ui
                                        .selectable_label(
                                            o.material.name == m.name,
                                            crate::i18n::tr_name(&m.name),
                                        )
                                        .clicked()
                                    {
                                        o.material = m;
                                        changed = true;
                                    }
                                }
                            });
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Farbe"));
                    if ui.color_edit_button_rgb(&mut o.material.diffuse).changed() {
                        let d = o.material.diffuse;
                        o.material.ambient = [d[0] * 0.4, d[1] * 0.4, d[2] * 0.45];
                        changed = true;
                    }
                    ui.label(crate::i18n::tr("2. Farbe"));
                    changed |= ui.color_edit_button_rgb(&mut o.material.color2).changed();
                });
            }
            if o.kind == Kind::Terrain {
                if ui
                    .button(crate::i18n::tr("Gelände-Editor öffnen…"))
                    .clicked()
                {
                    open_terrain = true;
                }
            }
            if let Some(m) = &o.mesh {
                ui.label(
                    RichText::new(crate::trf!("{} Dreiecke", m.tris.len()))
                        .small()
                        .weak(),
                );
            }
            changed |= ui
                .checkbox(&mut o.hidden, crate::i18n::tr("Ausgeblendet"))
                .changed();
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(crate::i18n::tr("Duplizieren")).clicked() {
                duplicate = true;
            }
            if ui.button(crate::i18n::tr("⬇ Auf Boden")).clicked() {
                ground = true;
            }
            if ui.button(crate::i18n::tr("🗑 Löschen")).clicked() {
                delete = true;
            }
        });
        if changed {
            self.mark();
            self.show_render = false;
        }
        if open_lab {
            self.material_lab = true;
        }
        if open_terrain {
            self.open_terrain_editor();
        }
        if duplicate {
            self.duplicate_selected();
        }
        if ground {
            self.drop_to_ground();
        }
        if delete {
            self.delete_selected();
        }
    }

    fn claude_panel(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);
        while self.chat_born.len() < self.chat.len() {
            self.chat_born.push(now);
        }
        self.chat_born.truncate(self.chat.len());
        if self.claude_opened_at < 0.0 {
            self.claude_opened_at = now;
        }
        let busy = self.chat_pending.is_some();
        let login = self.login.lock().unwrap().clone();
        let ok = matches!(login, Some(claude::Login::Subscription(_)));
        let typing_secs = |t: &str| t.chars().count() as f64 / 110.0 + 0.4;
        let mut animating = busy || now - self.claude_opened_at < 1.6;
        if let (Some(m), Some(b)) = (self.chat.last(), self.chat_born.last()) {
            if now - b < typing_secs(&m.text) {
                animating = true;
            }
        }

        // --- animated header
        let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
        {
            let p = ui.painter_at(hr);
            let n = 48;
            let phase = if animating {
                now as f32 * 1.4
            } else {
                self.claude_phase
            };
            if animating {
                self.claude_phase = phase;
            }
            let mut mesh = egui::Mesh::default();
            for i in 0..=n {
                let f = i as f32 / n as f32;
                let w = ((f * 5.0 - phase).sin() * 0.5 + 0.5) * 0.65;
                let a = [217.0, 119.0, 87.0];
                let b = [120.0, 96.0, 210.0];
                let c = Color32::from_rgb(
                    (a[0] + (b[0] - a[0]) * w) as u8,
                    (a[1] + (b[1] - a[1]) * w) as u8,
                    (a[2] + (b[2] - a[2]) * w) as u8,
                );
                let x = hr.left() + hr.width() * f;
                let base = mesh.vertices.len() as u32;
                mesh.colored_vertex(Pos2::new(x, hr.top()), c);
                mesh.colored_vertex(Pos2::new(x, hr.bottom()), c.gamma_multiply(0.75));
                if i > 0 {
                    mesh.add_triangle(base - 2, base - 1, base);
                    mesh.add_triangle(base - 1, base, base + 1);
                }
            }
            p.add(egui::Shape::mesh(mesh));
            // soft shine sweeping across
            let sx = hr.left() + ((phase * 0.35).rem_euclid(1.6) - 0.3) * hr.width();
            for k in 0..10 {
                let a = (10 - k) as u8 * 3;
                p.rect_filled(
                    Rect::from_center_size(
                        Pos2::new(sx, hr.center().y),
                        Vec2::new(8.0 + k as f32 * 9.0, hr.height()),
                    ),
                    0.0,
                    Color32::from_white_alpha(a),
                );
            }
            p.text(
                hr.left_center() + Vec2::new(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                crate::i18n::tr("✨ Claude"),
                egui::FontId::proportional(17.0),
                Color32::WHITE,
            );
            // status chip
            let (label, dot) = match &login {
                Some(claude::Login::Subscription(_)) => (
                    crate::i18n::tr("verbunden"),
                    Color32::from_rgb(120, 230, 120),
                ),
                None => (
                    crate::i18n::tr("verbinde…"),
                    Color32::from_rgb(255, 210, 120),
                ),
                _ => (
                    crate::i18n::tr("nicht verbunden"),
                    Color32::from_rgb(255, 140, 110),
                ),
            };
            let chip = Rect::from_min_size(
                Pos2::new(hr.right() - 118.0, hr.center().y - 11.0),
                Vec2::new(110.0, 22.0),
            );
            p.rect_filled(chip, 11.0, Color32::from_black_alpha(70));
            let pulse = ((now * 2.4).sin() as f32 * 0.5 + 0.5) * if ok { 1.0 } else { 0.4 };
            let dc = chip.left_center() + Vec2::new(12.0, 0.0);
            p.circle_filled(dc, 4.0 + pulse * 3.5, dot.gamma_multiply(0.25));
            p.circle_filled(dc, 4.0, dot);
            p.text(
                dc + Vec2::new(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
            if ok {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
        ui.add_space(4.0);
        ui.label(RichText::new(crate::i18n::tr("Beschreibe in eigenen Worten, was in der Szene passieren soll – Claude baut es für dich.")).small().weak());
        ui.add_space(4.0);

        // --- conversation
        let hist_h = (ui.available_height() - self.claude_bottom_h - 10.0).max(60.0);
        let mut pick_example = None;
        egui::ScrollArea::vertical()
            .max_height(hist_h)
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.chat.is_empty() {
                    let a = ease(((now - self.claude_opened_at) / 0.6).clamp(0.0, 1.0) as f32);
                    ui.add_space((1.0 - a) * 14.0);
                    ui.scope(|ui| {
                        ui.multiply_opacity(a);
                        ui.label(RichText::new(crate::i18n::tr("Probier zum Beispiel:")).weak());
                        for (i, ex) in [
                            crate::i18n::tr("Mach eine Insel im Meer bei Sonnenuntergang"),
                            crate::i18n::tr("Stell eine goldene Kugel auf den Berg"),
                            crate::i18n::tr("Morgennebel über dem See, Kamera etwas tiefer"),
                            crate::i18n::tr("Eine Säulenreihe aus Marmor-Zylindern am Ufer"),
                        ]
                        .iter()
                        .enumerate()
                        {
                            let ai = ease(
                                (((now - self.claude_opened_at) - 0.12 * i as f64) / 0.5)
                                    .clamp(0.0, 1.0) as f32,
                            );
                            ui.scope(|ui| {
                                ui.multiply_opacity(ai);
                                let r = egui::Frame::new()
                                    .fill(Color32::from_rgb(46, 46, 54))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(90, 80, 120)))
                                    .corner_radius(14.0)
                                    .inner_margin(egui::Margin::symmetric(10, 5))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(*ex)
                                                .color(Color32::from_rgb(230, 200, 170)),
                                        )
                                    })
                                    .response
                                    .interact(Sense::click());
                                if r.hovered() {
                                    ui.painter().rect_stroke(
                                        r.rect,
                                        14.0,
                                        Stroke::new(1.5, Color32::from_rgb(217, 119, 87)),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if r.clicked() {
                                    pick_example = Some(ex.to_string());
                                }
                            });
                            ui.add_space(2.0);
                        }
                    });
                }
                let last = self.chat.len().saturating_sub(1);
                for (i, m) in self.chat.iter().enumerate() {
                    let born = self.chat_born.get(i).copied().unwrap_or(0.0);
                    let a = ease(((now - born) / 0.35).clamp(0.0, 1.0) as f32);
                    let full_w = ui.available_width();
                    ui.add_space((1.0 - a) * 10.0);
                    ui.scope(|ui| {
                        ui.multiply_opacity(a);
                        ui.horizontal(|ui| {
                            let bw = full_w * 0.86;
                            if m.user {
                                ui.add_space(full_w - bw - 4.0 + (1.0 - a) * 20.0);
                            } else {
                                ui.add_space((1.0 - a) * 20.0);
                            }
                            let (bg, stroke) = if m.user {
                                (Color32::from_rgb(112, 72, 44), Stroke::NONE)
                            } else {
                                (
                                    Color32::from_rgb(42, 45, 56),
                                    Stroke::new(1.0, Color32::from_rgb(70, 66, 100)),
                                )
                            };
                            egui::Frame::new()
                                .fill(bg)
                                .stroke(stroke)
                                .corner_radius(12.0)
                                .inner_margin(egui::Margin::symmetric(10, 7))
                                .show(ui, |ui| {
                                    ui.set_width(bw - 20.0);
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(if m.user {
                                                crate::i18n::tr("Du")
                                            } else {
                                                crate::i18n::tr("✨ Claude")
                                            })
                                            .small()
                                            .strong()
                                            .color(
                                                if m.user {
                                                    Color32::from_rgb(255, 215, 180)
                                                } else {
                                                    Color32::from_rgb(200, 180, 255)
                                                },
                                            ),
                                        );
                                        // typewriter for the newest answer
                                        if !m.user && i == last && now - born < typing_secs(&m.text)
                                        {
                                            let n = (((now - born) * 110.0) as usize)
                                                .min(m.text.chars().count());
                                            let shown: String = m.text.chars().take(n).collect();
                                            ui.label(format!("{shown}▌"));
                                        } else {
                                            ui.label(&m.text);
                                        }
                                    });
                                });
                        });
                    });
                    ui.add_space(5.0);
                }
                if busy {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(46.0, 22.0), Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(r, 11.0, Color32::from_rgb(42, 45, 56));
                        for k in 0..3 {
                            let t = (now * 6.0 - k as f64 * 0.7).sin().max(0.0) as f32;
                            let c = Pos2::new(
                                r.left() + 12.0 + k as f32 * 11.0,
                                r.center().y - t * 4.0,
                            );
                            let col = Color32::from_rgb(217, 119, 87)
                                .lerp_to_gamma(Color32::from_rgb(170, 150, 255), t);
                            p.circle_filled(c, 3.2, col);
                        }
                        let shimmer = ((now * 3.0).sin() * 0.5 + 0.5) as f32;
                        ui.label(
                            RichText::new(crate::i18n::tr("Claude denkt nach…"))
                                .italics()
                                .color(Color32::from_gray((150.0 + shimmer * 90.0) as u8)),
                        );
                    });
                }
            });
        if let Some(ex) = pick_example {
            self.chat_input = ex;
        }

        // --- input
        let bottom_start = ui.cursor().top();
        ui.add_space(4.0);
        let focus_glow = ui.memory(|m| m.focused().is_some());
        let frame_stroke = if focus_glow {
            Stroke::new(1.5, Color32::from_rgb(217, 119, 87))
        } else {
            Stroke::new(1.0, Color32::from_gray(70))
        };
        let mut send_key = false;
        egui::Frame::new().fill(Color32::from_rgb(28, 28, 33)).stroke(frame_stroke).corner_radius(12.0).inner_margin(8.0).show(ui, |ui| {
            // Enter sends, Shift+Enter starts a new line: take the Enter key away from the text
            // field before it can insert a line break.
            let input_id = egui::Id::unique("claude_input");
            if ui.memory(|m| m.has_focus(input_id)) {
                let enter = ui.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                });
                if enter {
                    send_key = true;
                }
            }
            ui.add(
                egui::TextEdit::multiline(&mut self.chat_input)
                    .id(input_id)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .frame(egui::Frame::NONE)
                    .hint_text(crate::i18n::tr("Was soll Claude in der Szene machen? (Enter sendet, Umschalt+Enter = neue Zeile)")),
            );
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new(crate::i18n::tr("Verlauf leeren")).small()).frame(false)).clicked() {
                    self.chat.clear();
                    self.chat_born.clear();
                    self.claude_opened_at = now;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can = !busy && !self.chat_input.trim().is_empty();
                    let glow = if can { ((now * 3.0).sin() * 0.5 + 0.5) as f32 } else { 0.0 };
                    let fill = if can {
                        Color32::from_rgb(217, 119, 87).lerp_to_gamma(Color32::from_rgb(235, 150, 110), glow)
                    } else {
                        Color32::from_gray(60)
                    };
                    let b = egui::Button::new(RichText::new(if busy { "…" } else { crate::i18n::tr("Senden ▶") }).strong().color(Color32::WHITE)).fill(fill).corner_radius(14.0);
                    let r = ui.add_enabled(can, b).on_hover_text(crate::i18n::tr("Enter"));
                    if can {
                        ui.painter().rect_stroke(r.rect.expand(1.0 + glow * 2.0), 16.0, Stroke::new(1.0, Color32::from_rgb(217, 119, 87).gamma_multiply(0.5 * (1.0 - glow))), egui::StrokeKind::Outside);
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(40));
                    }
                    if r.clicked() {
                        self.send_prompt();
                    }
                });
            });
        });
        if send_key && !busy {
            self.send_prompt();
        }
        if animating {
            ui.ctx().request_repaint();
        }
        ui.add_space(6.0);
        egui::CollapsingHeader::new(crate::i18n::tr("Claude-Konto")).default_open(!ok).show(ui, |ui| {
            ui.label(
                RichText::new(crate::i18n::tr("🔒 Y2K-3D speichert keine Anmeldedaten. Es nutzt die Claude-Anmeldung, die auf diesem Computer schon vorhanden ist (von Claude Code verwaltet). Abmelden geht in Claude Code selbst."))
                    .small()
                    .color(Color32::from_rgb(170, 190, 210)),
            );
            match &login {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(crate::i18n::tr("Anmeldung wird geprüft…"));
                    });
                }
                Some(claude::Login::Subscription(plan)) => {
                    ui.label(RichText::new(crate::trf!("✔ Verbunden mit deinem {}", plan)).color(Color32::from_rgb(140, 210, 120)));
                    ui.label(RichText::new(crate::i18n::tr("Es wird dein eigenes Claude-Abo genutzt – kein API-Schlüssel nötig.")).small().weak());
                }
                Some(claude::Login::Other(m)) => {
                    ui.label(RichText::new(crate::trf!("Claude Code ist angemeldet, aber nicht mit einem Claude-Abo ({}).", m)).color(Color32::from_rgb(240, 180, 90)));
                    ui.label(RichText::new(crate::i18n::tr("Melde dich mit deinem Claude-Konto an, damit dein Abo genutzt wird.")).small());
                }
                Some(claude::Login::LoggedOut) => {
                    ui.label(RichText::new(crate::i18n::tr("Noch nicht angemeldet.")).color(Color32::from_rgb(240, 180, 90)));
                    ui.label(RichText::new(crate::i18n::tr("Klicke auf „Mit Claude-Konto anmelden“: Es öffnet sich ein Fenster und danach der Browser, in dem du dich mit deinem Claude-Abo (Pro oder Max) anmeldest.")).small());
                }
                Some(claude::Login::NotInstalled) => {
                    ui.label(RichText::new(crate::i18n::tr("Claude Code ist auf diesem Computer nicht installiert.")).color(Color32::from_rgb(240, 180, 90)));
                    ui.label(RichText::new(crate::i18n::tr("Das Programm nutzt dein Claude-Abo über Claude Code. Installiere es einmal von claude.com/claude-code und melde dich dort an.")).small());
                    ui.hyperlink_to(crate::i18n::tr("claude.com/claude-code öffnen"), "https://claude.com/claude-code");
                }
            }
            ui.horizontal(|ui| {
                let installed = !matches!(login, Some(claude::Login::NotInstalled) | None);
                if !ok && ui.add_enabled(installed, egui::Button::new(crate::i18n::tr("Mit Claude-Konto anmelden"))).clicked() {
                    if let Err(e) = claude::open_login() {
                        self.status = crate::trf!("Anmeldung ließ sich nicht öffnen: {}", e);
                    }
                }
                if ui.button(crate::i18n::tr("Erneut prüfen")).clicked() {
                    *self.login.lock().unwrap() = None;
                    self.login_checked = false;
                    self.login_announced = false;
                }
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Modell"));
                let cur = crate::settings::MODELS.iter().find(|m| m.0 == self.settings.model).map(|m| crate::i18n::tr(m.1)).unwrap_or(self.settings.model.as_str()).to_string();
                egui::ComboBox::from_id_salt("model").selected_text(cur).show_ui(ui, |ui| {
                    for (id, label) in crate::settings::MODELS {
                        if ui.selectable_label(self.settings.model == id, crate::i18n::tr(label)).clicked() {
                            self.settings.model = id.to_string();
                            self.settings.save();
                        }
                    }
                });
            });
        });
        // remember how tall everything below the chat is, so the chat leaves room for it
        self.claude_bottom_h = (ui.cursor().top() - bottom_start).max(120.0);
    }

    // -----------------------------------------------------------------------------------------
    // Thumbnails & previews

    fn ensure_thumbs(&mut self, ctx: &egui::Context) {
        if self.sky_thumbs.is_empty() {
            for s in sky_presets() {
                let (w, h) = (54usize, 34usize);
                let sun = s.sun_dir();
                let mut buf = Vec::with_capacity(w * h * 4);
                for y in 0..h {
                    for x in 0..w {
                        let sx = x as f32 / w as f32 * 2.0 - 1.0;
                        let sy = 0.55 - y as f32 / h as f32 * 1.1;
                        let az = (s.sun_azimuth).to_radians();
                        let fwd = Vec3::new(az.sin(), 0.0, -az.cos());
                        let right = fwd.cross(Vec3::Y);
                        let d = (fwd + right * sx * 0.9 + Vec3::Y * sy).normalize();
                        let c = if d.y < -0.08 {
                            let g = Vec3::new(0.32, 0.3, 0.24) * (0.5 + sun.y.max(0.0))
                                + Vec3::from(s.ambient) * 0.15;
                            g.lerp(Vec3::from(s.haze_color), s.haze * 0.6)
                        } else {
                            render::sky_color(&s, sun, d)
                        };
                        buf.extend_from_slice(&render::to_rgba(c));
                    }
                }
                let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &buf);
                let t = ctx.load_texture(format!("sky-{}", s.name), img, TextureOptions::LINEAR);
                self.sky_thumbs.push((s, t));
            }
        }
        if self.mat_thumbs.is_empty() {
            for m in material_presets() {
                let buf = material_ball(&m, 48);
                let img = egui::ColorImage::from_rgba_unmultiplied([48, 48], &buf);
                let t = ctx.load_texture(format!("mat-{}", m.name), img, TextureOptions::LINEAR);
                self.mat_thumbs.push((m, t));
            }
        }
    }

    fn mat_preview_tex(&mut self, ctx: &egui::Context, i: usize) -> Option<TextureHandle> {
        let m = &self.scene.objects[i].material;
        if let Some((pm, t)) = &self.mat_preview {
            if pm == m {
                return Some(t.clone());
            }
        }
        let buf = material_ball(m, 96);
        let img = egui::ColorImage::from_rgba_unmultiplied([96, 96], &buf);
        let t = ctx.load_texture("matprev", img, TextureOptions::LINEAR);
        self.mat_preview = Some((m.clone(), t.clone()));
        Some(t)
    }

    pub fn mat_thumbs(&self) -> &[(Material, TextureHandle)] {
        &self.mat_thumbs
    }

    // -----------------------------------------------------------------------------------------
    // Floating windows

    fn windows(&mut self, ctx: &egui::Context) {
        if self.material_lab {
            let mut open = true;
            let thumbs: Vec<(Material, egui::TextureId)> = self
                .mat_thumbs()
                .iter()
                .map(|(m, t)| (m.clone(), t.id()))
                .collect();
            let sel = self.sel_index();
            let prev = sel.and_then(|i| self.mat_preview_tex(ctx, i));
            let root = self.settings.library_root();
            let mut changed = false;
            egui::Window::new(crate::i18n::tr("🎨 Material-Labor"))
                .open(&mut open)
                .default_width(520.0)
                .show(ctx, |ui| match sel {
                    None => {
                        ui.label(crate::i18n::tr("Bitte zuerst ein Objekt auswählen."));
                    }
                    Some(i) => {
                        let o = &mut self.scene.objects[i];
                        changed = crate::labs::material_lab(
                            ui,
                            &mut o.material,
                            prev.as_ref(),
                            &thumbs,
                            root,
                        );
                    }
                });
            if changed {
                self.mark();
                self.show_render = false;
            }
            self.material_lab = open;
        }
        if self.sky_lab {
            let mut open = true;
            let mut changed = false;
            let mut mem_action = None;
            let mem: Vec<bool> = self.sky_memory.iter().map(|m| m.is_some()).collect();
            egui::Window::new(crate::i18n::tr("☀ Himmel & Nebel"))
                .open(&mut open)
                .default_width(380.0)
                .show(ctx, |ui| {
                    let (c, m) = crate::labs::sky_lab(ui, &mut self.scene.sky, &mem);
                    changed = c;
                    mem_action = m;
                });
            if let Some((slot, store)) = mem_action {
                if store {
                    self.sky_memory[slot] = Some(self.scene.sky.clone());
                } else if let Some(s) = self.sky_memory[slot].clone() {
                    self.scene.sky = s;
                    changed = true;
                }
            }
            if changed {
                self.mark();
                self.show_render = false;
            }
            self.sky_lab = open;
        }
        if self.render_settings {
            let mut open = true;
            let mut changed = false;
            egui::Window::new(crate::i18n::tr("Render-Einstellungen"))
                .open(&mut open)
                .show(ctx, |ui| {
                    let r = &mut self.scene.render;
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Bildgröße"));
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut r.width)
                                    .range(64..=4096)
                                    .suffix(crate::i18n::tr(" px")),
                            )
                            .changed();
                        ui.label("×");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut r.height)
                                    .range(64..=4096)
                                    .suffix(crate::i18n::tr(" px")),
                            )
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        for (w, h) in [
                            (640, 480),
                            (800, 600),
                            (1024, 768),
                            (1280, 720),
                            (1920, 1080),
                        ] {
                            if ui.small_button(format!("{w}×{h}")).clicked() {
                                r.width = w;
                                r.height = h;
                                changed = true;
                            }
                        }
                    });
                    changed |= ui
                        .checkbox(
                            &mut r.antialias,
                            crate::i18n::tr("Kantenglättung (feiner, etwas langsamer)"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut r.max_depth, 1..=8)
                                .text(crate::i18n::tr("Spiegelungstiefe")),
                        )
                        .changed();
                });
            if changed {
                self.mark();
            }
            self.render_settings = open;
        }
        if self.help_open {
            egui::Window::new(crate::i18n::tr("Bedienung")).open(&mut self.help_open).default_width(420.0).show(ctx, |ui| {
                ui.label(RichText::new(crate::i18n::tr("Arbeitsfenster")).strong());
                ui.label(crate::i18n::tr("• Linksklick: Objekt auswählen  • Ausgewähltes Objekt ziehen: verschieben (mit Umschalt: nach oben/unten)"));
                ui.label(crate::i18n::tr("• Ziehen auf freier Fläche oder mit der rechten Maustaste: Kamera um die Szene drehen"));
                ui.label(crate::i18n::tr("• Mittlere Maustaste: Kamera verschieben  • Mausrad: vor/zurück"));
                ui.label(crate::i18n::tr("• Doppelklick: Material-Labor bzw. Gelände-Editor öffnen"));
                ui.add_space(4.0);
                ui.label(RichText::new(crate::i18n::tr("Tastatur")).strong());
                ui.label(crate::i18n::tr("Entf löschen · Strg+D duplizieren · Strg+Z/Y rückgängig/wiederholen · Pfeiltasten/Bild↑↓ verschieben · Tab nächstes Objekt · Strg+R rendern · Esc anhalten"));
                ui.add_space(4.0);
                ui.label(RichText::new(crate::i18n::tr("Animation")).strong());
                ui.label(crate::i18n::tr("Zeitleiste unten anklicken, Szene einrichten, „◆ +“ drücken. Für weitere Zeitpunkte wiederholen, dann ▶ abspielen oder 🎞 als Einzelbilder rendern."));
                ui.add_space(4.0);
                ui.label(RichText::new("Claude").strong());
                ui.label(crate::i18n::tr("Rechts auf „✨ Claude“ klicken und Wünsche einfach hinschreiben. Genutzt wird dein eigenes Claude-Abo über das installierte Claude Code (einmal anmelden)."));
            });
        }
        if self.about_open {
            egui::Window::new(crate::i18n::tr("Über")).open(&mut self.about_open).show(ctx, |ui| {
                ui.heading("Y2K-3D");
                ui.label(crate::i18n::tr("Neubau eines klassischen 3D-Landschaftsprogramms für Windows 11."));
                ui.label(crate::i18n::tr("Portabel: läuft ohne Installation, alle Einstellungen liegen im Ordner „Daten“ neben dem Programm."));
                if let Some(r) = self.settings.library_root() {
                    ui.label(crate::trf!("Bibliotheken (Geländekarten, Bildtexturen, Modelle): {}", r.display()));
                }
            });
        }
    }
}

fn ctx_pointer_button(ui: &egui::Ui) -> Option<egui::PointerButton> {
    ui.input(|i| {
        if i.pointer.primary_down() {
            Some(egui::PointerButton::Primary)
        } else if i.pointer.secondary_down() {
            Some(egui::PointerButton::Secondary)
        } else if i.pointer.middle_down() {
            Some(egui::PointerButton::Middle)
        } else {
            None
        }
    })
}

/// A material sphere on a checker floor.
pub fn material_ball(m: &Material, size: usize) -> Vec<u8> {
    let mut s = Scene::empty();
    s.objects.clear();
    let g = s.add(Kind::Ground);
    s.objects[g].pos.y = -1.0;
    s.objects[g].material = Material {
        texture: TexKind::Checker,
        diffuse: [0.8, 0.8, 0.8],
        color2: [0.35, 0.35, 0.38],
        tex_scale: 5.0,
        specularity: 0.0,
        ..Default::default()
    };
    let b = s.add(Kind::Sphere);
    let o: &mut Object = &mut s.objects[b];
    o.pos = Vec3::ZERO;
    o.scale = Vec3::ONE;
    o.material = m.clone();
    s.sky.cloud_cover = 0.3;
    s.sky.haze = 0.0;
    s.sky.sun_azimuth = 150.0;
    s.sky.sun_elevation = 40.0;
    s.render.max_depth = 3;
    s.camera = Camera {
        pos: Vec3::new(0.0, 0.9, 3.4),
        yaw: 0.0,
        pitch: -14.0,
        fov: 50.0,
    };
    let prep = prepare(&s, View::from_camera(&s.camera, 1.0), None);
    render::render_image(&prep, size, size)
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
