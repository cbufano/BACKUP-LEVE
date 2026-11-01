// Shapes sent by the Rust side (app/src-tauri/src/lib.rs and crates/engine).

export type Mode = "sincronizar" | "somente-novos";

export interface Profile {
  nome: string;
  destino: string;
  origens: string[];
  modo: Mode;
  /** Leave out the folders that a tool creates again (node_modules, target...). */
  pular_recriaveis: boolean;
  /** Leave out temporary files (*.tmp, ~$*...). */
  pular_temporarios: boolean;
  /** Names of files and folders that stay out. `*.tmp` matches an ending. */
  pular: string[];
}

/** A folder that a tool creates again. See crates/engine/src/exclude.rs. */
export interface RebuildableRule {
  pasta: string;
  /** The parent folder needs one file of each group. Empty: always. */
  precisa: string[][];
  quem_recria: string;
}

export interface LeftOutItem {
  path: string;
  reason: string;
}

export interface Snapshot {
  size: number;
  mtime_ms: number;
}

export type ConflictKind =
  | "exists_with_different_content"
  | "changed_in_destination"
  | "changed_in_both"
  | "deleted_in_destination";

export interface FileItem {
  path: string;
  size: number;
}

export interface ConflictItem {
  path: string;
  kind: ConflictKind;
  source: Snapshot | null;
  destination: Snapshot | null;
}

export interface Report {
  started_at: string;
  finished_at: string;
  source: string;
  destination: string;
  mode: Mode;
  dry_run: boolean;
  cancelled: boolean;
  copied: FileItem[];
  updated: FileItem[];
  adopted: FileItem[];
  changed_in_source: FileItem[];
  removed_from_source: FileItem[];
  conflicts: ConflictItem[];
  errors: { path: string; message: string }[];
  left_out: LeftOutItem[];
  left_out_forgotten: number;
  unchanged: number;
  forgotten: number;
  partials_removed: number;
  bytes_copied: number;
}

export interface TargetResult {
  label: string;
  source: string;
  destination: string;
  report: Report | null;
  reportPath: string | null;
  error: string | null;
}

export interface Progress {
  target: number;
  targets: number;
  label: string;
  stage: "lendo" | "copiando";
  /** Folder being read. Empty while copying. */
  side: "origem" | "destino" | "";
  /** Files found so far in each folder. */
  sourceFound: number;
  destinationFound: number;
  /** Files the index expects in the destination. Zero when unknown. */
  destinationExpected: number;
  /** Files already in the backup and unchanged: not copied again. */
  alreadyDone: number;
  filesDone: number;
  filesTotal: number;
  bytesDone: number;
  bytesTotal: number;
  /** File being read or copied now. */
  path: string;
  /** Files finished since the previous event, oldest first. */
  recent: FileItem[];
}

export type Choice = "source_wins" | "destination_wins" | "keep_both" | "delete_in_source";

export interface Decision {
  path: string;
  choice: Choice;
  source: Snapshot | null;
  destination: Snapshot | null;
}

export interface Outcome {
  path: string;
  choice: Choice;
  ok: boolean;
  message: string;
}

export interface ReportEntry {
  label: string;
  path: string;
  stamp: string;
}

export type Provider = "nenhum" | "openai" | "grok" | "anthropic";

export interface ProviderKey {
  chave: string;
  /** Empty: the default model of the service. */
  modelo: string;
}

/** Program settings. See crates/engine/src/settings.rs. */
export interface Settings {
  provedor: Provider;
  openai: ProviderKey;
  grok: ProviderKey;
  anthropic: ProviderKey;
  novo_perfil_pular_recriaveis: boolean;
  novo_perfil_pular_temporarios: boolean;
}

/** A name the AI suggests to skip. The user decides. */
export interface Suggestion {
  nome: string;
  motivo: string;
}
