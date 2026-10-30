// Portuguese labels and number formats shared by the screens.

import type { Choice, ConflictKind, Snapshot } from "./types";

const number = new Intl.NumberFormat("pt-BR");
const date = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short", timeStyle: "short" });

export function formatCount(value: number): string {
  return number.format(value);
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0
    ? `${bytes} B`
    : `${value.toLocaleString("pt-BR", { maximumFractionDigits: 1, minimumFractionDigits: 1 })} ${units[unit]}`;
}

export function formatDate(ms: number): string {
  return date.format(new Date(ms));
}

const clock = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit", second: "2-digit" });

/** `11:22:05`. */
export function formatClock(ms: number): string {
  return clock.format(new Date(ms));
}

/** `45 s`, `2 min 05 s`, `1 h 07 min`. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours) return `${hours} h ${String(minutes).padStart(2, "0")} min`;
  if (minutes) return `${minutes} min ${String(seconds).padStart(2, "0")} s`;
  return `${seconds} s`;
}

/** `00:00:45`, `00:44:06`, `01:07:30`. Fits a narrow column. */
export function formatTimer(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const parts = [Math.floor(total / 3600), Math.floor((total % 3600) / 60), total % 60];
  return parts.map((part) => String(part).padStart(2, "0")).join(":");
}

export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(Math.round(bytesPerSecond))}/s`;
}

/** `2026-10-29_10-43-07` → `29/10/2026 10:43`. */
export function formatStamp(stamp: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})/.exec(stamp);
  return match ? `${match[3]}/${match[2]}/${match[1]} ${match[4]}:${match[5]}` : stamp;
}

export function formatSnapshot(snapshot: Snapshot | null): string {
  return snapshot ? `${formatBytes(snapshot.size)}, ${formatDate(snapshot.mtime_ms)}` : "não existe";
}

export const conflictLabel: Record<ConflictKind, string> = {
  exists_with_different_content: "Já existia no destino, com outro conteúdo",
  changed_in_destination: "Alterado no destino",
  changed_in_both: "Alterado nos dois lados",
  deleted_in_destination: "Apagado no destino",
};

/** Options valid for each conflict, in display order. */
export function choicesFor(kind: ConflictKind): { choice: Choice; label: string }[] {
  if (kind === "deleted_in_destination") {
    return [
      { choice: "source_wins", label: "Copiar de novo" },
      { choice: "delete_in_source", label: "Apagar na origem" },
    ];
  }
  return [
    { choice: "source_wins", label: "Origem vence" },
    { choice: "destination_wins", label: "Destino vence" },
    { choice: "keep_both", label: "Manter os dois" },
  ];
}
