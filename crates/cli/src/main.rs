//! `backup-leve` command line.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use backup_leve_engine::decision::Mode;
use backup_leve_engine::report::{conflict_label, format_bytes, mode_label};
use backup_leve_engine::{run, Event, Options, Report};
use clap::{Parser, Subcommand, ValueEnum};

/// Lines of conflicts and errors shown in the terminal. The report has all of them.
const TERMINAL_LIMIT: usize = 20;

const EXIT_FATAL: u8 = 1;
const EXIT_CONFLICTS: u8 = 3;
const EXIT_FILE_ERRORS: u8 = 4;

#[derive(Parser)]
#[command(
    name = "backup-leve",
    version,
    about = "Backup de pastas para um disco de destino.",
    after_help = "Códigos de saída: 0 = tudo certo, 1 = execução interrompida, \
                  3 = há conflitos para decidir, 4 = alguns arquivos não foram copiados."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Copia a origem para o destino e lista os conflitos.
    Sincronizar {
        /// Pasta que recebe o backup.
        #[arg(long)]
        origem: PathBuf,
        /// Pasta no disco de backup.
        #[arg(long)]
        destino: PathBuf,
        /// O que fazer com arquivos alterados na origem.
        #[arg(long, value_enum, default_value_t = Modo::Sincronizar)]
        modo: Modo,
        /// Mostra o que seria feito, sem copiar nada.
        #[arg(long)]
        simulacao: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Modo {
    /// Copia só arquivos novos.
    SomenteNovos,
    /// Copia arquivos novos e atualiza os alterados na origem.
    Sincronizar,
}

impl From<Modo> for Mode {
    fn from(modo: Modo) -> Self {
        match modo {
            Modo::SomenteNovos => Mode::AddOnly,
            Modo::Sincronizar => Mode::Sync,
        }
    }
}

fn main() -> ExitCode {
    let Command::Sincronizar {
        origem,
        destino,
        modo,
        simulacao,
    } = Cli::parse().command;
    let options = Options {
        mode: modo.into(),
        dry_run: simulacao,
    };

    println!("Backup Leve {}", env!("CARGO_PKG_VERSION"));
    println!("Origem:  {}", origem.display());
    println!("Destino: {}", destino.display());
    println!("Modo:    {}", mode_label(options.mode));
    if simulacao {
        println!("Simulação: nenhum arquivo será alterado.");
    }
    println!();

    let mut progress = Progress::new();
    match run(&origem, &destino, &options, |event| progress.show(event)) {
        Ok(report) => {
            progress.finish();
            print_summary(&report);
            exit_code(&report)
        }
        Err(err) => {
            progress.finish();
            eprintln!("Erro: {err}");
            ExitCode::from(EXIT_FATAL)
        }
    }
}

struct Progress {
    enabled: bool,
    total_files: u64,
    total_bytes: u64,
    last_draw: Option<Instant>,
    drawn: bool,
}

impl Progress {
    fn new() -> Self {
        Self {
            enabled: std::io::stderr().is_terminal(),
            total_files: 0,
            total_bytes: 0,
            last_draw: None,
            drawn: false,
        }
    }

    fn show(&mut self, event: Event<'_>) {
        match event {
            Event::Scanning => println!("Lendo as pastas..."),
            Event::Planned { files, bytes } => {
                self.total_files = files;
                self.total_bytes = bytes;
                println!("A copiar: {files} arquivos ({})", format_bytes(bytes));
            }
            Event::FileDone {
                done_files,
                done_bytes,
                ..
            } => {
                let due = self
                    .last_draw
                    .is_none_or(|last| last.elapsed() >= Duration::from_millis(200));
                if !self.enabled || (!due && done_files < self.total_files) {
                    return;
                }
                let percent = (done_bytes * 100)
                    .checked_div(self.total_bytes)
                    .unwrap_or(100);
                eprint!(
                    "\r  {done_files}/{} arquivos · {} de {} · {percent}%   ",
                    self.total_files,
                    format_bytes(done_bytes),
                    format_bytes(self.total_bytes),
                );
                let _ = std::io::stderr().flush();
                self.last_draw = Some(Instant::now());
                self.drawn = true;
            }
        }
    }

    fn finish(&mut self) {
        if self.drawn {
            eprintln!();
            self.drawn = false;
        }
    }
}

fn print_summary(report: &Report) {
    let (copied, updated, adopted) = if report.dry_run {
        (
            "Seriam copiados",
            "Seriam atualizados",
            "Seriam registrados",
        )
    } else {
        ("Copiados", "Atualizados", "Registrados sem cópia")
    };
    println!();
    println!("Resultado");
    println!(
        "  {copied:<24}{} ({})",
        report.copied.len(),
        format_bytes(report.copied.iter().map(|i| i.size).sum())
    );
    for (label, value) in [
        (updated, report.updated.len() as u64),
        (adopted, report.adopted.len() as u64),
        ("Sem mudança", report.unchanged),
        ("Alterados na origem", report.changed_in_source.len() as u64),
        (
            "Removidos na origem",
            report.removed_from_source.len() as u64,
        ),
        ("Conflitos", report.conflicts.len() as u64),
        ("Erros", report.errors.len() as u64),
    ] {
        println!("  {label:<24}{value}");
    }

    if !report.conflicts.is_empty() {
        println!();
        println!("Conflitos (o programa não alterou estes arquivos):");
        for item in report.conflicts.iter().take(TERMINAL_LIMIT) {
            println!("  {:<42}{}", conflict_label(item.kind), item.path);
        }
        more(report.conflicts.len());
    }
    if !report.errors.is_empty() {
        println!();
        println!("Erros (a próxima execução tenta de novo):");
        for item in report.errors.iter().take(TERMINAL_LIMIT) {
            println!("  {}: {}", item.path, item.message);
        }
        more(report.errors.len());
    }
    if let Some(path) = &report.saved_to {
        println!();
        println!("Relatório: {}", path.display());
    }
}

fn more(total: usize) {
    if total > TERMINAL_LIMIT {
        println!("  ... e mais {} (veja o relatório)", total - TERMINAL_LIMIT);
    }
}

fn exit_code(report: &Report) -> ExitCode {
    if !report.errors.is_empty() {
        ExitCode::from(EXIT_FILE_ERRORS)
    } else if !report.conflicts.is_empty() {
        ExitCode::from(EXIT_CONFLICTS)
    } else {
        ExitCode::SUCCESS
    }
}
