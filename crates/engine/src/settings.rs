//! Program settings, stored in `%APPDATA%\backup-leve\configuracoes.toml`.
//!
//! The file keeps the AI service and its key, and the options of a new profile.
//! The key is in plain text in the user's folder. The window says this to the user.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const SETTINGS_FILE: &str = "configuracoes.toml";

/// AI service that suggests names to skip. The AI only suggests; the user decides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Nenhum,
    Openai,
    Grok,
    Anthropic,
}

impl Provider {
    /// Model used when the user does not choose one.
    pub fn default_model(self) -> &'static str {
        match self {
            Self::Nenhum => "",
            Self::Openai => "gpt-4.1-mini",
            Self::Grok => "grok-3-mini",
            Self::Anthropic => "claude-haiku-5-5",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    #[serde(rename = "chave", default)]
    pub key: String,
    /// Empty: [`Provider::default_model`].
    #[serde(rename = "modelo", default)]
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "provedor", default)]
    pub provider: Provider,
    #[serde(default)]
    pub openai: Key,
    #[serde(default)]
    pub grok: Key,
    #[serde(default)]
    pub anthropic: Key,
    /// Options of a new profile.
    #[serde(rename = "novo_perfil_pular_recriaveis", default = "on")]
    pub new_profile_skip_rebuildable: bool,
    #[serde(rename = "novo_perfil_pular_temporarios", default = "on")]
    pub new_profile_skip_temporary: bool,
}

fn on() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: Provider::Nenhum,
            openai: Key::default(),
            grok: Key::default(),
            anthropic: Key::default(),
            new_profile_skip_rebuildable: true,
            new_profile_skip_temporary: true,
        }
    }
}

impl Settings {
    /// The key and the model of the chosen service. `None` without a service or key.
    pub fn active(&self) -> Option<(Provider, String, String)> {
        let key = match self.provider {
            Provider::Nenhum => return None,
            Provider::Openai => &self.openai,
            Provider::Grok => &self.grok,
            Provider::Anthropic => &self.anthropic,
        };
        if key.key.trim().is_empty() {
            return None;
        }
        let model = if key.model.trim().is_empty() {
            self.provider.default_model().to_owned()
        } else {
            key.model.trim().to_owned()
        };
        Some((self.provider, key.key.trim().to_owned(), model))
    }
}

/// `%APPDATA%\backup-leve\configuracoes.toml` on Windows, the equivalent elsewhere.
pub fn default_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("backup-leve").join(SETTINGS_FILE))
}

/// Reads the settings. A missing file gives the default settings.
pub fn load(path: &Path) -> Result<Settings> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(err) => return Err(err.into()),
    };
    toml::from_str(&text).map_err(|err| Error::Settings(format!("{}: {err}", path.display())))
}

/// Writes the settings. The old file is replaced only after a complete write.
pub fn save(path: &Path, settings: &Settings) -> Result<()> {
    let text = toml::to_string_pretty(settings).map_err(|err| Error::Settings(err.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial = path.with_extension("toml.parcial");
    fs::write(&partial, text)?;
    fs::rename(&partial, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = load(&tmp.path().join(SETTINGS_FILE)).unwrap();
        assert_eq!(settings, Settings::default());
        assert!(settings.active().is_none());
    }

    #[test]
    fn save_and_load_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(SETTINGS_FILE);
        let mut settings = Settings {
            provider: Provider::Grok,
            ..Settings::default()
        };
        settings.grok.key = " xai-123 ".into();
        save(&path, &settings).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, settings);
        assert_eq!(
            loaded.active(),
            Some((Provider::Grok, "xai-123".into(), "grok-3-mini".into()))
        );
    }

    #[test]
    fn a_service_without_key_is_not_active() {
        let settings = Settings {
            provider: Provider::Anthropic,
            ..Settings::default()
        };
        assert!(settings.active().is_none());
    }
}
