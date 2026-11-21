//! Editors: Material lab, Sky & Fog lab and the Terrain editor.

use crate::noise::time_seed;
use crate::scene::{sky_presets, Material, Sky, TexKind};
use crate::settings::{library_files, Settings};
use crate::terrain::{eval_curve, FractalKind, HeightMap};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use std::path::PathBuf;

fn slider(
    ui: &mut egui::Ui,
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
) -> bool {
    ui.add(egui::Slider::new(v, range).text(label)).changed()
}

fn color_row(ui: &mut egui::Ui, label: &str, c: &mut [f32; 3]) -> bool {
    let mut ch = false;
    ui.horizontal(|ui| {
        ui.add_sized([110.0, 18.0], egui::Label::new(label));
        ch = ui.color_edit_button_rgb(c).changed();
    });
    ch
}

pub fn material_lab(
    ui: &mut egui::Ui,
    m: &mut Material,
    preview: Option<&TextureHandle>,
    thumbs: &[(Material, egui::TextureId)],
    root: Option<PathBuf>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            if let Some(t) = preview {
                ui.image((t.id(), Vec2::splat(140.0)));
            }
            ui.add(egui::TextEdit::singleline(&mut m.name).desired_width(140.0));
        });
        ui.vertical(|ui| {
            ui.label(RichText::new(crate::i18n::tr("Vorlagen")).strong());
            egui::ScrollArea::vertical()
                .max_height(170.0)
                .id_salt("matgrid")
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (pm, tid) in thumbs {
                            let r = ui
                                .add(
                                    egui::Button::image(egui::Image::new((
                                        *tid,
                                        Vec2::splat(44.0),
                                    )))
                                    .selected(pm.name == m.name),
                                )
                                .on_hover_text(crate::i18n::tr_name(&pm.name));
                            if r.clicked() {
                                let keep_img = m.image.clone();
                                *m = pm.clone();
                                if m.image.is_none() {
                                    m.image = keep_img;
                                }
                                changed = true;
                            }
                        }
                    });
                });
        });
    });
    ui.separator();
    egui::Grid::new("matgrid2")
        .num_columns(2)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(crate::i18n::tr("Farbe")).strong());
                changed |= color_row(ui, crate::i18n::tr("Grundfarbe"), &mut m.diffuse);
                changed |= color_row(ui, crate::i18n::tr("Zweite Farbe"), &mut m.color2);
                changed |= color_row(ui, crate::i18n::tr("Umgebungsfarbe"), &mut m.ambient);
                changed |= color_row(ui, crate::i18n::tr("Glanzfarbe"), &mut m.specular);
                ui.add_space(4.0);
                ui.label(RichText::new(crate::i18n::tr("Textur")).strong());
                egui::ComboBox::from_id_salt("tex")
                    .selected_text(m.texture.label())
                    .show_ui(ui, |ui| {
                        for t in TexKind::ALL {
                            changed |= ui.selectable_value(&mut m.texture, t, t.label()).changed();
                        }
                    });
                changed |= slider(
                    ui,
                    &mut m.tex_scale,
                    0.05..=10.0,
                    crate::i18n::tr("Maßstab"),
                );
                if m.texture == TexKind::Image {
                    let name = m
                        .image
                        .as_deref()
                        .map(|p| {
                            std::path::Path::new(p)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        })
                        .unwrap_or_else(|| crate::i18n::tr("kein Bild").into());
                    ui.label(RichText::new(name).small());
                    if ui.button(crate::i18n::tr("Bild wählen…")).clicked() {
                        let mut d = rfd::FileDialog::new().add_filter(
                            crate::i18n::tr("Bild"),
                            &["tif", "tiff", "png", "jpg", "jpeg", "bmp"],
                        );
                        if let Some(r) = &root {
                            let p = r.join("Picture Textures");
                            if p.is_dir() {
                                d = d.set_directory(p);
                            }
                        }
                        if let Some(p) = d.pick_file() {
                            m.image = Some(p.to_string_lossy().to_string());
                            changed = true;
                        }
                    }
                    let files = library_files(
                        root.clone(),
                        "Picture Textures",
                        &["tif", "tiff", "png", "jpg"],
                    );
                    if !files.is_empty() {
                        egui::ComboBox::from_id_salt("pictex")
                            .selected_text(crate::i18n::tr("Bildtextur aus Bibliothek"))
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                for f in files {
                                    let n = f
                                        .file_stem()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .to_string();
                                    if ui.selectable_label(false, n).clicked() {
                                        m.image = Some(f.to_string_lossy().to_string());
                                        changed = true;
                                    }
                                }
                            });
                    }
                }
            });
            ui.vertical(|ui| {
                ui.label(RichText::new(crate::i18n::tr("Werte")).strong());
                changed |= slider(ui, &mut m.diffusion, 0.0..=1.5, crate::i18n::tr("Streuung"));
                changed |= slider(ui, &mut m.ambience, 0.0..=1.5, crate::i18n::tr("Umgebung"));
                changed |= slider(ui, &mut m.specularity, 0.0..=1.0, crate::i18n::tr("Glanz"));
                changed |= slider(
                    ui,
                    &mut m.metallicity,
                    0.0..=1.0,
                    crate::i18n::tr("Metallisch"),
                );
                changed |= slider(ui, &mut m.bump, 0.0..=2.0, crate::i18n::tr("Relief"));
                ui.add_space(4.0);
                ui.label(RichText::new(crate::i18n::tr("Optik")).strong());
                changed |= slider(
                    ui,
                    &mut m.transparency,
                    0.0..=1.0,
                    crate::i18n::tr("Transparenz"),
                );
                changed |= slider(
                    ui,
                    &mut m.reflection,
                    0.0..=1.0,
                    crate::i18n::tr("Spiegelung"),
                );
                changed |= slider(
                    ui,
                    &mut m.refraction,
                    1.0..=2.5,
                    crate::i18n::tr("Brechung"),
                );
            });
            ui.end_row();
        });
    changed
}

/// Returns (changed, memory action (slot, store?)).
pub fn sky_lab(ui: &mut egui::Ui, s: &mut Sky, memory: &[bool]) -> (bool, Option<(usize, bool)>) {
    let mut changed = false;
    let mut mem = None;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("skypreset")
            .selected_text(crate::i18n::tr_name(&s.name))
            .width(170.0)
            .show_ui(ui, |ui| {
                for p in sky_presets() {
                    if ui
                        .selectable_label(p.name == s.name, crate::i18n::tr_name(&p.name))
                        .clicked()
                    {
                        *s = p;
                        changed = true;
                    }
                }
            });
        ui.label(RichText::new(crate::i18n::tr("Speicher")).small().weak());
        for (i, &filled) in memory.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
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
                "Klick: abrufen · Rechtsklick: aktuellen Himmel speichern",
            ));
            if resp.clicked() && filled {
                mem = Some((i, false));
            }
            if resp.secondary_clicked() || (resp.clicked() && !filled) {
                mem = Some((i, true));
            }
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        // sun direction ball
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(110.0), Sense::click_and_drag());
        let p = ui.painter();
        p.circle_filled(r.center(), 52.0, Color32::from_rgb(40, 60, 95));
        p.circle_stroke(r.center(), 52.0, Stroke::new(1.0, Color32::from_gray(120)));
        p.circle_stroke(r.center(), 26.0, Stroke::new(0.5, Color32::from_gray(90)));
        p.text(
            r.center_top() + Vec2::new(0.0, 2.0),
            egui::Align2::CENTER_TOP,
            "N",
            egui::FontId::proportional(10.0),
            Color32::from_gray(180),
        );
        let az = s.sun_azimuth.to_radians();
        let rad = 52.0 * (1.0 - s.sun_elevation.max(-10.0) / 90.0).clamp(0.0, 1.1);
        let sp = r.center() + Vec2::new(az.sin(), -az.cos()) * rad;
        p.circle_filled(
            sp,
            7.0,
            if s.moon {
                Color32::from_rgb(220, 220, 200)
            } else {
                Color32::from_rgb(255, 210, 90)
            },
        );
        if let Some(pos) = resp.interact_pointer_pos() {
            if resp.dragged() || resp.clicked() {
                let d = pos - r.center();
                s.sun_azimuth = d.x.atan2(-d.y).to_degrees().rem_euclid(360.0);
                s.sun_elevation = (90.0 * (1.0 - d.length() / 52.0)).clamp(-10.0, 90.0);
                changed = true;
            }
        }
        resp.on_hover_text(crate::i18n::tr(
            "Sonne/Mond: in die Kugel klicken oder ziehen (Mitte = Mittag, Rand = Horizont)",
        ));
        ui.vertical(|ui| {
            changed |= slider(
                ui,
                &mut s.sun_azimuth,
                0.0..=360.0,
                crate::i18n::tr("Richtung °"),
            );
            changed |= slider(
                ui,
                &mut s.sun_elevation,
                -10.0..=90.0,
                crate::i18n::tr("Höhe °"),
            );
            changed |= slider(
                ui,
                &mut s.sun_intensity,
                0.0..=3.0,
                crate::i18n::tr("Sonnenstärke"),
            );
            changed |= slider(ui, &mut s.shadows, 0.0..=1.0, crate::i18n::tr("Schatten"));
            ui.horizontal(|ui| {
                changed |= ui
                    .checkbox(&mut s.sun_visible, crate::i18n::tr("Sonne sichtbar"))
                    .changed();
                changed |= ui.checkbox(&mut s.moon, crate::i18n::tr("Mond")).changed();
                changed |= ui
                    .checkbox(&mut s.stars, crate::i18n::tr("Sterne"))
                    .changed();
            });
        });
    });
    ui.separator();
    egui::CollapsingHeader::new(crate::i18n::tr("Farben"))
        .default_open(true)
        .show(ui, |ui| {
            changed |= color_row(ui, crate::i18n::tr("Himmel oben"), &mut s.zenith);
            changed |= color_row(ui, crate::i18n::tr("Horizont"), &mut s.horizon);
            changed |= color_row(ui, crate::i18n::tr("Sonnenlicht"), &mut s.sun_color);
            changed |= color_row(ui, crate::i18n::tr("Umgebungslicht"), &mut s.ambient);
        });
    egui::CollapsingHeader::new(crate::i18n::tr("Wolken"))
        .default_open(true)
        .show(ui, |ui| {
            changed |= slider(
                ui,
                &mut s.cloud_cover,
                0.0..=1.0,
                crate::i18n::tr("Bedeckung"),
            );
            changed |= slider(
                ui,
                &mut s.cloud_frequency,
                0.1..=4.0,
                crate::i18n::tr("Häufigkeit"),
            );
            changed |= slider(
                ui,
                &mut s.cloud_amplitude,
                0.05..=1.0,
                crate::i18n::tr("Weichheit"),
            );
            changed |= slider(
                ui,
                &mut s.cloud_height,
                10.0..=200.0,
                crate::i18n::tr("Höhe"),
            );
            changed |= color_row(ui, crate::i18n::tr("Wolkenfarbe"), &mut s.cloud_color);
        });
    egui::CollapsingHeader::new(crate::i18n::tr("Dunst & Nebel"))
        .default_open(true)
        .show(ui, |ui| {
            changed |= slider(ui, &mut s.haze, 0.0..=1.0, crate::i18n::tr("Dunst"));
            changed |= color_row(ui, crate::i18n::tr("Dunstfarbe"), &mut s.haze_color);
            changed |= slider(ui, &mut s.fog, 0.0..=1.0, crate::i18n::tr("Nebel"));
            changed |= slider(
                ui,
                &mut s.fog_height,
                0.2..=30.0,
                crate::i18n::tr("Nebelhöhe"),
            );
            changed |= color_row(ui, crate::i18n::tr("Nebelfarbe"), &mut s.fog_color);
        });
    if changed {
        if !sky_presets().iter().any(|p| p == s) && !s.name.ends_with('*') {
            s.name.push('*');
        }
    }
    (changed, mem)
}

// ---------------------------------------------------------------------------------------------
// Terrain editor

#[derive(PartialEq, Clone, Copy)]
enum TTab {
    Elevation,
    Filter,
    Picture,
}

pub struct TerrainEditor {
    pub target: u64,
    pub map: HeightMap,
    history: Vec<HeightMap>,
    tab: TTab,
    kind: FractalKind,
    brush_mode: usize,
    brush_size: f32,
    brush_strength: f32,
    flatten_level: f32,
    curve: Vec<[f32; 2]>,
    drag_point: Option<usize>,
    yaw: f32,
    pitch: f32,
    tex: Option<TextureHandle>,
    tex_dirty: bool,
    pic: Option<(String, HeightMap)>,
    pic_mode: usize,
    pic_amount: f32,
    pic_tex: Option<TextureHandle>,
    status: String,
    brushing: bool,
}

impl TerrainEditor {
    pub fn new(target: u64, map: HeightMap) -> TerrainEditor {
        TerrainEditor {
            target,
            map,
            history: Vec::new(),
            tab: TTab::Elevation,
            kind: FractalKind::Mountains,
            brush_mode: 0,
            brush_size: 12.0,
            brush_strength: 0.6,
            flatten_level: 0.3,
            curve: vec![[0.0, 0.0], [1.0, 1.0]],
            drag_point: None,
            yaw: 35.0,
            pitch: 35.0,
            tex: None,
            tex_dirty: true,
            pic: None,
            pic_mode: 0,
            pic_amount: 1.0,
            pic_tex: None,
            status: String::new(),
            brushing: false,
        }
    }

    fn save_history(&mut self) {
        self.history.push(self.map.clone());
        if self.history.len() > 30 {
            self.history.remove(0);
        }
        self.tex_dirty = true;
    }

    fn op(&mut self, f: impl FnOnce(&mut HeightMap)) {
        self.save_history();
        f(&mut self.map);
    }

    /// Returns Some(true) = apply, Some(false) = cancel, None = keep editing.
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &Settings) -> Option<bool> {
        let mut result = None;
        let ctx = ui.ctx().clone();
        if self.tex_dirty || self.tex.is_none() {
            let img = map_image(&self.map);
            match &mut self.tex {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => {
                    self.tex = Some(ctx.load_texture("terrain-map", img, TextureOptions::LINEAR))
                }
            }
            self.tex_dirty = false;
        }
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new(crate::i18n::tr("Gelände-Editor"))
                    .color(Color32::from_rgb(240, 175, 100)),
            );
            ui.add_space(20.0);
            for (t, n) in [
                (TTab::Elevation, crate::i18n::tr("Höhe")),
                (TTab::Filter, crate::i18n::tr("Filter")),
                (TTab::Picture, crate::i18n::tr("Bild")),
            ] {
                ui.selectable_value(&mut self.tab, t, RichText::new(n).strong());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new(crate::i18n::tr("✔ Übernehmen")).strong())
                            .fill(Color32::from_rgb(110, 75, 40)),
                    )
                    .clicked()
                {
                    result = Some(true);
                }
                if ui.button(crate::i18n::tr("✖ Abbrechen")).clicked() {
                    result = Some(false);
                }
                if ui
                    .add_enabled(
                        !self.history.is_empty(),
                        egui::Button::new(crate::i18n::tr("Rückgängig")),
                    )
                    .clicked()
                {
                    if let Some(m) = self.history.pop() {
                        self.map = m;
                        self.tex_dirty = true;
                    }
                }
            });
        });
        ui.separator();
        let avail = ui.available_size();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(250.0, avail.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(250.0);
                    egui::ScrollArea::vertical()
                        .id_salt("tools")
                        .show(ui, |ui| match self.tab {
                            TTab::Elevation => self.elevation_tools(ui),
                            TTab::Filter => self.filter_tools(ui),
                            TTab::Picture => self.picture_tools(ui, settings),
                        });
                },
            );
            ui.separator();
            let side = (avail.y - 30.0).min((avail.x - 290.0) * 0.5).max(150.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(crate::i18n::tr("Höhenkarte – mit der Maus malen"))
                        .small()
                        .weak(),
                );
                self.map_view(ui, side);
            });
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(crate::i18n::tr("3D-Vorschau – ziehen zum Drehen"))
                        .small()
                        .weak(),
                );
                self.preview_3d(ui, side);
            });
        });
        if !self.status.is_empty() {
            ui.label(
                RichText::new(&self.status)
                    .small()
                    .color(Color32::from_rgb(230, 180, 120)),
            );
        }
        result
    }

    fn elevation_tools(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(crate::i18n::tr("Neu erzeugen")).strong());
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("fk")
                .selected_text(self.kind.label())
                .show_ui(ui, |ui| {
                    for k in FractalKind::ALL {
                        ui.selectable_value(&mut self.kind, k, k.label());
                    }
                });
            if ui.button(crate::i18n::tr("Erzeugen")).clicked() {
                let (k, n) = (self.kind, self.map.size);
                self.op(|m| *m = HeightMap::fractal(n, k, time_seed()));
            }
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Auflösung"));
            let cur = self.map.size;
            egui::ComboBox::from_id_salt("res")
                .selected_text(format!("{cur}×{cur}"))
                .show_ui(ui, |ui| {
                    for n in [64usize, 128, 256, 512] {
                        if ui.selectable_label(cur == n, format!("{n}×{n}")).clicked() && n != cur
                        {
                            self.op(|m| *m = m.resample(n));
                        }
                    }
                });
        });
        ui.separator();
        ui.label(RichText::new(crate::i18n::tr("Formen")).strong());
        egui::Grid::new("ops").num_columns(2).show(ui, |ui| {
            let b = |ui: &mut egui::Ui, label: &str, tip: &str| {
                ui.add_sized([112.0, 22.0], egui::Button::new(label))
                    .on_hover_text(tip)
                    .clicked()
            };
            if b(
                ui,
                crate::i18n::tr("Erodieren"),
                crate::i18n::tr("Wasser trägt Material ab und bildet Rinnen"),
            ) {
                self.op(|m| m.erode(3, time_seed()));
            }
            if b(
                ui,
                crate::i18n::tr("Glätten"),
                crate::i18n::tr("Kanten weicher machen"),
            ) {
                self.op(|m| m.smooth(2));
            }
            ui.end_row();
            if b(
                ui,
                crate::i18n::tr("Schärfen"),
                crate::i18n::tr("Details betonen"),
            ) {
                self.op(|m| m.sharpen());
            }
            if b(
                ui,
                crate::i18n::tr("Rauschen"),
                crate::i18n::tr("Feine Unebenheiten hinzufügen"),
            ) {
                self.op(|m| m.add_noise(0.04, time_seed()));
            }
            ui.end_row();
            if b(
                ui,
                crate::i18n::tr("Anheben"),
                crate::i18n::tr("Alles etwas höher"),
            ) {
                self.op(|m| m.raise(0.05));
            }
            if b(
                ui,
                crate::i18n::tr("Absenken"),
                crate::i18n::tr("Alles etwas tiefer"),
            ) {
                self.op(|m| m.raise(-0.05));
            }
            ui.end_row();
            if b(
                ui,
                crate::i18n::tr("Verstärken"),
                crate::i18n::tr("Höhenunterschiede vergrößern"),
            ) {
                self.op(|m| m.amplify(1.2));
            }
            if b(
                ui,
                crate::i18n::tr("Abschwächen"),
                crate::i18n::tr("Höhenunterschiede verkleinern"),
            ) {
                self.op(|m| m.amplify(0.83));
            }
            ui.end_row();
            if b(
                ui,
                crate::i18n::tr("Spitzen"),
                crate::i18n::tr("Zufällige Spitzen aufsetzen"),
            ) {
                self.op(|m| m.spikes(time_seed()));
            }
            if b(
                ui,
                crate::i18n::tr("Terrassen"),
                crate::i18n::tr("Stufen wie bei einem Tafelberg"),
            ) {
                self.op(|m| m.terraces(6));
            }
            ui.end_row();
            if b(
                ui,
                crate::i18n::tr("Umkehren"),
                crate::i18n::tr("Berge werden Täler"),
            ) {
                self.op(|m| m.invert());
            }
            if b(
                ui,
                crate::i18n::tr("Insel-Rand"),
                crate::i18n::tr("Ränder zum Rand hin absenken"),
            ) {
                self.op(|m| m.round_edges());
            }
            ui.end_row();
            if b(
                ui,
                crate::i18n::tr("Normalisieren"),
                crate::i18n::tr("Volle Höhe ausnutzen"),
            ) {
                self.op(|m| m.normalize());
            }
            if b(
                ui,
                crate::i18n::tr("Alles flach"),
                crate::i18n::tr("Neu beginnen mit einer Ebene"),
            ) {
                self.op(|m| *m = HeightMap::flat(m.size));
            }
            ui.end_row();
        });
        ui.separator();
        ui.label(RichText::new(crate::i18n::tr("Pinsel")).strong());
        ui.horizontal_wrapped(|ui| {
            for (i, n) in [
                crate::i18n::tr("Anheben"),
                crate::i18n::tr("Absenken"),
                crate::i18n::tr("Glätten"),
                crate::i18n::tr("Ebnen"),
            ]
            .iter()
            .enumerate()
            {
                ui.selectable_value(&mut self.brush_mode, i, *n);
            }
        });
        ui.add(egui::Slider::new(&mut self.brush_size, 2.0..=80.0).text(crate::i18n::tr("Größe")));
        ui.add(
            egui::Slider::new(&mut self.brush_strength, 0.05..=1.0).text(crate::i18n::tr("Stärke")),
        );
        if self.brush_mode == 3 {
            ui.add(
                egui::Slider::new(&mut self.flatten_level, 0.0..=1.0).text(crate::i18n::tr("Höhe")),
            );
        }
    }

    fn filter_tools(&mut self, ui: &mut egui::Ui) {
        ui.label(crate::i18n::tr("Die Kurve ordnet jeder Höhe (waagrecht) eine neue Höhe (senkrecht) zu. Punkte ziehen, Doppelklick fügt einen Punkt hinzu, Rechtsklick entfernt ihn."));
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(220.0), Sense::click_and_drag());
        let p = ui.painter_at(r);
        p.rect_filled(r, 2.0, Color32::from_gray(22));
        for k in 1..4 {
            let f = k as f32 / 4.0;
            p.line_segment(
                [
                    Pos2::new(r.left() + r.width() * f, r.top()),
                    Pos2::new(r.left() + r.width() * f, r.bottom()),
                ],
                Stroke::new(0.5, Color32::from_gray(50)),
            );
            p.line_segment(
                [
                    Pos2::new(r.left(), r.top() + r.height() * f),
                    Pos2::new(r.right(), r.top() + r.height() * f),
                ],
                Stroke::new(0.5, Color32::from_gray(50)),
            );
        }
        let to =
            |c: [f32; 2]| Pos2::new(r.left() + c[0] * r.width(), r.bottom() - c[1] * r.height());
        let from = |p: Pos2| {
            [
                ((p.x - r.left()) / r.width()).clamp(0.0, 1.0),
                ((r.bottom() - p.y) / r.height()).clamp(0.0, 1.0),
            ]
        };
        let pts: Vec<Pos2> = (0..=64)
            .map(|i| {
                let x = i as f32 / 64.0;
                to([x, eval_curve(&self.curve, x)])
            })
            .collect();
        p.add(egui::Shape::line(
            pts,
            Stroke::new(2.0, Color32::from_rgb(240, 170, 80)),
        ));
        for c in &self.curve {
            p.circle_filled(to(*c), 4.0, Color32::WHITE);
        }
        if let Some(pos) = resp.interact_pointer_pos() {
            let nearest = self
                .curve
                .iter()
                .enumerate()
                .map(|(i, c)| (i, to(*c).distance(pos)))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            if resp.drag_started() {
                self.drag_point = nearest.filter(|n| n.1 < 12.0).map(|n| n.0);
            }
            if resp.dragged() {
                if let Some(i) = self.drag_point {
                    let mut c = from(pos);
                    let n = self.curve.len();
                    if i == 0 {
                        c[0] = 0.0;
                    } else if i == n - 1 {
                        c[0] = 1.0;
                    } else {
                        c[0] = c[0].clamp(self.curve[i - 1][0] + 0.01, self.curve[i + 1][0] - 0.01);
                    }
                    self.curve[i] = c;
                }
            }
            if resp.double_clicked() {
                let c = from(pos);
                let idx = self
                    .curve
                    .iter()
                    .position(|q| q[0] > c[0])
                    .unwrap_or(self.curve.len() - 1)
                    .max(1);
                self.curve.insert(idx, c);
            }
            if resp.secondary_clicked() {
                if let Some((i, d)) = nearest {
                    if d < 12.0 && i > 0 && i < self.curve.len() - 1 {
                        self.curve.remove(i);
                    }
                }
            }
        }
        if resp.drag_stopped() {
            self.drag_point = None;
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button(crate::i18n::tr("Linear")).clicked() {
                self.curve = vec![[0.0, 0.0], [1.0, 1.0]];
            }
            if ui.button(crate::i18n::tr("Plateau")).clicked() {
                self.curve = vec![[0.0, 0.0], [0.5, 0.75], [1.0, 0.8]];
            }
            if ui.button(crate::i18n::tr("Täler")).clicked() {
                self.curve = vec![[0.0, 0.0], [0.5, 0.2], [1.0, 1.0]];
            }
            if ui.button(crate::i18n::tr("Stufen")).clicked() {
                self.curve = vec![
                    [0.0, 0.0],
                    [0.3, 0.3],
                    [0.35, 0.5],
                    [0.65, 0.5],
                    [0.7, 0.8],
                    [1.0, 1.0],
                ];
            }
            if ui.button(crate::i18n::tr("Krater")).clicked() {
                self.curve = vec![[0.0, 0.3], [0.5, 1.0], [1.0, 0.2]];
            }
        });
        if ui
            .add(egui::Button::new(
                RichText::new(crate::i18n::tr("Filter anwenden")).strong(),
            ))
            .clicked()
        {
            let c = self.curve.clone();
            self.op(|m| m.apply_curve(&c));
            self.status = crate::i18n::tr("Filter angewendet").into();
        }
    }

    fn picture_tools(&mut self, ui: &mut egui::Ui, settings: &Settings) {
        ui.label(crate::i18n::tr(
            "Ein Graustufenbild als Höhenkarte verwenden: hell = hoch, dunkel = tief.",
        ));
        let mut load: Option<PathBuf> = None;
        if ui.button(crate::i18n::tr("Bild wählen…")).clicked() {
            let mut d = rfd::FileDialog::new().add_filter(
                crate::i18n::tr("Bild"),
                &["tif", "tiff", "png", "jpg", "jpeg", "bmp"],
            );
            if let Some(r) = settings.library_root() {
                let p = r.join("Terrain  Maps");
                if p.is_dir() {
                    d = d.set_directory(p);
                }
            }
            load = d.pick_file();
        }
        let files = library_files(
            settings.library_root(),
            "Terrain  Maps",
            &["tif", "tiff", "png", "jpg"],
        );
        if !files.is_empty() {
            ui.label(
                RichText::new(crate::i18n::tr("Geländekarten der alten CD"))
                    .small()
                    .weak(),
            );
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .id_salt("maps")
                .show(ui, |ui| {
                    for f in &files {
                        let n = f
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let sel = self
                            .pic
                            .as_ref()
                            .map(|p| p.0 == f.to_string_lossy())
                            .unwrap_or(false);
                        if ui.selectable_label(sel, n).clicked() {
                            load = Some(f.clone());
                        }
                    }
                });
        }
        if let Some(p) = load {
            match crate::picture::open(&p) {
                Ok(img) => {
                    let hm = HeightMap::from_image(&img, self.map.size);
                    self.pic_tex = Some(ui.ctx().load_texture(
                        "pic-map",
                        map_image(&hm),
                        TextureOptions::LINEAR,
                    ));
                    self.pic = Some((p.to_string_lossy().to_string(), hm));
                    self.status = String::new();
                }
                Err(e) => self.status = crate::trf!("Bild nicht lesbar: {}", e),
            }
        }
        if let Some(t) = &self.pic_tex {
            ui.image((t.id(), Vec2::splat(120.0)));
        }
        ui.horizontal_wrapped(|ui| {
            for (i, n) in [
                crate::i18n::tr("Ersetzen"),
                crate::i18n::tr("Addieren"),
                crate::i18n::tr("Multiplizieren"),
                crate::i18n::tr("Maximum"),
            ]
            .iter()
            .enumerate()
            {
                ui.selectable_value(&mut self.pic_mode, i, *n);
            }
        });
        ui.add(egui::Slider::new(&mut self.pic_amount, 0.0..=1.0).text(crate::i18n::tr("Stärke")));
        if ui
            .add_enabled(
                self.pic.is_some(),
                egui::Button::new(RichText::new(crate::i18n::tr("Bild übernehmen")).strong()),
            )
            .clicked()
        {
            let (mode, amt) = (self.pic_mode, self.pic_amount);
            let pm = self.pic.as_ref().unwrap().1.clone();
            self.op(|m| m.blend(&pm, mode, amt));
            self.status = crate::i18n::tr("Bild übernommen").into();
        }
    }

    fn map_view(&mut self, ui: &mut egui::Ui, side: f32) {
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click_and_drag());
        if let Some(t) = &self.tex {
            ui.painter().image(
                t.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.painter().rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, Color32::from_gray(90)),
            egui::StrokeKind::Outside,
        );
        let n = self.map.size as f32;
        if let Some(h) = resp.hover_pos() {
            let rad = self.brush_size / n * side;
            ui.painter()
                .circle_stroke(h, rad, Stroke::new(1.0, Color32::from_rgb(255, 120, 60)));
        }
        if resp.drag_started() || resp.clicked() {
            self.save_history();
            self.brushing = true;
        }
        if (resp.dragged() || resp.clicked()) && self.brushing {
            if let Some(pos) = resp.interact_pointer_pos() {
                let cx = (pos.x - r.left()) / side * (n - 1.0);
                let cz = (pos.y - r.top()) / side * (n - 1.0);
                self.map.brush(
                    cx,
                    cz,
                    self.brush_size,
                    self.brush_strength,
                    self.brush_mode,
                    self.flatten_level,
                );
                self.tex_dirty = true;
            }
        }
        if resp.drag_stopped() {
            self.brushing = false;
        }
    }

    fn preview_3d(&mut self, ui: &mut egui::Ui, side: f32) {
        let (r, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::drag());
        let p = ui.painter_at(r);
        p.rect_filled(r, 0.0, Color32::from_rgb(20, 20, 24));
        if resp.dragged() {
            let d = resp.drag_delta();
            self.yaw += d.x * 0.5;
            self.pitch = (self.pitch + d.y * 0.3).clamp(10.0, 85.0);
        }
        let g = 72usize;
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let scale = side * 0.36;
        let center = r.center() + Vec2::new(0.0, side * 0.08);
        let hscale = 0.45;
        let proj = |x: f32, y: f32, z: f32| -> (Pos2, f32) {
            let rx = x * cy - z * sy;
            let rz = x * sy + z * cy;
            let py = y * cp - rz * sp;
            let depth = rz * cp + y * sp;
            (center + Vec2::new(rx * scale, -py * scale), depth)
        };
        let light = glam::Vec3::new(-0.5, 0.8, -0.3).normalize();
        let mut quads: Vec<(f32, [Pos2; 4], Color32)> = Vec::with_capacity(g * g);
        let h = |i: usize, j: usize| self.map.sample(i as f32 / g as f32, j as f32 / g as f32);
        for j in 0..g {
            for i in 0..g {
                let x0 = i as f32 / g as f32 * 2.0 - 1.0;
                let x1 = (i + 1) as f32 / g as f32 * 2.0 - 1.0;
                let z0 = j as f32 / g as f32 * 2.0 - 1.0;
                let z1 = (j + 1) as f32 / g as f32 * 2.0 - 1.0;
                let (h00, h10, h01, h11) = (h(i, j), h(i + 1, j), h(i, j + 1), h(i + 1, j + 1));
                let a = proj(x0, h00 * hscale, z0);
                let b = proj(x1, h10 * hscale, z0);
                let c = proj(x1, h11 * hscale, z1);
                let d = proj(x0, h01 * hscale, z1);
                let nx = (h00 + h01 - h10 - h11) * 0.5 * hscale * g as f32 * 0.5;
                let nz = (h00 + h10 - h01 - h11) * 0.5 * hscale * g as f32 * 0.5;
                let nrm = glam::Vec3::new(nx, 1.0, nz).normalize();
                let lum = 0.35 + 0.65 * nrm.dot(light).max(0.0);
                let hh = (h00 + h11) * 0.5;
                let base = height_color(hh);
                let col = Color32::from_rgb(
                    (base[0] as f32 * lum) as u8,
                    (base[1] as f32 * lum) as u8,
                    (base[2] as f32 * lum) as u8,
                );
                quads.push(((a.1 + b.1 + c.1 + d.1) * 0.25, [a.0, b.0, c.0, d.0], col));
            }
        }
        quads.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut mesh = egui::Mesh::default();
        for (_, q, col) in quads {
            let base = mesh.vertices.len() as u32;
            for v in q {
                mesh.colored_vertex(v, col);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
        p.add(egui::Shape::mesh(mesh));
    }
}

fn height_color(h: f32) -> [u8; 3] {
    if h < 0.35 {
        [90, 120, 60]
    } else if h < 0.62 {
        [130, 115, 95]
    } else if h < 0.75 {
        [150, 145, 140]
    } else {
        [235, 235, 240]
    }
}

fn map_image(m: &HeightMap) -> egui::ColorImage {
    let n = m.size;
    let mut buf = Vec::with_capacity(n * n * 4);
    for v in &m.h {
        let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
        buf.extend_from_slice(&[g, g, g, 255]);
    }
    egui::ColorImage::from_rgba_unmultiplied([n, n], &buf)
}
