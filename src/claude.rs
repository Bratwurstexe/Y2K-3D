//! Built-in assistant: sends the request and a summary of the scene to Claude and turns the
//! answer into scene changes.

use crate::scene::{find_material, sky_presets, Kind, Object, Scene};
use crate::terrain::{FractalKind, HeightMap};
use glam::Vec3;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub const SYSTEM_PROMPT: &str = r#"Du bist der Assistent in Y2K-3D, einem 3D-Landschaftsprogramm (Szenen aus Gelände, Himmel, Wasser, Grundkörpern und Lichtern, gerendert mit einem Raytracer).
Der Benutzer beschreibt auf Deutsch, was er in der Szene haben möchte. Du antwortest AUSSCHLIESSLICH mit einem JSON-Objekt:
{"reply": "kurze freundliche Antwort auf Deutsch", "actions": [ ... ]}

Koordinaten: Y ist oben, der Boden liegt bei y=0. Die Kamera schaut standardmäßig von +Z Richtung -Z. Eine Einheit ist etwa ein Meter. Grundkörper haben bei scale=1 den Radius 1 (Kugel, Zylinder) bzw. Kantenlänge 2 (Würfel); ein Grundkörper liegt auf dem Boden, wenn pos.y = scale.y. Gelände reicht lokal von -1..1 in X/Z und 0..1 in Y (scale z.B. [30,10,30], pos.y = 0).

Mögliche actions:
- {"op":"add","kind":K,"name":"...","pos":[x,y,z],"scale":[x,y,z] oder Zahl,"rot":[x,y,z] in Grad,"material":"Presetname","color":[r,g,b] 0..1}
  K = sphere, cube, cylinder, cone, torus, pyramid, disk, square, terrain, stone, ground, water, cloudplane, pointlight, spotlight, parallellight
- {"op":"modify","target":"Name, Id oder 'selected'", beliebige Felder wie bei add, außerdem "reflection","transparency","specularity","bump" (0..1), "hidden":true/false, "light_color":[r,g,b], "intensity":Zahl}
- {"op":"delete","target":"..."}
- {"op":"terrain","target":"...","style":"mountains|hills|ridges|canyons|dunes|crater","erode":true/false}
- {"op":"sky","preset":"Name eines Himmels-Presets", und/oder Felder: sun_azimuth (0..360), sun_elevation (-10..90), haze, fog, fog_height, cloud_cover (0..1), cloud_frequency, cloud_height, cloud_amplitude, shadows (0..1), sun_intensity, stars, moon, sun_visible, zenith, horizon, sun_color, cloud_color, fog_color, haze_color, ambient (Farben als [r,g,b] 0..1)}
- {"op":"camera","pos":[x,y,z],"look_at":[x,y,z],"fov":Grad}
- {"op":"select","target":"..."}
- {"op":"clear"}  (neue leere Szene mit Bodenebene)
- {"op":"keyframe","time":Sekunden}  (speichert den aktuellen Zustand als Schlüsselbild der Animation)
- {"op":"render"}  (startet das Rendern)

Material-Presets: MATERIALS
Himmels-Presets: SKIES
Plane mehrere Schritte, wenn nötig, und halte die Szene stimmig (Objekte nicht im Boden versenken, Kamera so, dass man das Ergebnis sieht). Wenn etwas nicht möglich ist, erkläre es in "reply" und lass actions leer."#;

pub fn system_prompt() -> String {
    let mats: Vec<String> = crate::scene::material_presets()
        .into_iter()
        .map(|m| m.name)
        .collect();
    let skies: Vec<String> = sky_presets().into_iter().map(|s| s.name).collect();
    SYSTEM_PROMPT
        .replace("MATERIALS", &mats.join(", "))
        .replace("SKIES", &skies.join(", "))
}

pub fn scene_summary(scene: &Scene, selected: Option<u64>) -> String {
    let r = |v: Vec3| {
        [
            (v.x * 100.0).round() / 100.0,
            (v.y * 100.0).round() / 100.0,
            (v.z * 100.0).round() / 100.0,
        ]
    };
    let objs: Vec<Value> = scene
        .objects
        .iter()
        .map(|o| {
            json!({"id": o.id, "name": o.name, "kind": o.kind.key(), "pos": r(o.pos), "scale": r(o.scale), "rot": r(o.rot),
                   "material": o.material.name, "hidden": o.hidden, "selected": Some(o.id) == selected})
        })
        .collect();
    let s = &scene.sky;
    json!({
        "objects": objs,
        "sky": {"name": s.name, "sun_azimuth": s.sun_azimuth, "sun_elevation": s.sun_elevation, "haze": s.haze, "fog": s.fog,
                "cloud_cover": s.cloud_cover, "stars": s.stars, "moon": s.moon},
        "camera": {"pos": r(scene.camera.pos), "yaw": scene.camera.yaw, "pitch": scene.camera.pitch, "fov": scene.camera.fov},
        "animation": {"duration": scene.anim.duration, "keyframes": scene.anim.keys.iter().map(|k| k.time).collect::<Vec<_>>()}
    })
    .to_string()
}

#[derive(Clone)]
pub struct ChatMsg {
    pub user: bool,
    pub text: String,
}

pub type Pending = Arc<Mutex<Option<Result<String, String>>>>;

/// Sends the request in the background through the installed Claude Code, which is signed in
/// with the person's own Claude subscription (Pro/Max). No API key is involved.
pub fn ask(model: String, history: Vec<ChatMsg>, scene_json: String, prompt: String) -> Pending {
    let out: Pending = Arc::new(Mutex::new(None));
    let o2 = out.clone();
    std::thread::spawn(move || {
        let res = call(&model, &history, &scene_json, &prompt);
        *o2.lock().unwrap() = Some(res);
    });
    out
}

/// Where Claude Code is installed, if it is.
pub fn find_cli() -> Option<std::path::PathBuf> {
    let names = ["claude.exe", "claude.cmd", "claude"];
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(h) = std::env::var_os("USERPROFILE") {
        let h = std::path::PathBuf::from(h);
        dirs.push(h.join(".local").join("bin"));
        dirs.push(h.join(".claude").join("local"));
    }
    if let Some(a) = std::env::var_os("APPDATA") {
        dirs.push(std::path::PathBuf::from(a).join("npm"));
    }
    if let Some(a) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(std::path::PathBuf::from(a).join("Programs").join("claude"));
    }
    for d in dirs {
        for n in names {
            let p = d.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn command(cli: &std::path::Path) -> std::process::Command {
    let is_cmd = cli
        .extension()
        .map(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        .unwrap_or(false);
    let mut c = if is_cmd {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(cli);
        c
    } else {
        std::process::Command::new(cli)
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // no console window
    }
    c.current_dir(std::env::temp_dir());
    c
}

#[derive(Clone, Debug)]
pub enum Login {
    NotInstalled,
    LoggedOut,
    Subscription(String),
    Other(String),
}

/// Whether Claude Code is installed and signed in (and how).
pub fn login_status() -> Login {
    let Some(cli) = find_cli() else {
        return Login::NotInstalled;
    };
    let Ok(out) = command(&cli).args(["auth", "status"]).output() else {
        return Login::NotInstalled;
    };
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if !v["loggedIn"].as_bool().unwrap_or(false) {
        return Login::LoggedOut;
    }
    let method = v["authMethod"].as_str().unwrap_or("").to_string();
    let plan = v["subscriptionType"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_default();
    if method.contains("claude.ai") || method.contains("oauth") {
        Login::Subscription(if plan.is_empty() {
            crate::i18n::tr("Claude-Abo").into()
        } else {
            let mut c = plan.chars();
            let cap: String = c
                .next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default();
            crate::trf!("Claude-{}-Abo", cap)
        })
    } else {
        Login::Other(method)
    }
}

/// Opens a console window in which the person signs in with their Claude account.
pub fn open_login() -> Result<(), String> {
    let cli = find_cli().ok_or(crate::i18n::tr("Claude Code ist nicht installiert."))?;
    let mut c = std::process::Command::new("cmd");
    c.arg("/C")
        .arg("start")
        .arg(crate::i18n::tr("Claude anmelden"))
        .arg("cmd")
        .arg("/K")
        .arg(cli)
        .arg("auth")
        .arg("login")
        .arg("--claudeai");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn call(
    model: &str,
    history: &[ChatMsg],
    scene_json: &str,
    prompt: &str,
) -> Result<String, String> {
    use std::io::Write;
    let cli = find_cli().ok_or(crate::i18n::tr("Claude Code wurde nicht gefunden. Bitte Claude Code installieren und mit deinem Claude-Konto anmelden (siehe unten)."))?;
    let mut text = String::new();
    let skip = history.len().saturating_sub(12);
    if history.len() > skip {
        text.push_str("Bisheriger Verlauf:\n");
        for m in &history[skip..] {
            text.push_str(if m.user { "Benutzer: " } else { "Assistent: " });
            text.push_str(&m.text);
            text.push('\n');
        }
        text.push('\n');
    }
    text.push_str(&crate::trf!(
        "Aktuelle Szene (JSON): {}\n\nWunsch: {}",
        scene_json,
        prompt
    ));
    if crate::i18n::is_english() {
        text.push_str("\n\n(The user works in English: write the \"reply\" field in English.)");
    }

    let sys_file = std::env::temp_dir().join(format!("y2k3d_system_{}.txt", std::process::id()));
    std::fs::write(&sys_file, system_prompt()).map_err(|e| e.to_string())?;
    let mut child = command(&cli)
        .arg("-p")
        .args(["--output-format", "json"])
        .arg("--system-prompt-file")
        .arg(&sys_file)
        .args(["--tools", ""])
        .arg("--strict-mcp-config")
        .arg("--no-session-persistence")
        .arg("--safe-mode")
        .args(["--model", model])
        .env_remove("ANTHROPIC_API_KEY")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| crate::trf!("Claude Code ließ sich nicht starten: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&sys_file);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(stdout.trim()).unwrap_or(Value::Null);
    let result = v["result"].as_str().unwrap_or("").to_string();
    if v["is_error"].as_bool().unwrap_or(false) || (result.is_empty() && !out.status.success()) {
        let detail: String = if !result.is_empty() {
            result
        } else {
            String::from_utf8_lossy(&out.stderr)
                .trim()
                .chars()
                .take(300)
                .collect()
        };
        let low = detail.to_lowercase();
        if low.contains("login")
            || low.contains("log in")
            || low.contains("auth")
            || low.contains("401")
        {
            return Err(crate::i18n::tr("Claude Code ist nicht angemeldet. Bitte unten auf „Mit Claude-Konto anmelden“ klicken.").into());
        }
        if low.contains("limit") {
            return Err(crate::trf!(
                "Das Nutzungslimit deines Claude-Abos ist gerade erreicht: {}",
                detail
            ));
        }
        return Err(crate::trf!("Claude meldet einen Fehler: {}", detail));
    }
    if result.is_empty() {
        return Err(crate::i18n::tr("Claude hat keine Antwort geliefert.").into());
    }
    Ok(result)
}

/// Pulls the JSON object out of an answer (it may be wrapped in a code block).
pub fn parse_answer(text: &str) -> (String, Vec<Value>) {
    let start = text.find('{');
    let end = text.rfind('}');
    if let (Some(s), Some(e)) = (start, end) {
        if let Ok(v) = serde_json::from_str::<Value>(&text[s..=e]) {
            let reply = v["reply"].as_str().unwrap_or("").to_string();
            let actions = v["actions"].as_array().cloned().unwrap_or_default();
            return (reply, actions);
        }
    }
    (text.trim().to_string(), Vec::new())
}

fn vec3(v: &Value) -> Option<Vec3> {
    if let Some(n) = v.as_f64() {
        return Some(Vec3::splat(n as f32));
    }
    let a = v.as_array()?;
    if a.len() < 3 {
        return None;
    }
    Some(Vec3::new(
        a[0].as_f64()? as f32,
        a[1].as_f64()? as f32,
        a[2].as_f64()? as f32,
    ))
}

fn color(v: &Value) -> Option<[f32; 3]> {
    vec3(v).map(|c| {
        let c = if c.max_element() > 1.0 { c / 255.0 } else { c };
        [
            c.x.clamp(0.0, 1.0),
            c.y.clamp(0.0, 1.0),
            c.z.clamp(0.0, 1.0),
        ]
    })
}

fn find_target(scene: &Scene, t: &Value, selected: Option<u64>) -> Option<usize> {
    if let Some(id) = t.as_u64() {
        return scene.index_of(id);
    }
    let s = t.as_str()?.trim();
    if s.eq_ignore_ascii_case("selected") || s.eq_ignore_ascii_case("ausgewählt") {
        return selected.and_then(|id| scene.index_of(id));
    }
    if let Ok(id) = s.parse::<u64>() {
        if let Some(i) = scene.index_of(id) {
            return Some(i);
        }
    }
    let l = s.to_lowercase();
    scene
        .objects
        .iter()
        .position(|o| o.name.to_lowercase() == l)
        .or_else(|| {
            scene
                .objects
                .iter()
                .position(|o| o.name.to_lowercase().contains(&l))
        })
        .or_else(|| {
            scene
                .objects
                .iter()
                .rposition(|o| o.kind.key() == l || o.kind.label().to_lowercase() == l)
        })
}

fn apply_fields(o: &mut Object, a: &Value) {
    if let Some(v) = vec3(&a["pos"]) {
        o.pos = v;
    }
    if let Some(v) = vec3(&a["scale"]) {
        o.scale = v.max(Vec3::splat(0.01));
    }
    if let Some(v) = vec3(&a["rot"]) {
        o.rot = v;
    }
    if let Some(n) = a["name"].as_str() {
        o.name = n.to_string();
    }
    if let Some(m) = a["material"].as_str().and_then(find_material) {
        o.material = m;
    }
    if let Some(c) = color(&a["color"]) {
        o.material.diffuse = c;
        o.material.ambient = [c[0] * 0.4, c[1] * 0.4, c[2] * 0.45];
        if o.material.texture == crate::scene::TexKind::None {
            o.material.color2 = [c[0] * 0.6, c[1] * 0.6, c[2] * 0.6];
        }
    }
    let f = |k: &str| a[k].as_f64().map(|v| (v as f32).clamp(0.0, 1.0));
    if let Some(v) = f("reflection") {
        o.material.reflection = v;
    }
    if let Some(v) = f("transparency") {
        o.material.transparency = v;
    }
    if let Some(v) = f("specularity") {
        o.material.specularity = v;
    }
    if let Some(v) = f("bump") {
        o.material.bump = v;
    }
    if let Some(h) = a["hidden"].as_bool() {
        o.hidden = h;
    }
    if let Some(c) = color(&a["light_color"]) {
        o.light.color = c;
    }
    if let Some(i) = a["intensity"].as_f64() {
        o.light.intensity = i as f32;
    }
}

fn style(s: &str) -> FractalKind {
    match s {
        "hills" => FractalKind::Hills,
        "ridges" => FractalKind::Ridges,
        "canyons" => FractalKind::Canyons,
        "dunes" => FractalKind::Dunes,
        "crater" => FractalKind::Crater,
        _ => FractalKind::Mountains,
    }
}

pub struct Applied {
    pub log: Vec<String>,
    pub select: Option<u64>,
    pub render: bool,
}

pub fn apply_actions(scene: &mut Scene, actions: &[Value], selected: Option<u64>) -> Applied {
    let mut out = Applied {
        log: Vec::new(),
        select: None,
        render: false,
    };
    let mut sel = selected;
    for a in actions {
        let op = a["op"].as_str().unwrap_or("");
        match op {
            "add" => {
                let Some(kind) = a["kind"].as_str().and_then(Kind::from_key) else {
                    out.log
                        .push(crate::trf!("Unbekannte Objektart: {}", a["kind"]));
                    continue;
                };
                if kind == Kind::Mesh || kind == Kind::Picture {
                    out.log.push(
                        crate::i18n::tr("Modelle und Bildobjekte bitte über das Menü importieren.")
                            .into(),
                    );
                    continue;
                }
                let i = scene.add(kind);
                if kind == Kind::Terrain {
                    if let Some(st) = a["style"].as_str() {
                        let seed = scene.objects[i].id * 977 + 5;
                        scene.objects[i].terrain =
                            Some(Arc::new(HeightMap::fractal(256, style(st), seed)));
                    }
                }
                apply_fields(&mut scene.objects[i], a);
                out.log
                    .push(crate::trf!("„{}“ hinzugefügt", scene.objects[i].name));
                sel = Some(scene.objects[i].id);
                out.select = sel;
            }
            "modify" | "select" | "delete" | "terrain" => {
                let Some(i) = find_target(scene, &a["target"], sel) else {
                    out.log
                        .push(crate::trf!("Objekt nicht gefunden: {}", a["target"]));
                    continue;
                };
                match op {
                    "modify" => {
                        apply_fields(&mut scene.objects[i], a);
                        out.log
                            .push(crate::trf!("„{}“ geändert", scene.objects[i].name));
                    }
                    "select" => {
                        sel = Some(scene.objects[i].id);
                        out.select = sel;
                    }
                    "delete" => {
                        let o = scene.objects.remove(i);
                        out.log.push(crate::trf!("„{}“ gelöscht", o.name));
                        if sel == Some(o.id) {
                            sel = None;
                            out.select = None;
                        }
                    }
                    _ => {
                        let o = &mut scene.objects[i];
                        if o.kind == Kind::Terrain {
                            let seed = crate::noise::time_seed();
                            let mut m = HeightMap::fractal(
                                256,
                                style(a["style"].as_str().unwrap_or("mountains")),
                                seed,
                            );
                            if a["erode"].as_bool().unwrap_or(false) {
                                m.erode(4, seed);
                            }
                            o.terrain = Some(Arc::new(m));
                            out.log
                                .push(crate::trf!("Gelände „{}“ neu geformt", o.name));
                        }
                    }
                }
            }
            "sky" => {
                let s = &mut scene.sky;
                if let Some(p) = a["preset"].as_str() {
                    let pl = p.to_lowercase();
                    if let Some(sk) = sky_presets()
                        .into_iter()
                        .find(|k| k.name.to_lowercase().contains(&pl))
                    {
                        *s = sk;
                    }
                }
                let num = |k: &str| a[k].as_f64().map(|v| v as f32);
                macro_rules! setf {
                    ($($f:ident),*) => { $( if let Some(v) = num(stringify!($f)) { s.$f = v; } )* };
                }
                setf!(
                    sun_azimuth,
                    sun_elevation,
                    haze,
                    fog,
                    fog_height,
                    cloud_cover,
                    cloud_frequency,
                    cloud_height,
                    cloud_amplitude,
                    shadows,
                    sun_intensity
                );
                macro_rules! setc {
                    ($($f:ident),*) => { $( if let Some(v) = color(&a[stringify!($f)]) { s.$f = v; } )* };
                }
                setc!(
                    zenith,
                    horizon,
                    sun_color,
                    cloud_color,
                    fog_color,
                    haze_color,
                    ambient
                );
                if let Some(b) = a["stars"].as_bool() {
                    s.stars = b;
                }
                if let Some(b) = a["moon"].as_bool() {
                    s.moon = b;
                }
                if let Some(b) = a["sun_visible"].as_bool() {
                    s.sun_visible = b;
                }
                out.log.push(crate::i18n::tr("Himmel angepasst").into());
            }
            "camera" => {
                if let Some(p) = vec3(&a["pos"]) {
                    scene.camera.pos = p;
                }
                if let Some(t) = vec3(&a["look_at"]) {
                    scene.camera.look_at(t);
                }
                if let Some(f) = a["fov"].as_f64() {
                    scene.camera.fov = (f as f32).clamp(10.0, 150.0);
                }
                out.log.push(crate::i18n::tr("Kamera bewegt").into());
            }
            "clear" => {
                *scene = Scene::empty();
                sel = None;
                out.select = None;
                out.log.push(crate::i18n::tr("Neue leere Szene").into());
            }
            "keyframe" => {
                let t = a["time"].as_f64().unwrap_or(0.0) as f32;
                scene.set_key(t.max(0.0));
                if t > scene.anim.duration {
                    scene.anim.duration = t;
                }
                out.log.push(crate::trf!("Schlüsselbild bei {:.1} s", t));
            }
            "render" => out.render = true,
            _ => out.log.push(crate::trf!("Unbekannter Schritt: {}", op)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_becomes_scene_changes() {
        let mut s = Scene::empty();
        let t = s.add(Kind::Terrain);
        s.objects[t].name = "Berge".into();
        s.add(Kind::Pyramid);
        let st = s.add(Kind::Stone);
        s.objects[st].name = "Stein 5".into();
        let text = r#"Klar! ```json
{"reply":"Erledigt","actions":[
 {"op":"add","kind":"sphere","name":"Goldkugel","pos":[0,9,-10],"scale":2,"material":"Gold"},
 {"op":"modify","target":"Pyramide","color":[0,0,1]},
 {"op":"sky","preset":"Sonnenuntergang","fog":0.2},
 {"op":"terrain","target":"Berge","style":"dunes","erode":true},
 {"op":"camera","pos":[0,10,45],"look_at":[0,3,0]},
 {"op":"keyframe","time":2},
 {"op":"delete","target":"Stein 5"},
 {"op":"render"}]}
```"#;
        let (reply, actions) = parse_answer(text);
        assert_eq!(reply, "Erledigt");
        let n0 = s.objects.len();
        let a = apply_actions(&mut s, &actions, None);
        assert!(a.render);
        assert_eq!(s.objects.len(), n0); // one added, one deleted
        let g = s.objects.iter().find(|o| o.name == "Goldkugel").unwrap();
        assert_eq!(g.material.name, "Gold");
        assert_eq!(s.sky.name, "Sonnenuntergang");
        assert!((s.sky.fog - 0.2).abs() < 1e-6);
        assert_eq!(s.anim.keys.len(), 1);
        let p = s.objects.iter().find(|o| o.kind == Kind::Pyramid).unwrap();
        assert_eq!(p.material.diffuse, [0.0, 0.0, 1.0]);
    }

    #[test]
    #[ignore]
    fn answers_through_subscription() {
        eprintln!("{:?}", login_status());
        let p = ask(
            "sonnet".into(),
            vec![],
            scene_summary(&Scene::demo(), None),
            "Mach den Himmel zum Sonnenuntergang".into(),
        );
        for _ in 0..1800 {
            if let Some(r) = p.lock().unwrap().take() {
                let t = r.expect("Antwort");
                eprintln!("{t}");
                let (_, actions) = parse_answer(&t);
                assert!(!actions.is_empty());
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("keine Antwort");
    }
}
