//! What stays out of the backup. See `docs/REGRAS-DE-SINCRONISMO.md`, "O que fica de fora".
//!
//! Two kinds of rules:
//! - Rebuildable folders: a tool creates them again from a file next to them
//!   (`node_modules` from `package.json`, `target` from `Cargo.toml`).
//!   The marker file must be there; a folder with the same name elsewhere is copied.
//! - Names the user chose: `Downloads`, `*.tmp`. They match files and folders.
//!
//! The functions here do no I/O.

use std::collections::HashSet;

use serde::Serialize;

/// File that marks a cache folder (Cache Directory Tagging Specification).
pub const CACHE_TAG: &str = "CACHEDIR.TAG";
/// First bytes of a valid [`CACHE_TAG`].
pub const CACHE_TAG_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";

const JS: &[&str] = &["package.json"];
const JS_LOCK: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
    "bun.lock",
];
const PYTHON: &[&str] = &["requirements.txt", "pyproject.toml", "setup.py", "pipfile"];
const RUST: &[&str] = &["cargo.toml"];
const GRADLE: &[&str] = &[
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
];
const DOTNET: &[&str] = &["*.csproj", "*.vbproj", "*.fsproj"];
const DOTNET_SOLUTION: &[&str] = &["*.sln"];
const FLUTTER: &[&str] = &["pubspec.yaml"];
const COCOAPODS: &[&str] = &["podfile"];

/// A folder that a tool creates again.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Rule {
    /// Folder name, in lowercase.
    #[serde(rename = "pasta")]
    pub folder: &'static str,
    /// The parent folder must have one file of each group. `*.ext` matches any name
    /// with that ending. No group: the folder always stays out.
    #[serde(rename = "precisa")]
    pub needs: &'static [&'static [&'static str]],
    /// Who creates the folder again, in Portuguese.
    #[serde(rename = "quem_recria")]
    pub tool: &'static str,
}

const fn rule(
    folder: &'static str,
    needs: &'static [&'static [&'static str]],
    tool: &'static str,
) -> Rule {
    Rule {
        folder,
        needs,
        tool,
    }
}

/// Rebuildable folders. `vendor` is not here: WordPress plugins keep needed code in it.
pub const REBUILDABLE: &[Rule] = &[
    rule("node_modules", &[JS], "npm, yarn ou pnpm (JavaScript)"),
    rule(".next", &[JS], "Next.js"),
    rule(".nuxt", &[JS], "Nuxt"),
    rule(".svelte-kit", &[JS], "SvelteKit"),
    rule(".angular", &[JS], "Angular"),
    rule(".astro", &[JS], "Astro"),
    rule(".turbo", &[JS], "Turborepo"),
    rule(".parcel-cache", &[JS], "Parcel"),
    rule(".vite", &[JS], "Vite"),
    rule(".expo", &[JS], "Expo"),
    rule("dist", &[JS, JS_LOCK], "o build do projeto JavaScript"),
    rule("build", &[JS, JS_LOCK], "o build do projeto JavaScript"),
    rule("out", &[JS, JS_LOCK], "o build do projeto JavaScript"),
    rule(".output", &[JS, JS_LOCK], "o build do projeto JavaScript"),
    rule("__pycache__", &[], "Python"),
    rule(".pytest_cache", &[], "pytest"),
    rule(".mypy_cache", &[], "mypy"),
    rule(".ruff_cache", &[], "Ruff"),
    rule(".venv", &[PYTHON], "pip, a partir da lista de pacotes"),
    rule("venv", &[PYTHON], "pip, a partir da lista de pacotes"),
    rule("target", &[RUST], "cargo (Rust)"),
    rule("build", &[GRADLE], "Gradle (Android)"),
    rule(".gradle", &[GRADLE], "Gradle (Android)"),
    rule("bin", &[DOTNET], "Visual Studio (.NET)"),
    rule("obj", &[DOTNET], "Visual Studio (.NET)"),
    rule(".vs", &[DOTNET_SOLUTION], "Visual Studio"),
    rule(".dart_tool", &[FLUTTER], "Flutter"),
    rule("build", &[FLUTTER], "Flutter"),
    rule("pods", &[COCOAPODS], "CocoaPods (iOS)"),
];

/// Temporary files: a program creates them while it works and does not need them later.
/// `~$*` is the lock file that Word and Excel leave next to an open document.
pub const TEMPORARY: &[&str] = &[
    "*.tmp",
    "*.temp",
    "~$*",
    "*.crdownload",
    "*.swp",
    "*.swo",
    "*.pyc",
    "*.pyo",
    ".ds_store",
];

/// Whether `text` is a valid name to skip: not empty, no path separator, not only `*`.
pub fn is_valid_name(text: &str) -> bool {
    Pattern::parse(text).is_some()
}

/// A name the user chose: exact, `*.ext` for an ending or `~$*` for a start.
/// Compared without case.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Exact(String),
    Ending(String),
    Start(String),
}

impl Pattern {
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_lowercase();
        if text.is_empty() || text.contains('/') || text.contains('\\') {
            return None;
        }
        if let Some(start) = text.strip_suffix('*') {
            if !start.is_empty() && !start.contains('*') {
                return Some(Self::Start(start.to_owned()));
            }
        }
        match text.strip_prefix('*') {
            Some("") => None,
            Some(ending) => Some(Self::Ending(ending.to_owned())),
            None => Some(Self::Exact(text)),
        }
    }

    fn matches(&self, name: &str) -> bool {
        match self {
            Self::Exact(exact) => name == exact,
            Self::Ending(ending) => name.len() > ending.len() && name.ends_with(ending.as_str()),
            Self::Start(start) => name.len() > start.len() && name.starts_with(start.as_str()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Exclusions {
    /// Leave out the [`REBUILDABLE`] folders and the folders with a [`CACHE_TAG`].
    pub rebuildable: bool,
    /// Leave out the [`TEMPORARY`] files.
    pub temporary: bool,
    names: Vec<Pattern>,
}

impl Exclusions {
    /// No rule: everything is copied.
    pub const fn none() -> Self {
        Self {
            rebuildable: false,
            temporary: false,
            names: Vec::new(),
        }
    }

    /// `names`: the user's names. An empty name or a name with a path separator is ignored.
    pub fn new(rebuildable: bool, names: &[String]) -> Self {
        Self {
            rebuildable,
            temporary: false,
            names: names
                .iter()
                .filter_map(|name| Pattern::parse(name))
                .collect(),
        }
    }

    /// Also leave out the [`TEMPORARY`] files when `on`.
    pub fn with_temporary(mut self, on: bool) -> Self {
        if on && !self.temporary {
            self.temporary = true;
            self.names
                .extend(TEMPORARY.iter().filter_map(|name| Pattern::parse(name)));
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        !self.rebuildable && self.names.is_empty()
    }

    /// Whether a file or folder named `name` (in lowercase) is one of the user's names
    /// or a temporary file.
    pub fn skips_name(&self, name: &str) -> bool {
        self.names.iter().any(|pattern| pattern.matches(name))
    }

    /// Whether a rebuildable rule might apply to a folder named `name` (in lowercase).
    /// When `false`, the caller does not need the names of the files next to it.
    pub fn may_skip_folder(&self, name: &str) -> bool {
        self.rebuildable && REBUILDABLE.iter().any(|rule| rule.folder == name)
    }

    /// The rebuildable rule for a folder named `name` whose parent has the files
    /// `siblings`. All names in lowercase.
    pub fn rebuildable_rule(
        &self,
        name: &str,
        siblings: &HashSet<String>,
    ) -> Option<&'static Rule> {
        if !self.rebuildable {
            return None;
        }
        REBUILDABLE.iter().find(|rule| {
            rule.folder == name && rule.needs.iter().all(|group| has_one_of(siblings, group))
        })
    }

    /// Whether a folder named `name` (in lowercase) that holds a valid [`CACHE_TAG`]
    /// stays out. A virtual environment follows its own rule: some tools tag it, but
    /// without a package list nobody knows what to install again.
    pub fn skips_tagged_cache(&self, name: &str) -> bool {
        self.rebuildable && !matches!(name, ".venv" | "venv")
    }

    /// Whether an index key stays out: one of its folders is in `folders`, or one
    /// of its names is one of the user's names. `key` and `folders` are folded with
    /// [`crate::paths::fold_key`].
    pub fn covers(&self, key: &str, folders: &HashSet<String>) -> bool {
        if !folders.is_empty() {
            let mut end = 0;
            while let Some(slash) = key[end..].find('/') {
                end += slash;
                if folders.contains(&key[..end]) {
                    return true;
                }
                end += 1;
            }
        }
        !self.names.is_empty()
            && key
                .split('/')
                .any(|name| self.skips_name(&name.to_lowercase()))
    }
}

fn has_one_of(siblings: &HashSet<String>, group: &[&str]) -> bool {
    group.iter().any(|marker| match marker.strip_prefix('*') {
        Some(ending) => siblings.iter().any(|name| name.ends_with(ending)),
        None => siblings.contains(*marker),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn node_modules_stays_out_only_next_to_package_json() {
        let rules = Exclusions::new(true, &[]);
        assert!(rules
            .rebuildable_rule("node_modules", &files(&["package.json", "index.html"]))
            .is_some());
        assert!(rules
            .rebuildable_rule("node_modules", &files(&["leia-me.txt"]))
            .is_none());
    }

    #[test]
    fn build_output_needs_package_json_and_a_lock_file() {
        let rules = Exclusions::new(true, &[]);
        let project = files(&["package.json", "yarn.lock"]);
        assert!(rules.rebuildable_rule("dist", &project).is_some());
        // A WordPress plugin can have a package.json and ship its own `dist`.
        assert!(rules
            .rebuildable_rule("dist", &files(&["package.json", "plugin.php"]))
            .is_none());
        assert!(rules
            .rebuildable_rule("dist", &files(&["index.php"]))
            .is_none());
    }

    #[test]
    fn same_name_can_come_from_different_tools() {
        let rules = Exclusions::new(true, &[]);
        let android = rules.rebuildable_rule("build", &files(&["build.gradle.kts"]));
        assert_eq!(android.map(|rule| rule.tool), Some("Gradle (Android)"));
        let dotnet = rules.rebuildable_rule("bin", &files(&["conversor.csproj", "program.cs"]));
        assert!(dotnet.is_some());
    }

    #[test]
    fn python_cache_always_stays_out_and_venv_needs_a_package_list() {
        let rules = Exclusions::new(true, &[]);
        assert!(rules.rebuildable_rule("__pycache__", &files(&[])).is_some());
        assert!(rules
            .rebuildable_rule(".venv", &files(&["requirements.txt"]))
            .is_some());
        // Without a package list, nobody knows what to install again.
        assert!(rules
            .rebuildable_rule(".venv", &files(&["narrar.py"]))
            .is_none());
        assert!(!rules.skips_tagged_cache(".venv"));
        assert!(rules.skips_tagged_cache("target-teste"));
    }

    #[test]
    fn nothing_stays_out_when_the_option_is_off() {
        let rules = Exclusions::new(false, &[]);
        assert!(rules.is_empty());
        assert!(!rules.may_skip_folder("node_modules"));
        assert!(rules
            .rebuildable_rule("node_modules", &files(&["package.json"]))
            .is_none());
        assert!(!rules.skips_tagged_cache("target"));
    }

    #[test]
    fn user_names_match_exact_names_and_endings() {
        let rules = Exclusions::new(false, &["Downloads".into(), "*.TMP".into(), " ".into()]);
        assert!(rules.skips_name("downloads"));
        assert!(rules.skips_name("rascunho.tmp"));
        assert!(!rules.skips_name(".tmp"));
        assert!(!rules.skips_name("downloads antigos"));
    }

    #[test]
    fn temporary_files_stay_out_only_when_the_option_is_on() {
        let off = Exclusions::new(false, &[]);
        assert!(!off.skips_name("rascunho.tmp"));
        let on = Exclusions::new(false, &[]).with_temporary(true);
        assert!(!on.is_empty());
        assert!(on.skips_name("rascunho.tmp"));
        assert!(on.skips_name("~$relatorio.docx"));
        assert!(on.skips_name(".ds_store"));
        assert!(!on.skips_name("relatorio.docx"));
        assert!(!on.skips_name("~$"));
    }

    #[test]
    fn a_name_can_match_a_start() {
        let rules = Exclusions::new(false, &["Copia de*".into()]);
        assert!(rules.skips_name("copia de notas.txt"));
        assert!(!rules.skips_name("notas.txt"));
        assert!(is_valid_name("~$*"));
        assert!(!is_valid_name("*"));
        assert!(!is_valid_name("a/b"));
    }

    #[test]
    fn a_name_with_a_folder_separator_is_ignored() {
        let rules = Exclusions::new(false, &["site/node_modules".into(), "*".into()]);
        assert!(rules.is_empty());
    }

    #[test]
    fn covers_keys_under_a_left_out_folder() {
        let rules = Exclusions::new(true, &["*.log".into()]);
        let folders = files(&["app/node_modules", "site/dist"]);
        assert!(rules.covers("app/node_modules/react/index.js", &folders));
        assert!(rules.covers("site/dist/index.html", &folders));
        assert!(rules.covers("notas/erro.log", &folders));
        assert!(!rules.covers("app/node_modules_velho/index.js", &folders));
        assert!(!rules.covers("app/package.json", &folders));
        assert!(!rules.covers("site/dist", &folders));
    }
}
