//! The scene: objects, materials, sky & fog, camera and animation keys.

use crate::mesh::TriMesh;
use crate::terrain::{FractalKind, HeightMap};
use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Sphere,
    Cube,
    Cylinder,
    Cone,
    Torus,
    Pyramid,
    Disk,
    Square,
    Terrain,
    Stone,
    Mesh,
    Ground,
    Water,
    CloudPlane,
    Picture,
    RadialLight,
    SpotLight,
    ParallelLight,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Sphere => crate::i18n::tr("Kugel"),
            Kind::Cube => crate::i18n::tr("Würfel"),
            Kind::Cylinder => crate::i18n::tr("Zylinder"),
            Kind::Cone => crate::i18n::tr("Kegel"),
            Kind::Torus => crate::i18n::tr("Torus"),
            Kind::Pyramid => crate::i18n::tr("Pyramide"),
            Kind::Disk => crate::i18n::tr("Scheibe"),
            Kind::Square => crate::i18n::tr("Quadrat"),
            Kind::Terrain => crate::i18n::tr("Gelände"),
            Kind::Stone => crate::i18n::tr("Stein"),
            Kind::Mesh => crate::i18n::tr("Modell"),
            Kind::Ground => crate::i18n::tr("Bodenebene"),
            Kind::Water => crate::i18n::tr("Wasserebene"),
            Kind::CloudPlane => crate::i18n::tr("Wolkenebene"),
            Kind::Picture => crate::i18n::tr("Bildobjekt"),
            Kind::RadialLight => crate::i18n::tr("Punktlicht"),
            Kind::SpotLight => crate::i18n::tr("Spotlicht"),
            Kind::ParallelLight => crate::i18n::tr("Paralleles Licht"),
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Kind::Sphere => "sphere",
            Kind::Cube => "cube",
            Kind::Cylinder => "cylinder",
            Kind::Cone => "cone",
            Kind::Torus => "torus",
            Kind::Pyramid => "pyramid",
            Kind::Disk => "disk",
            Kind::Square => "square",
            Kind::Terrain => "terrain",
            Kind::Stone => "stone",
            Kind::Mesh => "mesh",
            Kind::Ground => "ground",
            Kind::Water => "water",
            Kind::CloudPlane => "cloudplane",
            Kind::Picture => "picture",
            Kind::RadialLight => "pointlight",
            Kind::SpotLight => "spotlight",
            Kind::ParallelLight => "parallellight",
        }
    }
    pub fn from_key(s: &str) -> Option<Kind> {
        Kind::ALL
            .iter()
            .copied()
            .find(|k| k.key() == s.to_lowercase())
    }
    pub const ALL: [Kind; 18] = [
        Kind::Sphere,
        Kind::Cube,
        Kind::Cylinder,
        Kind::Cone,
        Kind::Torus,
        Kind::Pyramid,
        Kind::Disk,
        Kind::Square,
        Kind::Terrain,
        Kind::Stone,
        Kind::Mesh,
        Kind::Ground,
        Kind::Water,
        Kind::CloudPlane,
        Kind::Picture,
        Kind::RadialLight,
        Kind::SpotLight,
        Kind::ParallelLight,
    ];
    pub fn is_light(self) -> bool {
        matches!(
            self,
            Kind::RadialLight | Kind::SpotLight | Kind::ParallelLight
        )
    }
    pub fn is_infinite(self) -> bool {
        matches!(self, Kind::Ground | Kind::Water | Kind::CloudPlane)
    }
    /// Wireframe colour family.
    pub fn wire_color(self) -> [u8; 3] {
        match self {
            Kind::Terrain | Kind::Stone => [200, 140, 90],
            Kind::Ground | Kind::Water | Kind::CloudPlane => [120, 160, 200],
            k if k.is_light() => [255, 230, 120],
            Kind::Mesh | Kind::Picture => [170, 200, 150],
            _ => [210, 210, 225],
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum TexKind {
    None,
    Noise,
    Marble,
    Wood,
    Granite,
    Checker,
    Stripes,
    Altitude,
    Clouds,
    Image,
    Iridescent,
}

impl TexKind {
    pub const ALL: [TexKind; 11] = [
        TexKind::None,
        TexKind::Noise,
        TexKind::Marble,
        TexKind::Wood,
        TexKind::Granite,
        TexKind::Checker,
        TexKind::Stripes,
        TexKind::Altitude,
        TexKind::Clouds,
        TexKind::Image,
        TexKind::Iridescent,
    ];
    pub fn label(self) -> &'static str {
        match self {
            TexKind::None => crate::i18n::tr("Einfarbig"),
            TexKind::Noise => crate::i18n::tr("Rauschen"),
            TexKind::Marble => crate::i18n::tr("Marmor"),
            TexKind::Wood => crate::i18n::tr("Holz"),
            TexKind::Granite => crate::i18n::tr("Granit/Fels"),
            TexKind::Checker => crate::i18n::tr("Schachbrett"),
            TexKind::Stripes => crate::i18n::tr("Streifen"),
            TexKind::Altitude => crate::i18n::tr("Landschaft (Höhe)"),
            TexKind::Clouds => crate::i18n::tr("Wolken"),
            TexKind::Image => crate::i18n::tr("Bild"),
            TexKind::Iridescent => crate::i18n::tr("Schillernd (holografisch)"),
        }
    }
    #[allow(dead_code)]
    pub fn key(self) -> &'static str {
        match self {
            TexKind::None => "none",
            TexKind::Noise => "noise",
            TexKind::Marble => "marble",
            TexKind::Wood => "wood",
            TexKind::Granite => "granite",
            TexKind::Checker => "checker",
            TexKind::Stripes => "stripes",
            TexKind::Altitude => "landscape",
            TexKind::Clouds => "clouds",
            TexKind::Image => "image",
            TexKind::Iridescent => "iridescent",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Material {
    pub name: String,
    pub diffuse: [f32; 3],
    pub color2: [f32; 3],
    pub ambient: [f32; 3],
    pub specular: [f32; 3],
    pub diffusion: f32,
    pub ambience: f32,
    pub specularity: f32,
    pub metallicity: f32,
    pub reflection: f32,
    pub transparency: f32,
    pub refraction: f32,
    pub bump: f32,
    pub texture: TexKind,
    pub tex_scale: f32,
    #[serde(default)]
    pub image: Option<String>,
    /// Light the surface gives off by itself (neon look).
    #[serde(default)]
    pub glow: f32,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            name: "Standard".into(),
            diffuse: [0.75, 0.75, 0.75],
            color2: [0.3, 0.3, 0.3],
            ambient: [0.3, 0.3, 0.35],
            specular: [1.0, 1.0, 1.0],
            diffusion: 0.85,
            ambience: 0.2,
            specularity: 0.3,
            metallicity: 0.0,
            reflection: 0.0,
            transparency: 0.0,
            refraction: 1.33,
            bump: 0.0,
            texture: TexKind::None,
            tex_scale: 1.0,
            image: None,
            glow: 0.0,
        }
    }
}

fn mat(name: &str, d: [f32; 3], c2: [f32; 3], tex: TexKind, scale: f32) -> Material {
    Material {
        name: name.into(),
        diffuse: d,
        color2: c2,
        texture: tex,
        tex_scale: scale,
        ..Default::default()
    }
}

pub fn material_presets() -> Vec<Material> {
    let mut v = vec![
        mat(
            "Standard",
            [0.75, 0.75, 0.75],
            [0.3, 0.3, 0.3],
            TexKind::None,
            1.0,
        ),
        Material {
            specularity: 0.0,
            bump: 0.6,
            ..mat(
                "Gebirgslandschaft",
                [0.33, 0.42, 0.22],
                [0.55, 0.5, 0.45],
                TexKind::Altitude,
                1.0,
            )
        },
        Material {
            bump: 0.4,
            ..mat(
                "Wüstensand",
                [0.86, 0.72, 0.5],
                [0.7, 0.55, 0.35],
                TexKind::Altitude,
                1.0,
            )
        },
        Material {
            bump: 0.8,
            specularity: 0.05,
            ..mat(
                "Fels",
                [0.5, 0.47, 0.43],
                [0.28, 0.26, 0.25],
                TexKind::Granite,
                1.5,
            )
        },
        Material {
            bump: 0.3,
            ..mat(
                "Roter Sandstein",
                [0.72, 0.36, 0.2],
                [0.5, 0.22, 0.12],
                TexKind::Stripes,
                3.0,
            )
        },
        Material {
            specularity: 0.8,
            reflection: 0.15,
            ..mat(
                "Marmor",
                [0.92, 0.92, 0.9],
                [0.35, 0.38, 0.45],
                TexKind::Marble,
                1.5,
            )
        },
        Material {
            specularity: 0.4,
            ..mat(
                "Holz",
                [0.62, 0.4, 0.2],
                [0.38, 0.22, 0.1],
                TexKind::Wood,
                2.0,
            )
        },
        Material {
            specularity: 1.0,
            metallicity: 1.0,
            reflection: 0.75,
            ..mat(
                "Chrom",
                [0.85, 0.85, 0.9],
                [0.5, 0.5, 0.5],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 1.0,
            metallicity: 1.0,
            reflection: 0.55,
            ..mat(
                "Gold",
                [1.0, 0.76, 0.3],
                [0.6, 0.4, 0.1],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 1.0,
            metallicity: 0.7,
            reflection: 0.35,
            bump: 0.2,
            ..mat(
                "Kupfer",
                [0.85, 0.45, 0.25],
                [0.3, 0.5, 0.4],
                TexKind::Noise,
                2.0,
            )
        },
        Material {
            specularity: 1.0,
            reflection: 0.1,
            transparency: 0.9,
            refraction: 1.5,
            ..mat(
                "Glas",
                [0.9, 0.95, 1.0],
                [0.9, 0.9, 0.9],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 1.0,
            reflection: 0.45,
            transparency: 0.55,
            refraction: 1.33,
            bump: 0.15,
            ..mat(
                "Wasser",
                [0.1, 0.3, 0.42],
                [0.05, 0.2, 0.3],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 0.9,
            reflection: 0.3,
            transparency: 0.4,
            refraction: 1.31,
            ..mat(
                "Eis",
                [0.75, 0.88, 0.95],
                [0.6, 0.75, 0.9],
                TexKind::Noise,
                1.0,
            )
        },
        Material {
            specularity: 0.0,
            transparency: 0.0,
            ..mat(
                "Wolken",
                [1.0, 1.0, 1.0],
                [0.7, 0.72, 0.8],
                TexKind::Clouds,
                1.0,
            )
        },
        Material {
            specularity: 0.5,
            ..mat(
                "Schachbrett",
                [0.95, 0.95, 0.95],
                [0.1, 0.1, 0.12],
                TexKind::Checker,
                2.0,
            )
        },
        Material {
            bump: 0.5,
            specularity: 0.0,
            ..mat(
                "Gras",
                [0.25, 0.48, 0.15],
                [0.15, 0.3, 0.08],
                TexKind::Noise,
                4.0,
            )
        },
        Material {
            bump: 0.25,
            specularity: 0.0,
            ..mat(
                "Schnee",
                [0.97, 0.97, 1.0],
                [0.82, 0.86, 0.95],
                TexKind::Noise,
                2.0,
            )
        },
        Material {
            bump: 0.4,
            specularity: 0.6,
            ..mat(
                "Lava",
                [1.0, 0.35, 0.05],
                [0.15, 0.05, 0.03],
                TexKind::Marble,
                2.0,
            )
        },
        Material {
            specularity: 0.7,
            ..mat(
                "Plastik Blau",
                [0.15, 0.3, 0.85],
                [0.1, 0.2, 0.6],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 0.7,
            ..mat(
                "Plastik Rot",
                [0.85, 0.12, 0.1],
                [0.6, 0.1, 0.1],
                TexKind::None,
                1.0,
            )
        },
        // --- Y2K look
        Material {
            specularity: 1.0,
            metallicity: 1.0,
            reflection: 0.85,
            ..mat(
                "Y2K Chrom",
                [0.72, 0.82, 1.0],
                [0.5, 0.6, 0.9],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 1.0,
            metallicity: 0.6,
            reflection: 0.45,
            ..mat(
                "Holografisch",
                [0.9, 0.88, 1.0],
                [1.0, 1.0, 1.0],
                TexKind::Iridescent,
                1.0,
            )
        },
        Material {
            specularity: 0.8,
            metallicity: 0.2,
            reflection: 0.2,
            ..mat(
                "Perlmutt",
                [0.96, 0.94, 0.92],
                [1.0, 1.0, 1.0],
                TexKind::Iridescent,
                0.5,
            )
        },
        Material {
            specularity: 1.0,
            reflection: 0.15,
            ..mat(
                "Bubblegum",
                [1.0, 0.42, 0.76],
                [0.8, 0.3, 0.6],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 1.0,
            reflection: 0.12,
            transparency: 0.8,
            refraction: 1.45,
            ..mat(
                "Eisglas Aqua",
                [0.55, 0.95, 1.0],
                [0.4, 0.8, 0.9],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 0.8,
            glow: 0.65,
            ..mat(
                "Neon Limette",
                [0.62, 1.0, 0.18],
                [0.4, 0.8, 0.1],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 0.8,
            glow: 0.7,
            ..mat(
                "Neon Pink",
                [1.0, 0.2, 0.78],
                [0.8, 0.1, 0.6],
                TexKind::None,
                1.0,
            )
        },
        Material {
            specularity: 1.0,
            metallicity: 1.0,
            reflection: 0.8,
            bump: 0.25,
            ..mat(
                "Flüssigmetall",
                [0.86, 0.86, 0.92],
                [0.86, 0.86, 0.92],
                TexKind::Noise,
                1.5,
            )
        },
        Material {
            specularity: 0.7,
            reflection: 0.3,
            ..mat(
                "Y2K Schachbrett",
                [1.0, 0.55, 0.86],
                [0.3, 0.82, 1.0],
                TexKind::Checker,
                1.4,
            )
        },
    ];
    for m in &mut v {
        m.ambient = [m.diffuse[0] * 0.4, m.diffuse[1] * 0.4, m.diffuse[2] * 0.45];
    }
    v
}

pub fn find_material(name: &str) -> Option<Material> {
    let n = name.to_lowercase();
    material_presets()
        .into_iter()
        .find(|m| m.name.to_lowercase() == n || m.name.to_lowercase().contains(&n))
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct LightProps {
    pub color: [f32; 3],
    pub intensity: f32,
    pub cone: f32,
    pub falloff: bool,
}

impl Default for LightProps {
    fn default() -> Self {
        LightProps {
            color: [1.0, 0.96, 0.88],
            intensity: 1.0,
            cone: 35.0,
            falloff: true,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    pub name: String,
    pub kind: Kind,
    pub pos: Vec3,
    pub rot: Vec3,
    pub scale: Vec3,
    pub material: Material,
    #[serde(default)]
    pub terrain: Option<Arc<HeightMap>>,
    #[serde(default)]
    pub mesh: Option<Arc<TriMesh>>,
    #[serde(default)]
    pub light: LightProps,
    #[serde(default)]
    pub hidden: bool,
}

impl Object {
    pub fn new(kind: Kind, id: u64) -> Object {
        let mut o = Object {
            id,
            name: format!("{} {}", kind.label(), id),
            kind,
            pos: Vec3::new(0.0, 2.0, 0.0),
            rot: Vec3::ZERO,
            scale: Vec3::splat(2.0),
            material: Material::default(),
            terrain: None,
            mesh: None,
            light: LightProps::default(),
            hidden: false,
        };
        match kind {
            Kind::Terrain => {
                o.pos = Vec3::ZERO;
                o.scale = Vec3::new(20.0, 7.0, 20.0);
                o.terrain = Some(Arc::new(HeightMap::fractal(
                    256,
                    FractalKind::Mountains,
                    id * 7919 + 3,
                )));
                o.material = find_material("Gebirgslandschaft").unwrap();
            }
            Kind::Stone => {
                o.pos = Vec3::new(0.0, 1.0, 0.0);
                o.scale = Vec3::splat(1.5);
                o.mesh = Some(Arc::new(TriMesh::stone(id * 31 + 7)));
                o.material = find_material("Fels").unwrap();
            }
            Kind::Ground => {
                o.pos = Vec3::ZERO;
                o.scale = Vec3::ONE;
                o.material = mat(
                    "Boden",
                    [0.45, 0.42, 0.33],
                    [0.32, 0.3, 0.24],
                    TexKind::Noise,
                    0.6,
                );
                o.material.specularity = 0.0;
            }
            Kind::Water => {
                o.pos = Vec3::new(0.0, 0.6, 0.0);
                o.scale = Vec3::ONE;
                o.material = find_material("Wasser").unwrap();
            }
            Kind::CloudPlane => {
                o.pos = Vec3::new(0.0, 40.0, 0.0);
                o.scale = Vec3::ONE;
                o.material = find_material("Wolken").unwrap();
            }
            Kind::Disk | Kind::Square => {
                o.pos = Vec3::new(0.0, 0.05, 0.0);
            }
            Kind::Picture => {
                o.rot = Vec3::new(90.0, 0.0, 0.0);
                o.pos = Vec3::new(0.0, 2.0, 0.0);
                o.material.texture = TexKind::Image;
            }
            Kind::RadialLight => {
                o.pos = Vec3::new(0.0, 8.0, 0.0);
                o.scale = Vec3::splat(0.5);
                o.light.intensity = 1.5;
            }
            Kind::SpotLight => {
                o.pos = Vec3::new(0.0, 10.0, 0.0);
                o.rot = Vec3::new(0.0, 0.0, 0.0);
                o.scale = Vec3::splat(0.5);
                o.light.intensity = 2.0;
            }
            Kind::ParallelLight => {
                o.pos = Vec3::new(0.0, 12.0, 0.0);
                o.rot = Vec3::new(30.0, 0.0, 0.0);
                o.scale = Vec3::splat(0.5);
                o.light.intensity = 0.6;
                o.light.falloff = false;
            }
            _ => {}
        }
        o
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            self.rot.y.to_radians(),
            self.rot.x.to_radians(),
            self.rot.z.to_radians(),
        )
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation(), self.pos)
    }

    /// Direction a spot or parallel light shines (local -Y).
    pub fn light_dir(&self) -> Vec3 {
        (self.rotation() * Vec3::NEG_Y).normalize()
    }

    pub fn polygon_count(&self) -> usize {
        match self.kind {
            Kind::Terrain => self
                .terrain
                .as_ref()
                .map(|t| (t.size - 1).pow(2) * 2)
                .unwrap_or(0),
            Kind::Stone | Kind::Mesh => self.mesh.as_ref().map(|m| m.tris.len()).unwrap_or(0),
            Kind::Sphere => 1,
            Kind::Torus => 1,
            Kind::Cube => 6,
            _ => 1,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Sky {
    pub name: String,
    pub sun_azimuth: f32,
    pub sun_elevation: f32,
    pub zenith: [f32; 3],
    pub horizon: [f32; 3],
    pub sun_color: [f32; 3],
    pub ambient: [f32; 3],
    pub sun_intensity: f32,
    pub sun_visible: bool,
    pub shadows: f32,
    pub haze: f32,
    pub haze_color: [f32; 3],
    pub fog: f32,
    pub fog_height: f32,
    pub fog_color: [f32; 3],
    pub cloud_cover: f32,
    pub cloud_frequency: f32,
    pub cloud_height: f32,
    pub cloud_amplitude: f32,
    pub cloud_color: [f32; 3],
    pub stars: bool,
    pub moon: bool,
}

impl Default for Sky {
    fn default() -> Self {
        Sky {
            name: "Sanfte Sonne".into(),
            sun_azimuth: 210.0,
            sun_elevation: 35.0,
            zenith: [0.22, 0.42, 0.78],
            horizon: [0.72, 0.8, 0.9],
            sun_color: [1.0, 0.95, 0.85],
            ambient: [0.45, 0.5, 0.6],
            sun_intensity: 1.0,
            sun_visible: true,
            shadows: 0.85,
            haze: 0.15,
            haze_color: [0.78, 0.82, 0.9],
            fog: 0.0,
            fog_height: 3.0,
            fog_color: [0.85, 0.85, 0.88],
            cloud_cover: 0.35,
            cloud_frequency: 1.0,
            cloud_height: 60.0,
            cloud_amplitude: 0.6,
            cloud_color: [1.0, 1.0, 1.0],
            stars: false,
            moon: false,
        }
    }
}

impl Sky {
    pub fn sun_dir(&self) -> Vec3 {
        let az = self.sun_azimuth.to_radians();
        let el = self.sun_elevation.to_radians();
        Vec3::new(el.cos() * az.sin(), el.sin(), -el.cos() * az.cos()).normalize()
    }
}

pub fn sky_presets() -> Vec<Sky> {
    let d = Sky::default();
    vec![
        d.clone(),
        Sky {
            name: "Klarer Mittag".into(),
            sun_elevation: 65.0,
            zenith: [0.12, 0.32, 0.75],
            horizon: [0.6, 0.75, 0.92],
            haze: 0.08,
            cloud_cover: 0.12,
            ..d.clone()
        },
        Sky {
            name: "Sonnenuntergang".into(),
            sun_azimuth: 260.0,
            sun_elevation: 4.0,
            zenith: [0.2, 0.22, 0.45],
            horizon: [1.0, 0.55, 0.25],
            sun_color: [1.0, 0.55, 0.25],
            ambient: [0.45, 0.3, 0.3],
            haze: 0.35,
            haze_color: [0.95, 0.55, 0.35],
            cloud_cover: 0.4,
            cloud_color: [1.0, 0.65, 0.5],
            ..d.clone()
        },
        Sky {
            name: "Morgennebel".into(),
            sun_azimuth: 120.0,
            sun_elevation: 12.0,
            zenith: [0.45, 0.55, 0.7],
            horizon: [0.9, 0.86, 0.8],
            sun_color: [1.0, 0.88, 0.7],
            fog: 0.35,
            fog_height: 2.5,
            haze: 0.3,
            cloud_cover: 0.15,
            ..d.clone()
        },
        Sky {
            name: "Gewitter".into(),
            sun_elevation: 30.0,
            zenith: [0.2, 0.22, 0.26],
            horizon: [0.45, 0.47, 0.5],
            sun_color: [0.7, 0.72, 0.75],
            sun_intensity: 0.6,
            sun_visible: false,
            ambient: [0.35, 0.36, 0.4],
            shadows: 0.3,
            haze: 0.4,
            haze_color: [0.5, 0.52, 0.55],
            cloud_cover: 0.85,
            cloud_amplitude: 0.9,
            cloud_color: [0.55, 0.57, 0.6],
            ..d.clone()
        },
        Sky {
            name: "Mondnacht".into(),
            sun_azimuth: 160.0,
            sun_elevation: 30.0,
            zenith: [0.01, 0.02, 0.06],
            horizon: [0.06, 0.08, 0.16],
            sun_color: [0.55, 0.6, 0.8],
            sun_intensity: 0.35,
            sun_visible: false,
            ambient: [0.1, 0.12, 0.2],
            haze: 0.05,
            haze_color: [0.05, 0.06, 0.12],
            cloud_cover: 0.15,
            cloud_color: [0.3, 0.32, 0.4],
            stars: true,
            moon: true,
            ..d.clone()
        },
        Sky {
            name: "Wüstenhitze".into(),
            sun_elevation: 55.0,
            zenith: [0.3, 0.5, 0.85],
            horizon: [0.95, 0.88, 0.75],
            sun_color: [1.0, 0.97, 0.88],
            haze: 0.35,
            haze_color: [0.95, 0.88, 0.75],
            cloud_cover: 0.0,
            ..d.clone()
        },
        Sky {
            name: "Fremder Planet".into(),
            sun_azimuth: 180.0,
            sun_elevation: 20.0,
            zenith: [0.35, 0.1, 0.3],
            horizon: [0.95, 0.5, 0.3],
            sun_color: [0.9, 1.0, 0.75],
            ambient: [0.4, 0.3, 0.35],
            haze: 0.3,
            haze_color: [0.8, 0.45, 0.4],
            cloud_cover: 0.5,
            cloud_color: [0.9, 0.6, 0.8],
            ..d.clone()
        },
        Sky {
            name: "Bewölkt".into(),
            sun_elevation: 40.0,
            zenith: [0.45, 0.52, 0.62],
            horizon: [0.78, 0.8, 0.84],
            shadows: 0.45,
            sun_intensity: 0.75,
            cloud_cover: 0.75,
            cloud_amplitude: 0.7,
            haze: 0.25,
            ..d.clone()
        },
        Sky {
            name: "Dämmerung".into(),
            sun_azimuth: 90.0,
            sun_elevation: -2.0,
            zenith: [0.06, 0.08, 0.25],
            horizon: [0.7, 0.4, 0.45],
            sun_color: [0.8, 0.45, 0.5],
            sun_intensity: 0.4,
            ambient: [0.25, 0.22, 0.32],
            haze: 0.2,
            haze_color: [0.5, 0.35, 0.45],
            cloud_cover: 0.3,
            cloud_color: [0.75, 0.5, 0.6],
            stars: true,
            ..d.clone()
        },
        // --- Y2K look
        Sky {
            name: "Vaporwave".into(),
            sun_azimuth: 180.0,
            sun_elevation: 9.0,
            zenith: [0.22, 0.07, 0.45],
            horizon: [1.0, 0.45, 0.72],
            sun_color: [1.0, 0.6, 0.82],
            ambient: [0.6, 0.45, 0.65],
            sun_intensity: 1.1,
            haze: 0.22,
            haze_color: [1.0, 0.52, 0.8],
            cloud_cover: 0.25,
            cloud_color: [1.0, 0.72, 0.95],
            ..d.clone()
        },
        Sky {
            name: "Cyber-Himmel".into(),
            sun_azimuth: 200.0,
            sun_elevation: 45.0,
            zenith: [0.0, 0.32, 0.75],
            horizon: [0.55, 0.95, 1.0],
            sun_color: [0.88, 1.0, 1.0],
            ambient: [0.5, 0.66, 0.82],
            haze: 0.1,
            haze_color: [0.6, 0.95, 1.0],
            cloud_cover: 0.3,
            ..d.clone()
        },
        Sky {
            name: "Holo-Pastell".into(),
            sun_azimuth: 220.0,
            sun_elevation: 30.0,
            zenith: [0.55, 0.7, 1.0],
            horizon: [1.0, 0.86, 0.95],
            sun_color: [1.0, 0.95, 1.0],
            ambient: [0.72, 0.66, 0.82],
            haze: 0.2,
            haze_color: [0.95, 0.86, 1.0],
            cloud_cover: 0.45,
            cloud_color: [1.0, 0.9, 1.0],
            ..d.clone()
        },
        Sky {
            name: "Millennium-Nacht".into(),
            sun_azimuth: 160.0,
            sun_elevation: 25.0,
            zenith: [0.02, 0.02, 0.12],
            horizon: [0.16, 0.05, 0.36],
            sun_color: [0.6, 0.8, 1.0],
            sun_intensity: 0.6,
            sun_visible: false,
            ambient: [0.28, 0.22, 0.48],
            haze: 0.15,
            haze_color: [0.3, 0.2, 0.6],
            cloud_cover: 0.1,
            cloud_color: [0.4, 0.3, 0.6],
            stars: true,
            ..d.clone()
        },
    ]
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Camera {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            pos: Vec3::new(0.0, 7.0, 38.0),
            yaw: 0.0,
            pitch: -8.0,
            fov: 60.0,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let y = self.yaw.to_radians();
        let p = self.pitch.to_radians();
        Vec3::new(p.cos() * y.sin(), p.sin(), -p.cos() * y.cos()).normalize()
    }
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let f = self.forward();
        let r = f.cross(Vec3::Y).normalize_or(Vec3::X);
        let u = r.cross(f).normalize();
        (f, r, u)
    }
    pub fn look_at(&mut self, target: Vec3) {
        let d = (target - self.pos).normalize_or(Vec3::NEG_Z);
        self.pitch = d.y.asin().to_degrees();
        self.yaw = d.x.atan2(-d.z).to_degrees();
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ObjKey {
    pub id: u64,
    pub pos: Vec3,
    pub rot: Vec3,
    pub scale: Vec3,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: f32,
    pub camera: Camera,
    pub objects: Vec<ObjKey>,
    pub sun_azimuth: f32,
    pub sun_elevation: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Animation {
    pub duration: f32,
    pub fps: u32,
    pub keys: Vec<Keyframe>,
}

impl Default for Animation {
    fn default() -> Self {
        Animation {
            duration: 5.0,
            fps: 15,
            keys: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub antialias: bool,
    pub max_depth: u32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings {
            width: 800,
            height: 600,
            antialias: true,
            max_depth: 4,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Scene {
    pub objects: Vec<Object>,
    pub sky: Sky,
    pub camera: Camera,
    #[serde(default)]
    pub anim: Animation,
    #[serde(default)]
    pub render: RenderSettings,
    pub next_id: u64,
}

impl Scene {
    pub fn empty() -> Scene {
        let mut s = Scene {
            objects: Vec::new(),
            sky: Sky::default(),
            camera: Camera::default(),
            anim: Animation::default(),
            render: RenderSettings::default(),
            next_id: 1,
        };
        s.add(Kind::Ground);
        s
    }

    /// The scene the program opens with: an eroded mountain range above a lake at golden hour,
    /// with a few showpieces in the foreground.
    /// The scene the program opens with: a late-90s chrome-and-gloss showcase.
    pub fn demo() -> Scene {
        use crate::i18n::tr;
        let mut s = Scene::empty();
        s.objects[0].material = find_material("Y2K Schachbrett").unwrap();
        s.objects[0].name = tr("Schachbrettboden").into();
        // distant mountains
        let t = s.add(Kind::Terrain);
        {
            let o = &mut s.objects[t];
            o.terrain = Some(Arc::new(HeightMap::fractal(256, FractalKind::Ridges, 2000)));
            o.pos = Vec3::new(0.0, -0.5, -70.0);
            o.scale = Vec3::new(90.0, 16.0, 30.0);
            o.material = find_material("Perlmutt").unwrap();
            o.name = tr("Berge").into();
        }
        let mut put =
            |kind: Kind, mat: &str, name: &'static str, pos: Vec3, scale: Vec3, rot: Vec3| {
                let i = s.add(kind);
                let o = &mut s.objects[i];
                o.material = find_material(mat).unwrap();
                o.name = tr(name).to_string();
                o.pos = pos;
                o.scale = scale;
                o.rot = rot;
            };
        put(
            Kind::Sphere,
            "Y2K Chrom",
            "Chromkugel",
            Vec3::new(0.0, 3.2, 0.0),
            Vec3::splat(3.2),
            Vec3::ZERO,
        );
        put(
            Kind::Torus,
            "Holografisch",
            "Holo-Ring",
            Vec3::new(-8.0, 4.2, 1.5),
            Vec3::splat(3.4),
            Vec3::new(70.0, 30.0, 0.0),
        );
        put(
            Kind::Cube,
            "Bubblegum",
            "Würfel",
            Vec3::new(7.5, 2.6, 2.5),
            Vec3::splat(1.7),
            Vec3::new(35.0, 40.0, 20.0),
        );
        put(
            Kind::Sphere,
            "Eisglas Aqua",
            "Glaskugel",
            Vec3::new(-2.8, 1.4, 7.0),
            Vec3::splat(1.4),
            Vec3::ZERO,
        );
        put(
            Kind::Pyramid,
            "Holografisch",
            "Pyramide",
            Vec3::new(10.0, 3.0, -9.0),
            Vec3::new(3.0, 3.0, 3.0),
            Vec3::new(0.0, 20.0, 0.0),
        );
        put(
            Kind::Cylinder,
            "Neon Pink",
            "Neonsäule",
            Vec3::new(-12.0, 3.5, -6.0),
            Vec3::new(0.45, 3.5, 0.45),
            Vec3::ZERO,
        );
        put(
            Kind::Cylinder,
            "Neon Limette",
            "Neonsäule",
            Vec3::new(13.0, 3.0, 1.0),
            Vec3::new(0.4, 3.0, 0.4),
            Vec3::ZERO,
        );
        put(
            Kind::Sphere,
            "Flüssigmetall",
            "Metallperle",
            Vec3::new(3.6, 0.9, 6.5),
            Vec3::splat(0.9),
            Vec3::ZERO,
        );
        put(
            Kind::Sphere,
            "Neon Pink",
            "Perle",
            Vec3::new(5.4, 0.55, 8.0),
            Vec3::splat(0.55),
            Vec3::ZERO,
        );
        put(
            Kind::Torus,
            "Y2K Chrom",
            "Chrom-Ring",
            Vec3::new(2.5, 0.35, 10.0),
            Vec3::new(1.6, 1.2, 1.6),
            Vec3::ZERO,
        );
        // coloured lights
        let l = s.add(Kind::RadialLight);
        s.objects[l].pos = Vec3::new(-10.0, 9.0, 8.0);
        s.objects[l].light.color = [1.0, 0.35, 0.85];
        s.objects[l].light.intensity = 2.2;
        s.objects[l].name = tr("Pinkes Licht").into();
        let l = s.add(Kind::RadialLight);
        s.objects[l].pos = Vec3::new(10.0, 8.0, 9.0);
        s.objects[l].light.color = [0.3, 0.9, 1.0];
        s.objects[l].light.intensity = 2.0;
        s.objects[l].name = tr("Cyan-Licht").into();
        s.sky = sky_presets()
            .into_iter()
            .find(|k| k.name == "Vaporwave")
            .unwrap();
        s.camera = Camera {
            pos: Vec3::new(0.0, 6.0, 25.0),
            yaw: 0.0,
            pitch: -9.0,
            fov: 58.0,
        };
        s.render.width = 1024;
        s.render.height = 576;
        s
    }

    /// The golden-hour landscape demo.
    pub fn demo_landscape() -> Scene {
        use crate::i18n::tr;
        let mut s = Scene::empty();
        s.objects[0].material.diffuse = [0.42, 0.36, 0.26];
        // mountain range in the back
        let t = s.add(Kind::Terrain);
        {
            let mut m = HeightMap::fractal(256, FractalKind::Mountains, 1997);
            m.erode(4, 1997);
            m.smooth(1);
            let o = &mut s.objects[t];
            o.terrain = Some(Arc::new(m));
            o.pos = Vec3::new(-4.0, -0.6, -70.0);
            o.scale = Vec3::new(80.0, 26.0, 40.0);
            o.name = tr("Bergkette").into();
        }
        // small island in the lake
        let i = s.add(Kind::Terrain);
        {
            let mut m = HeightMap::fractal(128, FractalKind::Hills, 42);
            m.round_edges();
            let o = &mut s.objects[i];
            o.terrain = Some(Arc::new(m));
            o.pos = Vec3::new(-19.0, -0.2, -6.0);
            o.scale = Vec3::new(11.0, 4.5, 9.0);
            o.name = tr("Insel").into();
        }
        // sandy shore in the foreground
        let beach = s.add(Kind::Terrain);
        {
            let n = 128;
            let mut m = HeightMap::flat(n);
            for z in 0..n {
                for x in 0..n {
                    let u = x as f32 / (n - 1) as f32;
                    let v = z as f32 / (n - 1) as f32;
                    let edge = 0.12 + 0.08 * (u * 9.0).sin() + 0.05 * (u * 23.0 + 1.0).sin();
                    let t = ((v - edge) / 0.3).clamp(0.0, 1.0);
                    let rise = t * t * (3.0 - 2.0 * t);
                    let ripple = crate::noise::fbm(Vec3::new(u * 12.0, 0.0, v * 12.0), 4) * 0.05;
                    m.h[z * n + x] = (rise * 0.8 + ripple * rise).clamp(0.0, 1.0);
                }
            }
            let o = &mut s.objects[beach];
            o.terrain = Some(Arc::new(m));
            o.pos = Vec3::new(0.0, 0.0, 15.0);
            o.scale = Vec3::new(40.0, 1.7, 15.0);
            o.material = Material {
                name: "Sand".into(),
                diffuse: [0.88, 0.76, 0.56],
                color2: [0.72, 0.6, 0.42],
                ambient: [0.35, 0.3, 0.22],
                texture: TexKind::Noise,
                tex_scale: 3.0,
                bump: 0.35,
                specularity: 0.05,
                ..Default::default()
            };
            o.name = tr("Strand").into();
        }
        let shore = 1.36; // top of the beach
        let w = s.add(Kind::Water);
        s.objects[w].pos.y = 0.8;
        s.objects[w].name = tr("See").into();
        // pedestal with a chrome sphere
        let ped = s.add(Kind::Cylinder);
        {
            let o = &mut s.objects[ped];
            o.pos = Vec3::new(8.0, shore + 1.3, 16.0);
            o.scale = Vec3::new(1.15, 1.3, 1.15);
            o.material = find_material("Marmor").unwrap();
            o.name = tr("Sockel").into();
        }
        let b = s.add(Kind::Sphere);
        {
            let o = &mut s.objects[b];
            o.pos = Vec3::new(8.0, shore + 2.6 + 1.6, 16.0);
            o.scale = Vec3::splat(1.6);
            o.material = find_material("Chrom").unwrap();
            o.name = tr("Chromkugel").into();
        }
        // glass sphere resting on the shore
        let g = s.add(Kind::Sphere);
        {
            let o = &mut s.objects[g];
            o.pos = Vec3::new(-4.5, shore + 1.45, 21.0);
            o.scale = Vec3::splat(1.5);
            o.material = find_material("Glas").unwrap();
            o.name = tr("Glaskugel").into();
        }
        // golden ring standing in the water
        let r = s.add(Kind::Torus);
        {
            let o = &mut s.objects[r];
            o.pos = Vec3::new(1.0, 3.2, 2.0);
            o.rot = Vec3::new(90.0, 20.0, 0.0);
            o.scale = Vec3::splat(4.2);
            o.material = find_material("Gold").unwrap();
            o.name = tr("Goldring").into();
        }
        // marble columns on the left shore
        for k in 0..3 {
            let c = s.add(Kind::Cylinder);
            let o = &mut s.objects[c];
            let hgt = 3.0 - k as f32 * 0.6;
            o.pos = Vec3::new(
                -14.0 + k as f32 * 3.4,
                shore + hgt - 0.1,
                18.5 - k as f32 * 1.8,
            );
            o.scale = Vec3::new(0.55, hgt, 0.55);
            o.material = find_material("Marmor").unwrap();
            o.name = format!("{} {}", tr("Säule"), k + 1);
        }
        // a few stones
        for (k, (x, z, sc)) in [
            (-0.5, 23.0, 1.2f32),
            (3.0, 25.0, 0.7),
            (12.0, 21.0, 1.0),
            (-9.5, 22.5, 0.8),
        ]
        .iter()
        .enumerate()
        {
            let st = s.add(Kind::Stone);
            let o = &mut s.objects[st];
            o.mesh = Some(Arc::new(TriMesh::stone(100 + k as u64)));
            o.pos = Vec3::new(*x, shore + sc * 0.45, *z);
            o.scale = Vec3::new(sc * 1.4, *sc, sc * 1.2);
            o.rot.y = k as f32 * 47.0;
            o.name = format!("{} {}", tr("Stein"), k + 1);
        }
        // golden hour
        s.sky = Sky {
            name: "Goldene Stunde".into(),
            sun_azimuth: 228.0,
            sun_elevation: 21.0,
            zenith: [0.08, 0.2, 0.52],
            horizon: [1.0, 0.66, 0.38],
            sun_color: [1.0, 0.82, 0.6],
            ambient: [0.5, 0.45, 0.5],
            sun_intensity: 1.3,
            sun_visible: true,
            shadows: 0.8,
            haze: 0.08,
            haze_color: [0.98, 0.72, 0.52],
            fog: 0.0,
            fog_height: 2.0,
            fog_color: [0.95, 0.85, 0.8],
            cloud_cover: 0.42,
            cloud_frequency: 1.2,
            cloud_height: 70.0,
            cloud_amplitude: 0.55,
            cloud_color: [1.0, 0.86, 0.76],
            stars: false,
            moon: false,
        };
        s.camera = Camera {
            pos: Vec3::new(0.0, 7.0, 41.0),
            yaw: 0.0,
            pitch: -6.0,
            fov: 58.0,
        };
        s.render.width = 1024;
        s.render.height = 576;
        s
    }

    pub fn add(&mut self, kind: Kind) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.push(Object::new(kind, id));
        self.objects.len() - 1
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.objects.iter().position(|o| o.id == id)
    }

    pub fn polygon_count(&self) -> usize {
        self.objects.iter().map(|o| o.polygon_count()).sum()
    }

    pub fn snapshot_key(&self, time: f32) -> Keyframe {
        Keyframe {
            time,
            camera: self.camera,
            objects: self
                .objects
                .iter()
                .map(|o| ObjKey {
                    id: o.id,
                    pos: o.pos,
                    rot: o.rot,
                    scale: o.scale,
                })
                .collect(),
            sun_azimuth: self.sky.sun_azimuth,
            sun_elevation: self.sky.sun_elevation,
        }
    }

    pub fn set_key(&mut self, time: f32) {
        let k = self.snapshot_key(time);
        self.anim.keys.retain(|x| (x.time - time).abs() > 0.02);
        self.anim.keys.push(k);
        self.anim
            .keys
            .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    /// Move everything to where the animation says it is at `t`.
    pub fn apply_time(&mut self, t: f32) {
        let keys = &self.anim.keys;
        if keys.is_empty() {
            return;
        }
        let n = keys.len();
        let mut i1 = keys.iter().position(|k| k.time >= t).unwrap_or(n - 1);
        if i1 == 0 {
            i1 = 1.min(n - 1);
        }
        let i0 = i1.saturating_sub(1);
        let k0 = &keys[i0];
        let k1 = &keys[i1];
        let span = (k1.time - k0.time).max(1e-4);
        let u = ((t - k0.time) / span).clamp(0.0, 1.0);
        let km = &keys[i0.saturating_sub(1)];
        let kp = &keys[(i1 + 1).min(n - 1)];
        let cr = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| -> Vec3 {
            let u2 = u * u;
            let u3 = u2 * u;
            0.5 * ((2.0 * b)
                + (-a + c) * u
                + (2.0 * a - 5.0 * b + 4.0 * c - d) * u2
                + (-a + 3.0 * b - 3.0 * c + d) * u3)
        };
        let lerp = |a: f32, b: f32| a + (b - a) * u;
        self.camera.pos = cr(km.camera.pos, k0.camera.pos, k1.camera.pos, kp.camera.pos);
        self.camera.yaw = lerp(k0.camera.yaw, k1.camera.yaw);
        self.camera.pitch = lerp(k0.camera.pitch, k1.camera.pitch);
        self.camera.fov = lerp(k0.camera.fov, k1.camera.fov);
        self.sky.sun_azimuth = lerp(k0.sun_azimuth, k1.sun_azimuth);
        self.sky.sun_elevation = lerp(k0.sun_elevation, k1.sun_elevation);
        let find = |k: &Keyframe, id: u64| k.objects.iter().find(|o| o.id == id).cloned();
        for o in &mut self.objects {
            if let (Some(a), Some(b)) = (find(k0, o.id), find(k1, o.id)) {
                let am = find(km, o.id).unwrap_or(a.clone());
                let bp = find(kp, o.id).unwrap_or(b.clone());
                o.pos = cr(am.pos, a.pos, b.pos, bp.pos);
                o.rot = a.rot + (b.rot - a.rot) * u;
                o.scale = a.scale + (b.scale - a.scale) * u;
            }
        }
    }
}
