// Bridge to the Rust commands. Outside Tauri (plain `npm run dev` in a browser),
// it uses the sample data in mock.ts, so the screens can be designed without a backup disk.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import * as mock from "./mock";
import type {
  Decision,
  Outcome,
  Profile,
  Progress,
  RebuildableRule,
  ReportEntry,
  TargetResult,
} from "./types";

const inTauri = "__TAURI_INTERNALS__" in window;

export const api = {
  /** True outside Tauri, when the screens run on sample data. */
  isPreview: !inTauri,

  listProfiles: (): Promise<Profile[]> =>
    inTauri ? invoke("list_profiles") : mock.listProfiles(),

  saveProfiles: (profiles: Profile[]): Promise<void> =>
    inTauri ? invoke("save_profiles", { profiles }) : mock.saveProfiles(profiles),

  runProfile: (name: string, dryRun: boolean): Promise<TargetResult[]> =>
    inTauri ? invoke("run_profile", { name, dryRun }) : mock.runProfile(dryRun),

  cancelRun: (): Promise<void> => (inTauri ? invoke("cancel_run") : mock.cancelRun()),

  resolveConflicts: (
    source: string,
    destination: string,
    decisions: Decision[],
  ): Promise<Outcome[]> =>
    inTauri
      ? invoke("resolve_conflicts", { source, destination, decisions })
      : mock.resolveConflicts(decisions),

  listReports: (name: string): Promise<ReportEntry[]> =>
    inTauri ? invoke("list_reports", { name }) : mock.listReports(),

  openReport: (path: string): Promise<void> =>
    inTauri ? invoke("open_report", { path }) : Promise.resolve(),

  openFolder: (path: string): Promise<void> =>
    inTauri ? invoke("open_folder", { path }) : Promise.resolve(),

  rebuildableRules: (): Promise<RebuildableRule[]> =>
    inTauri ? invoke("rebuildable_rules") : mock.rebuildableRules(),

  folderExists: (path: string): Promise<boolean> =>
    inTauri ? invoke("folder_exists", { path }) : Promise.resolve(!path.startsWith("F:")),

  pickFolders: async (multiple: boolean): Promise<string[]> => {
    if (!inTauri) return mock.pickFolders(multiple);
    const picked = await open({ directory: true, multiple });
    if (!picked) return [];
    return Array.isArray(picked) ? picked : [picked];
  },

  onProgress: async (handler: (progress: Progress) => void): Promise<() => void> =>
    inTauri
      ? listen<Progress>("progresso", (event) => handler(event.payload))
      : mock.onProgress(handler),
};

export function errorText(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
}
