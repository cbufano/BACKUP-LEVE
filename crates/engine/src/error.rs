//! Errors that stop a whole run. Errors on a single file go to the report instead.

use std::path::PathBuf;

/// Messages are in Portuguese because the user reads them directly.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("a pasta de origem não existe: {0}")]
    SourceNotFound(PathBuf),

    #[error("o disco de destino não está conectado: {0}")]
    DestinationUnavailable(PathBuf),

    #[error("a origem e o destino não podem ficar um dentro do outro: {0} e {1}")]
    NestedFolders(PathBuf, PathBuf),

    #[error("este destino já recebe o backup de outra origem: {0}")]
    IndexBelongsToOtherSource(String),

    #[error("o índice foi criado por uma versão mais nova do Backup Leve (esquema {0})")]
    UnsupportedIndexVersion(i64),

    #[error("erro de disco: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro no índice: {0}")]
    Index(#[from] rusqlite::Error),

    #[error("erro ao gravar o relatório: {0}")]
    Report(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
