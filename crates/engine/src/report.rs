//! Report of one run, saved as JSON and HTML in `<destination>/.backup-leve/relatorios/`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::decision::{ConflictKind, Mode, Snapshot};
use crate::error::Result;

/// The HTML shows at most this many rows per list. The JSON has all of them.
const HTML_ROW_LIMIT: usize = 1_000;

#[derive(Debug, Clone, Serialize)]
pub struct FileItem {
    pub path: String,
    pub size: u64,
}

impl FileItem {
    pub fn new(path: &str, size: u64) -> Self {
        Self {
            path: path.to_owned(),
            size,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictItem {
    pub path: String,
    pub kind: ConflictKind,
    pub source: Option<Snapshot>,
    pub destination: Option<Snapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorItem {
    pub path: String,
    pub message: String,
}

impl ErrorItem {
    pub fn new(path: &str, message: &str) -> Self {
        Self {
            path: path.to_owned(),
            message: message.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub version: &'static str,
    pub started_at: String,
    pub finished_at: String,
    pub source: String,
    pub destination: String,
    pub mode: Mode,
    pub dry_run: bool,
    pub copied: Vec<FileItem>,
    pub updated: Vec<FileItem>,
    /// Already equal on both sides: recorded in the index without copying.
    pub adopted: Vec<FileItem>,
    pub changed_in_source: Vec<FileItem>,
    pub removed_from_source: Vec<FileItem>,
    pub conflicts: Vec<ConflictItem>,
    pub errors: Vec<ErrorItem>,
    pub unchanged: u64,
    pub forgotten: u64,
    pub partials_removed: u64,
    pub bytes_copied: u64,
    /// HTML report on disk. `None` on a dry run.
    #[serde(skip)]
    pub saved_to: Option<PathBuf>,
}

impl Report {
    pub fn new(
        source: &Path,
        destination: &Path,
        mode: Mode,
        dry_run: bool,
        started_at: DateTime<Local>,
    ) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            started_at: started_at.to_rfc3339(),
            finished_at: String::new(),
            source: display_absolute(source),
            destination: display_absolute(destination),
            mode,
            dry_run,
            copied: Vec::new(),
            updated: Vec::new(),
            adopted: Vec::new(),
            changed_in_source: Vec::new(),
            removed_from_source: Vec::new(),
            conflicts: Vec::new(),
            errors: Vec::new(),
            unchanged: 0,
            forgotten: 0,
            partials_removed: 0,
            bytes_copied: 0,
            saved_to: None,
        }
    }

    /// Writes `relatorio_<stamp>.json` and `relatorio_<stamp>.html` into `dir`.
    pub fn save(&mut self, dir: &Path, stamp: &str) -> Result<()> {
        fs::create_dir_all(dir)?;
        let base = format!("relatorio_{stamp}");
        fs::write(
            dir.join(format!("{base}.json")),
            serde_json::to_vec_pretty(self)?,
        )?;
        let html = dir.join(format!("{base}.html"));
        fs::write(&html, self.to_html())?;
        self.saved_to = Some(html);
        Ok(())
    }

    pub fn to_html(&self) -> String {
        let mut html = String::new();
        let _ = write!(
            html,
            "<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <title>Relatório do Backup Leve</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n"
        );
        let _ = writeln!(
            html,
            "<header><h1>Relatório do Backup Leve</h1>\
             <p class=\"route\"><span>{}</span> → <span>{}</span></p>\
             <p class=\"meta\">{} · modo {}{}</p></header>",
            escape(&self.source),
            escape(&self.destination),
            escape(&format_rfc3339(&self.started_at)),
            mode_label(self.mode),
            if self.dry_run {
                " · simulação, nada foi alterado"
            } else {
                ""
            },
        );

        html.push_str("<section class=\"cards\">\n");
        let copied_label = format!(
            "{} ({})",
            self.copied.len(),
            format_bytes(self.copied.iter().map(|item| item.size).sum())
        );
        for (label, value, alert) in [
            ("Copiados", copied_label, false),
            ("Atualizados", self.updated.len().to_string(), false),
            (
                "Registrados sem cópia",
                self.adopted.len().to_string(),
                false,
            ),
            ("Sem mudança", self.unchanged.to_string(), false),
            (
                "Alterados na origem",
                self.changed_in_source.len().to_string(),
                false,
            ),
            (
                "Removidos na origem",
                self.removed_from_source.len().to_string(),
                false,
            ),
            (
                "Conflitos",
                self.conflicts.len().to_string(),
                !self.conflicts.is_empty(),
            ),
            (
                "Erros",
                self.errors.len().to_string(),
                !self.errors.is_empty(),
            ),
        ] {
            let class = if alert { "card alert" } else { "card" };
            let _ = writeln!(
                html,
                "<div class=\"{class}\"><span>{label}</span><strong>{}</strong></div>",
                escape(&value)
            );
        }
        html.push_str("</section>\n");

        if !self.conflicts.is_empty() {
            html.push_str(
                "<section><h2>Conflitos</h2><p class=\"note\">O programa não alterou estes arquivos. \
                 Você decide o que fazer com cada um.</p>\
                 <table><thead><tr><th>Arquivo</th><th>Situação</th><th>Origem</th><th>Destino</th></tr></thead><tbody>\n",
            );
            for item in self.conflicts.iter().take(HTML_ROW_LIMIT) {
                let _ = writeln!(
                    html,
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape(&item.path),
                    conflict_label(item.kind),
                    snapshot_cell(item.source),
                    snapshot_cell(item.destination),
                );
            }
            html.push_str("</tbody></table>");
            limit_note(&mut html, self.conflicts.len());
            html.push_str("</section>\n");
        }

        if !self.errors.is_empty() {
            html.push_str(
                "<section><h2>Erros</h2><p class=\"note\">Estes arquivos não foram copiados. \
                 A próxima execução tenta de novo.</p>\
                 <table><thead><tr><th>Arquivo</th><th>Mensagem</th></tr></thead><tbody>\n",
            );
            for item in self.errors.iter().take(HTML_ROW_LIMIT) {
                let _ = writeln!(
                    html,
                    "<tr><td>{}</td><td>{}</td></tr>",
                    escape(&item.path),
                    escape(&item.message)
                );
            }
            html.push_str("</tbody></table>");
            limit_note(&mut html, self.errors.len());
            html.push_str("</section>\n");
        }

        file_section(
            &mut html,
            "Alterados na origem",
            "Modo somente novos: a versão nova não foi copiada.",
            &self.changed_in_source,
        );
        file_section(
            &mut html,
            "Removidos na origem",
            "A cópia continua no destino.",
            &self.removed_from_source,
        );
        file_section(
            &mut html,
            "Atualizados",
            "A versão anterior do destino está na pasta .backup-leve/substituidos.",
            &self.updated,
        );
        file_section(&mut html, "Copiados", "", &self.copied);
        file_section(
            &mut html,
            "Registrados sem cópia",
            "Já estavam iguais no destino.",
            &self.adopted,
        );

        let _ = write!(
            html,
            "<footer>Backup Leve {} · terminado em {}</footer>\n</main>\n</body>\n</html>\n",
            self.version,
            escape(&format_rfc3339(&self.finished_at)),
        );
        html
    }
}

pub fn conflict_label(kind: ConflictKind) -> &'static str {
    match kind {
        ConflictKind::ExistsWithDifferentContent => "existe no destino com conteúdo diferente",
        ConflictKind::ChangedInDestination => "alterado no destino",
        ConflictKind::ChangedInBoth => "alterado nos dois lados",
        ConflictKind::DeletedInDestination => "apagado no destino",
    }
}

pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::AddOnly => "somente novos",
        Mode::Sync => "sincronizar",
    }
}

/// `1536` → `1,5 KB`.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit]).replace('.', ",")
    }
}

pub fn format_mtime(mtime_ms: i64) -> String {
    DateTime::from_timestamp_millis(mtime_ms)
        .map(|time| {
            time.with_timezone(&Local)
                .format("%d/%m/%Y %H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
}

fn display_absolute(path: &Path) -> String {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn format_rfc3339(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.format("%d/%m/%Y %H:%M:%S").to_string())
        .unwrap_or_else(|_| value.to_owned())
}

fn snapshot_cell(snapshot: Option<Snapshot>) -> String {
    match snapshot {
        Some(s) => format!("{} · {}", format_bytes(s.size), format_mtime(s.mtime_ms)),
        None => "—".into(),
    }
}

fn file_section(html: &mut String, title: &str, note: &str, items: &[FileItem]) {
    if items.is_empty() {
        return;
    }
    let _ = write!(html, "<section><h2>{title}</h2>");
    if !note.is_empty() {
        let _ = write!(html, "<p class=\"note\">{note}</p>");
    }
    html.push_str(
        "<table><thead><tr><th>Arquivo</th><th class=\"num\">Tamanho</th></tr></thead><tbody>\n",
    );
    for item in items.iter().take(HTML_ROW_LIMIT) {
        let _ = writeln!(
            html,
            "<tr><td>{}</td><td class=\"num\">{}</td></tr>",
            escape(&item.path),
            format_bytes(item.size)
        );
    }
    html.push_str("</tbody></table>");
    limit_note(html, items.len());
    html.push_str("</section>\n");
}

fn limit_note(html: &mut String, total: usize) {
    if total > HTML_ROW_LIMIT {
        let _ = write!(
            html,
            "<p class=\"note\">Mostrando {HTML_ROW_LIMIT} de {total}. A lista completa está no arquivo .json.</p>"
        );
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const STYLE: &str = "
:root { --bg: #f6f5f1; --panel: #ffffff; --text: #1f2328; --muted: #5f6670; --line: #e3e1da;
        --accent: #2f6f5e; --alert: #b4472f; --alert-bg: #fbeee9; }
@media (prefers-color-scheme: dark) {
  :root { --bg: #16181b; --panel: #1f2226; --text: #e8e6e1; --muted: #9aa0a8; --line: #30343a;
          --accent: #7cc4ad; --alert: #f08e74; --alert-bg: #3a2420; }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--text);
       font: 15px/1.5 system-ui, -apple-system, 'Segoe UI', sans-serif; }
main { max-width: 1040px; margin: 0 auto; padding: 32px 16px 48px; }
h1 { margin: 0 0 4px; font-size: 26px; letter-spacing: -0.01em; }
h2 { margin: 0 0 4px; font-size: 18px; }
.route { margin: 0; font-family: ui-monospace, Consolas, monospace; font-size: 13px; word-break: break-all; }
.meta, .note, footer { color: var(--muted); font-size: 13px; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 10px; margin: 24px 0; }
.card { background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 12px 14px; }
.card span { display: block; color: var(--muted); font-size: 12px; }
.card strong { font-size: 20px; font-variant-numeric: tabular-nums; }
.card.alert { border-color: var(--alert); background: var(--alert-bg); }
.card.alert strong { color: var(--alert); }
section { margin: 28px 0; }
table { width: 100%; border-collapse: collapse; background: var(--panel); border: 1px solid var(--line);
        border-radius: 10px; overflow: hidden; font-size: 13px; }
th, td { text-align: left; padding: 7px 10px; border-bottom: 1px solid var(--line); vertical-align: top; }
th { color: var(--muted); font-weight: 600; }
td:first-child { word-break: break-all; }
.num { text-align: right; white-space: nowrap; font-variant-numeric: tabular-nums; }
tr:last-child td { border-bottom: 0; }
footer { margin-top: 40px; border-top: 1px solid var(--line); padding-top: 12px; }
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_in_portuguese() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1,5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5,0 GB");
    }

    #[test]
    fn escapes_file_names_in_html() {
        let mut report = Report::new(
            Path::new("origem"),
            Path::new("destino"),
            Mode::Sync,
            false,
            Local::now(),
        );
        report.copied.push(FileItem::new("<script>.txt", 1));
        let html = report.to_html();
        assert!(html.contains("&lt;script&gt;.txt"));
        assert!(!html.contains("<script>.txt"));
    }

    #[test]
    fn copied_card_counts_only_copied_files() {
        let mut report = Report::new(
            Path::new("origem"),
            Path::new("destino"),
            Mode::Sync,
            false,
            Local::now(),
        );
        report.updated.push(FileItem::new("a.txt", 30));
        report.bytes_copied = 30;
        assert!(report.to_html().contains("<strong>0 (0 B)</strong>"));
    }
}
