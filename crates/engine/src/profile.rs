//! Backup profiles, stored in `%APPDATA%\backup-leve\perfis.toml` (ADR 0003).
//!
//! A profile has one destination and one or more source folders.
//! Each source goes to `<destination>/<source folder name>`.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf, Prefix};

use serde::{Deserialize, Serialize};

use crate::decision::Mode;
use crate::error::{Error, Result};
use crate::exclude::Exclusions;
use crate::paths;

pub const PROFILES_FILE: &str = "perfis.toml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "destino")]
    pub destination: PathBuf,
    #[serde(rename = "origens", default)]
    pub sources: Vec<PathBuf>,
    #[serde(rename = "modo", default)]
    pub mode: Mode,
    /// Leave out the folders that a tool creates again (`node_modules`, `target`...).
    /// On by default, also for profiles saved before this option existed.
    #[serde(rename = "pular_recriaveis", default = "on")]
    pub skip_rebuildable: bool,
    /// Names of files and folders that stay out. `*.tmp` matches an ending.
    #[serde(rename = "pular", default)]
    pub skip_names: Vec<String>,
}

fn on() -> bool {
    true
}

/// One source folder and the destination folder that receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Target {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProfilesFile {
    #[serde(rename = "perfil", default)]
    profiles: Vec<Profile>,
}

impl Profile {
    /// What stays out of the backup of each source.
    pub fn exclusions(&self) -> Exclusions {
        Exclusions::new(self.skip_rebuildable, &self.skip_names)
    }

    /// Pairs of source and destination folders, in the order of `sources`.
    pub fn targets(&self) -> Vec<Target> {
        self.sources
            .iter()
            .map(|source| Target {
                source: source.clone(),
                destination: self.destination.join(folder_label(source)),
            })
            .collect()
    }

    /// Problems that make the profile unusable, in Portuguese. Empty when valid.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.name.trim().is_empty() {
            problems.push("o perfil precisa de um nome".to_owned());
        }
        if self.destination.as_os_str().is_empty() {
            problems.push("escolha o disco de destino".to_owned());
        }
        if self.sources.is_empty() {
            problems.push("escolha pelo menos uma pasta".to_owned());
        }
        let mut labels = HashSet::new();
        for source in &self.sources {
            let label = folder_label(source);
            if !labels.insert(paths::fold_key(&label)) {
                problems.push(format!(
                    "duas pastas têm o mesmo nome \"{label}\"; no destino, uma ocuparia o lugar da outra"
                ));
            }
        }
        problems
    }
}

/// Name of the destination subfolder for `source`: its last name, or `Disco D` for a drive root.
pub fn folder_label(source: &Path) -> String {
    if let Some(name) = source.file_name() {
        return name.to_string_lossy().into_owned();
    }
    if let Some(Component::Prefix(prefix)) = source.components().next() {
        if let Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) = prefix.kind() {
            return format!("Disco {}", (letter as char).to_ascii_uppercase());
        }
    }
    "raiz".to_owned()
}

/// `%APPDATA%\backup-leve\perfis.toml` on Windows, the equivalent elsewhere.
pub fn default_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("backup-leve").join(PROFILES_FILE))
}

/// Reads the profiles. A missing file means no profiles yet.
pub fn load(path: &Path) -> Result<Vec<Profile>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let file: ProfilesFile = toml::from_str(&text)
        .map_err(|err| Error::Profile(format!("{}: {err}", path.display())))?;
    Ok(file.profiles)
}

/// Validates and writes the profiles. The old file is replaced only after a complete write.
pub fn save(path: &Path, profiles: &[Profile]) -> Result<()> {
    let mut names = HashSet::new();
    for profile in profiles {
        if let Some(problem) = profile.problems().into_iter().next() {
            return Err(Error::Profile(format!(
                "perfil \"{}\": {problem}",
                profile.name
            )));
        }
        if !names.insert(profile.name.trim().to_lowercase()) {
            return Err(Error::Profile(format!(
                "já existe um perfil com o nome \"{}\"",
                profile.name
            )));
        }
    }
    let text = toml::to_string_pretty(&ProfilesFile {
        profiles: profiles.to_vec(),
    })
    .map_err(|err| Error::Profile(err.to_string()))?;
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

    fn profile(name: &str, sources: &[&str]) -> Profile {
        Profile {
            name: name.to_owned(),
            destination: PathBuf::from("E:/Backup"),
            sources: sources.iter().map(PathBuf::from).collect(),
            mode: Mode::Sync,
            skip_rebuildable: true,
            skip_names: Vec::new(),
        }
    }

    #[test]
    fn each_source_goes_to_a_folder_with_its_name() {
        let targets = profile("Trabalho", &["D:/Docs", "C:/Fotos"]).targets();
        assert_eq!(
            targets[0].destination,
            PathBuf::from("E:/Backup").join("Docs")
        );
        assert_eq!(
            targets[1].destination,
            PathBuf::from("E:/Backup").join("Fotos")
        );
    }

    #[cfg(windows)]
    #[test]
    fn drive_root_gets_a_readable_name() {
        assert_eq!(folder_label(Path::new("d:\\")), "Disco D");
    }

    #[test]
    fn sources_with_the_same_name_are_a_problem() {
        let problems = profile("Trabalho", &["D:/a/Docs", "C:/b/docs"]).problems();
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn save_and_load_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config").join(PROFILES_FILE);
        let mut second = profile("Fotos", &["D:/Fotos"]);
        second.mode = Mode::AddOnly;
        let profiles = vec![profile("Trabalho", &["D:/Docs"]), second];

        save(&path, &profiles).unwrap();

        assert_eq!(load(&path).unwrap(), profiles);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[[perfil]]"));
        assert!(text.contains("modo = \"somente-novos\""));
    }

    #[test]
    fn an_old_profile_leaves_out_the_rebuildable_folders() {
        let text =
            "[[perfil]]\nnome = \"Antigo\"\ndestino = 'E:\\Backup'\norigens = ['D:\\Docs']\n";
        let file: ProfilesFile = toml::from_str(text).unwrap();
        let old = &file.profiles[0];
        assert!(old.skip_rebuildable);
        assert!(old.skip_names.is_empty());
        assert!(!old.exclusions().is_empty());
    }

    #[test]
    fn missing_file_means_no_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(&tmp.path().join(PROFILES_FILE)).unwrap().is_empty());
    }

    #[test]
    fn duplicate_names_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let profiles = vec![
            profile("Trabalho", &["D:/a"]),
            profile("trabalho", &["D:/b"]),
        ];
        assert!(save(&tmp.path().join(PROFILES_FILE), &profiles).is_err());
    }
}
