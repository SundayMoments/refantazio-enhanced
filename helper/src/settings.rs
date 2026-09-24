use crate::storage::{self, Change, Store};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub enum Kind {
    Toggle,
    Range(f32, f32),
    Choice(&'static [(&'static str, &'static str)]),
}
pub struct Setting {
    pub section: &'static str,
    pub key: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub kind: Kind,
    pub help: &'static str,
    pub group: &'static str,
}
pub const SETTINGS: &[Setting] = &[
    Setting {
        section: "Luma",
        key: "SRUserType",
        label: "Anti-aliasing",
        default: "1",
        kind: Kind::Choice(&[
            ("1", "Automatic"),
            ("2", "DLSS / DLAA (RTX)"),
            ("3", "FSR"),
            ("0", "Off"),
        ]),
        help: "Use 100% Rendering Scale in the game for native-resolution DLAA; lower scales upscale. DLSS requires an NVIDIA RTX GPU.",
        group: "Image quality",
    },
    Setting {
        section: "Luma",
        key: "DLSSRenderPreset",
        label: "DLSS preset",
        default: "0",
        kind: Kind::Choice(&[
            ("0", "Default"),
            ("10", "J"),
            ("11", "K"),
            ("12", "L"),
            ("13", "M"),
            ("5", "E (legacy)"),
            ("6", "F (legacy)"),
        ]),
        help: "Used when DLSS is active. Default lets NVIDIA choose. E/F are deprecated. Save with the game closed; applies on next launch.",
        group: "Image quality",
    },
    Setting {
        section: "Luma",
        key: "UpscalingMode",
        label: "Upscaling",
        default: "0",
        kind: Kind::Choice(&[
            ("0", "Automatic"),
            ("1", "Use selected AA method"),
            ("2", "Game scaling"),
        ]),
        help: "For FSR upscaling, choose Use selected AA method. Automatic uses DLSS when available.",
        group: "Image quality",
    },
    Setting {
        section: "Luma",
        key: "TemporalDetailBias",
        label: "Texture detail bias",
        default: "0",
        kind: Kind::Range(-1., 0.),
        help: "0 favors stability. Negative values can sharpen textures but increase shimmer. Luma's global custom mip bias overrides this setting if enabled.",
        group: "Image quality",
    },
    Setting {
        section: "Luma",
        key: "ParticleHistoryRejection",
        label: "Reject particle history",
        default: "1",
        kind: Kind::Toggle,
        help: "Experimental: can reduce particle trails at the cost of more noise.",
        group: "Image quality",
    },
    Setting {
        section: "Luma",
        key: "SceneUiMsaaSamples",
        label: "Scene UI anti-aliasing",
        default: "8",
        kind: Kind::Choice(&[("1", "Off"), ("2", "2×"), ("4", "4×"), ("8", "8×")]),
        help: "Smooths in-scene UI. Higher sample counts cost GPU memory and performance.",
        group: "Image quality",
    },
    Setting {
        section: "Luma",
        key: "EngineSkipLogos",
        label: "Skip startup logos",
        default: "1",
        kind: Kind::Toggle,
        help: "Skip logos before the title screen.",
        group: "Game",
    },
    Setting {
        section: "Luma",
        key: "EngineSkipMovie",
        label: "Skip opening movie",
        default: "0",
        kind: Kind::Toggle,
        help: "Leave off to keep the opening movie.",
        group: "Game",
    },
    Setting {
        section: "Luma",
        key: "EngineMenuFPS",
        label: "Unlock menu frame rate",
        default: "1",
        kind: Kind::Toggle,
        help: "Respects the FPS limit selected in the game with V-Sync off. Does not set a fixed FPS or refresh rate.",
        group: "Game",
    },
    Setting {
        section: "Luma",
        key: "EngineShadowResolution",
        label: "Shadow resolution",
        default: "4096",
        kind: Kind::Choice(&[
            ("2048", "2048 · stock"),
            ("4096", "4096"),
            ("8192", "8192 · demanding"),
        ]),
        help: "Higher values use more GPU memory and may reduce performance.",
        group: "Game",
    },
    Setting {
        section: "Luma",
        key: "EngineLOD",
        label: "Draw distance",
        default: "20",
        kind: Kind::Range(10., 50.),
        help: "Stock is 10. Higher values keep distant objects detailed and may reduce performance.",
        group: "Game",
    },
    Setting {
        section: "Luma",
        key: "EngineFOV",
        label: "Field of view multiplier",
        default: "1",
        kind: Kind::Range(0.75, 1.5),
        help: "1.0 preserves original gameplay framing.",
        group: "Game",
    },
    Setting {
        section: "Luma",
        key: "EngineDisableShake",
        label: "Disable camera shake",
        default: "0",
        kind: Kind::Toggle,
        help: "Removes camera shake when enabled.",
        group: "Game",
    },
    Setting {
        section: "Luma",
        key: "EngineControllerIcons",
        label: "Always show controller prompts",
        default: "0",
        kind: Kind::Toggle,
        help: "Leave off for automatic keyboard/controller prompts.",
        group: "Game",
    },
    Setting {
        section: "STYLE",
        key: "FontScale",
        label: "ReShade overlay text scale",
        default: "1.5",
        kind: Kind::Range(0.75, 3.),
        help: "1.5 is a useful starting point on a 4K display. Open the overlay with Home.",
        group: "Overlay",
    },
];

#[derive(Clone)]
pub struct Document {
    pub original: Option<Vec<u8>>,
    pub values: BTreeMap<String, String>,
}
fn key(s: &str, k: &str) -> String {
    format!("{s}/{k}")
}
fn section(line: &str) -> Option<&str> {
    let l = line.trim();
    l.strip_prefix('[')?.strip_suffix(']')
}
fn assignment(line: &str) -> Option<(&str, &str)> {
    let l = line.trim();
    if l.starts_with([';', '#']) {
        return None;
    }
    let (k, v) = l.split_once('=')?;
    Some((k.trim(), v.trim()))
}
impl Document {
    pub fn parse(original: Option<Vec<u8>>) -> Result<Self> {
        let text = std::str::from_utf8(original.as_deref().unwrap_or_default())
            .context("ReShade.ini is not UTF-8; no settings were changed")?
            .trim_start_matches('\u{feff}');
        let mut values: BTreeMap<_, _> = SETTINGS
            .iter()
            .map(|s| (key(s.section, s.key), s.default.to_owned()))
            .collect();
        let mut group = "";
        let mut seen = BTreeSet::new();
        let mut sections = BTreeSet::new();
        let mut legacy_upscaling = None;
        for line in text.lines() {
            if let Some(s) = section(line) {
                group = s;
                if SETTINGS.iter().any(|v| v.section == s) {
                    ensure!(
                        sections.insert(s),
                        "Duplicate [{s}] section; resolve it before using the helper"
                    );
                }
            } else if let Some((k, v)) = assignment(line) {
                if group == "Luma" && k == "UseSRForUpscaling" {
                    ensure!(
                        legacy_upscaling.is_none(),
                        "Duplicate legacy upscaling setting"
                    );
                    legacy_upscaling = Some(v == "1");
                }
                let id = key(group, k);
                if let Some(slot) = values.get_mut(&id) {
                    ensure!(seen.insert(id.clone()), "Duplicate setting: {id}");
                    *slot = v.into();
                }
            }
        }
        if !seen.contains("Luma/UpscalingMode") && legacy_upscaling == Some(true) {
            values.insert("Luma/UpscalingMode".into(), "1".into());
        }
        Ok(Self { original, values })
    }
    pub fn load(root: &Path) -> Result<Self> {
        Self::parse(storage::read_optional(&storage::safe_path(
            root,
            "ReShade.ini",
        )?)?)
    }
    pub fn get(&self, s: &Setting) -> &str {
        &self.values[&key(s.section, s.key)]
    }
    pub fn set(&mut self, s: &Setting, value: String) {
        self.values.insert(key(s.section, s.key), value);
    }
    pub fn defaults(&mut self) {
        for s in SETTINGS {
            self.set(s, s.default.into());
        }
    }
    pub fn render(&self) -> Result<Vec<u8>> {
        for s in SETTINGS {
            let value = self.get(s);
            let valid = match s.kind {
                Kind::Toggle => matches!(value, "0" | "1"),
                Kind::Range(min, max) => value
                    .parse::<f32>()
                    .is_ok_and(|n| n.is_finite() && n >= min && n <= max),
                Kind::Choice(options) => options.iter().any(|(v, _)| *v == value),
            };
            ensure!(
                valid,
                "Unsupported value for {}. Choose a supported value or restore recommended settings.",
                s.label
            );
        }
        let original = std::str::from_utf8(self.original.as_deref().unwrap_or_default())?;
        let bom = original.starts_with('\u{feff}');
        let newline = if original.contains("\r\n") || original.is_empty() {
            "\r\n"
        } else {
            "\n"
        };
        let mut lines: Vec<String> = original
            .trim_start_matches('\u{feff}')
            .lines()
            .map(str::to_owned)
            .collect();
        for group in ["Luma", "STYLE"] {
            let start = lines.iter().position(|l| section(l) == Some(group));
            let start = if let Some(s) = start {
                s
            } else {
                if !lines.is_empty() && !lines.last().unwrap().is_empty() {
                    lines.push(String::new());
                }
                lines.push(format!("[{group}]"));
                lines.len() - 1
            };
            let mut end = (start + 1..lines.len())
                .find(|&i| section(&lines[i]).is_some())
                .unwrap_or(lines.len());
            for s in SETTINGS.iter().filter(|s| s.section == group) {
                let replacement = format!("{}={}", s.key, self.get(s));
                if let Some(i) = (start + 1..end)
                    .find(|&i| assignment(&lines[i]).is_some_and(|(k, _)| k == s.key))
                {
                    lines[i] = replacement;
                } else {
                    lines.insert(end, replacement);
                    end += 1;
                }
            }
        }
        Ok(format!(
            "{}{}{}",
            if bom { "\u{feff}" } else { "" },
            lines.join(newline),
            newline
        )
        .into_bytes())
    }
    pub fn save(&self, store: &Store) -> Result<()> {
        ensure!(
            !store.pending()?,
            "An interrupted operation needs recovery first"
        );
        ensure!(
            !store.receipt()?.files.is_empty()
                || storage::safe_path(store.root(), "Luma-Metaphor ReFantazio.addon")?.is_file(),
            "Install the mod before saving its settings"
        );
        ensure!(
            storage::read_optional(&storage::safe_path(store.root(), "ReShade.ini")?)?
                == self.original,
            "ReShade.ini changed since loading. Reload settings before saving."
        );
        let bytes = self.render()?;
        let before = store.backup("ReShade.ini")?;
        let after = Some(store.put(&bytes)?);
        store.transact(
            store.receipt()?,
            vec![Change {
                name: "ReShade.ini".into(),
                before,
                after,
            }],
        )
    }
}
