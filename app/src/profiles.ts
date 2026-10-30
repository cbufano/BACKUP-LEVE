// Profile rules mirrored from crates/engine/src/profile.rs, to warn while the user edits.

import type { Profile } from "./types";

/** Destination subfolder name: last folder name, or `Disco D` for a drive root. */
export function folderLabel(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const drive = /^([A-Za-z]):$/.exec(trimmed);
  if (drive) return `Disco ${drive[1].toUpperCase()}`;
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || "raiz";
}

export function destinationFor(profile: Profile, source: string): string {
  const separator = profile.destino.includes("/") && !profile.destino.includes("\\") ? "/" : "\\";
  return `${profile.destino.replace(/[\\/]+$/, "")}${separator}${folderLabel(source)}`;
}

/** Drive root of a Windows path (`E:\`), or the path itself elsewhere. */
export function driveOf(path: string): string {
  const drive = /^([A-Za-z]:)[\\/]?/.exec(path);
  return drive ? `${drive[1]}\\` : path;
}

/** Problems of `all[index]`. A repeated name is a problem only for the later profile. */
export function profileProblems(all: Profile[], index: number): string[] {
  const profile = all[index];
  const problems: string[] = [];
  if (!profile.nome.trim()) problems.push("Dê um nome ao perfil.");
  const sameNameBefore = all
    .slice(0, index)
    .some((other) => other.nome.trim().toLowerCase() === profile.nome.trim().toLowerCase());
  if (profile.nome.trim() && sameNameBefore) {
    problems.push(`Já existe outro perfil com o nome "${profile.nome}".`);
  }
  if (!profile.destino) problems.push("Escolha o disco de destino.");
  if (profile.origens.length === 0) problems.push("Adicione pelo menos uma pasta.");
  const labels = new Set<string>();
  for (const source of profile.origens) {
    const label = folderLabel(source).toLowerCase();
    if (labels.has(label)) {
      problems.push(
        `Duas pastas têm o nome "${folderLabel(source)}". No destino, uma ocuparia o lugar da outra.`,
      );
    }
    labels.add(label);
  }
  return problems;
}

export function newProfileName(all: Profile[]): string {
  let name = "Novo perfil";
  for (let n = 2; all.some((p) => p.nome === name); n += 1) name = `Novo perfil ${n}`;
  return name;
}
