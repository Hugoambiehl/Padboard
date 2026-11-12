//! The PC's language, for every text the desktop produces: the editor (the
//! frontend asks for it with `get_locale`) and what it sends to the tablets
//! (button states, folder subtitles, the demo profile). French when the
//! system language is French, English otherwise; `PADBOARD_LOCALE`
//! (`fr` / `en`) forces it (end-to-end tests run in French).

use std::sync::OnceLock;

use serde::Serialize;

pub const VARIABLE: &str = "PADBOARD_LOCALE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    En,
    Fr,
}

impl Locale {
    /// `fr`, `fr-FR`, `fr_CA.UTF-8`… are French, anything else English.
    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        let language = tag.split(['-', '_', '.']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("fr") {
            Self::Fr
        } else {
            Self::En
        }
    }

    fn detect() -> Self {
        if let Ok(forced) = std::env::var(VARIABLE) {
            return Self::from_tag(&forced);
        }
        sys_locale::get_locale().map_or(Self::En, |tag| Self::from_tag(&tag))
    }
}

static LOCALE: OnceLock<Locale> = OnceLock::new();

/// The language of this run (detected once). Unit tests use French, the
/// language their expected texts are written in.
#[must_use]
pub fn current() -> Locale {
    *LOCALE.get_or_init(|| {
        if cfg!(test) {
            Locale::Fr
        } else {
            Locale::detect()
        }
    })
}

/// Texts the desktop sends to the tablets.
#[derive(Debug)]
pub struct Texts {
    pub muted: &'static str,
    pub scene_active: &'static str,
    pub recording: &'static str,
    pub live: &'static str,
    pub output_muted: &'static str,
    volume: &'static str,
    folder: &'static str,
    folder_empty: &'static str,
    folder_one: &'static str,
    folder_many: &'static str,
}

impl Texts {
    /// "Volume 40 %" / "Volume 40%".
    #[must_use]
    pub fn volume(&self, percent: u32) -> String {
        self.volume.replace("{n}", &percent.to_string())
    }

    /// Default subtitle of a folder button: "Dossier · 3 boutons".
    #[must_use]
    pub fn folder(&self, buttons: usize) -> String {
        let count = match buttons {
            0 => self.folder_empty.to_owned(),
            1 => self.folder_one.to_owned(),
            n => self.folder_many.replace("{n}", &n.to_string()),
        };
        format!("{} · {count}", self.folder)
    }
}

const FR: Texts = Texts {
    muted: "Muet",
    scene_active: "Scène active",
    recording: "REC",
    live: "EN DIRECT",
    output_muted: "Coupé",
    volume: "Volume {n} %",
    folder: "Dossier",
    folder_empty: "vide",
    folder_one: "1 bouton",
    folder_many: "{n} boutons",
};

const EN: Texts = Texts {
    muted: "Muted",
    scene_active: "Active scene",
    recording: "REC",
    live: "LIVE",
    output_muted: "Muted",
    volume: "Volume {n}%",
    folder: "Folder",
    folder_empty: "empty",
    folder_one: "1 button",
    folder_many: "{n} buttons",
};

#[must_use]
pub fn texts_for(locale: Locale) -> &'static Texts {
    match locale {
        Locale::Fr => &FR,
        Locale::En => &EN,
    }
}

/// The texts of this run's language.
#[must_use]
pub fn texts() -> &'static Texts {
    texts_for(current())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags() {
        for tag in ["fr", "fr-FR", "fr_CA.UTF-8", "FR"] {
            assert_eq!(Locale::from_tag(tag), Locale::Fr, "{tag}");
        }
        for tag in ["en-US", "de_DE.UTF-8", "C", "POSIX", ""] {
            assert_eq!(Locale::from_tag(tag), Locale::En, "{tag}");
        }
    }

    #[test]
    fn folder_subtitles() {
        assert_eq!(texts_for(Locale::Fr).folder(0), "Dossier · vide");
        assert_eq!(texts_for(Locale::Fr).folder(3), "Dossier · 3 boutons");
        assert_eq!(texts_for(Locale::En).folder(1), "Folder · 1 button");
        assert_eq!(texts_for(Locale::En).volume(40), "Volume 40%");
    }
}
