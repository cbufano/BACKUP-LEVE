// Sample data for designing the screens in a browser, without Tauri.

import type {
  Decision,
  Outcome,
  Profile,
  Progress,
  RebuildableRule,
  Report,
  ReportEntry,
  Settings,
  Suggestion,
  TargetResult,
} from "./types";

let profiles: Profile[] = [
  {
    nome: "Trabalho",
    destino: "E:\\Backup",
    origens: ["D:\\Documentos", "D:\\Projetos\\Clientes"],
    modo: "sincronizar",
    pular_recriaveis: true,
    pular_temporarios: true,
    pular: ["*.tmp"],
  },
  {
    nome: "Fotos da família",
    destino: "F:\\Fotos",
    origens: ["D:\\Fotos"],
    modo: "somente-novos",
    pular_recriaveis: true,
    pular_temporarios: true,
    pular: [],
  },
];

const JS = ["package.json"];
const LOCK = ["package-lock.json", "yarn.lock", "pnpm-lock.yaml", "bun.lockb", "bun.lock"];

export async function rebuildableRules(): Promise<RebuildableRule[]> {
  return [
    { pasta: "node_modules", precisa: [JS], quem_recria: "npm, yarn ou pnpm (JavaScript)" },
    { pasta: ".next", precisa: [JS], quem_recria: "Next.js" },
    { pasta: "dist", precisa: [JS, LOCK], quem_recria: "o build do projeto JavaScript" },
    { pasta: "build", precisa: [JS, LOCK], quem_recria: "o build do projeto JavaScript" },
    { pasta: "__pycache__", precisa: [], quem_recria: "Python" },
    { pasta: ".venv", precisa: [["requirements.txt", "pyproject.toml", "setup.py", "pipfile"]], quem_recria: "pip, a partir da lista de pacotes" },
    { pasta: "target", precisa: [["cargo.toml"]], quem_recria: "cargo (Rust)" },
    { pasta: "build", precisa: [["build.gradle", "build.gradle.kts"]], quem_recria: "Gradle (Android)" },
    { pasta: "bin", precisa: [["*.csproj", "*.vbproj", "*.fsproj"]], quem_recria: "Visual Studio (.NET)" },
  ];
}

export async function temporaryNames(): Promise<string[]> {
  return ["*.tmp", "*.temp", "~$*", "*.crdownload", "*.swp", "*.swo", "*.pyc", "*.pyo", ".ds_store"];
}

let settings: Settings = {
  provedor: "nenhum",
  openai: { chave: "", modelo: "" },
  grok: { chave: "", modelo: "" },
  anthropic: { chave: "", modelo: "" },
  novo_perfil_pular_recriaveis: true,
  novo_perfil_pular_temporarios: true,
};

export async function getSettings(): Promise<Settings> {
  return structuredClone(settings);
}

export async function saveSettings(next: Settings): Promise<void> {
  settings = structuredClone(next);
}

export async function testAi(next: Settings): Promise<string> {
  await new Promise((done) => setTimeout(done, 600));
  if (next.provedor === "nenhum" || !next[next.provedor].chave.trim()) {
    throw "escolha um serviço de IA e escreva a chave";
  }
  return "OK";
}

export async function suggestSkips(): Promise<Suggestion[]> {
  await new Promise((done) => setTimeout(done, 1200));
  if (settings.provedor === "nenhum") throw "configure um serviço de IA na engrenagem, no alto da tela";
  return [
    { nome: "Library", motivo: "Cache do Unity. O editor cria a pasta de novo ao abrir o projeto." },
    { nome: "*.pdb", motivo: "Símbolos de depuração gerados pelo compilador." },
    { nome: ".cache", motivo: "Cache de ferramentas. É criado de novo quando a ferramenta roda." },
  ];
}

const listeners = new Set<(progress: Progress) => void>();
let cancelled = false;

export async function listProfiles(): Promise<Profile[]> {
  return structuredClone(profiles);
}

export async function saveProfiles(next: Profile[]): Promise<void> {
  profiles = structuredClone(next);
}

export async function pickFolders(multiple: boolean): Promise<string[]> {
  return multiple ? ["D:\\Música", "D:\\Planilhas"] : ["G:\\Backup novo"];
}

export async function onProgress(handler: (progress: Progress) => void): Promise<() => void> {
  listeners.add(handler);
  return () => listeners.delete(handler);
}

export async function cancelRun(): Promise<void> {
  cancelled = true;
}

export async function runProfile(dryRun: boolean): Promise<TargetResult[]> {
  cancelled = false;
  const found = 12_480;
  for (let step = 0; step <= 40 && !cancelled; step++) {
    const side = step <= 20 ? "origem" : "destino";
    const sourceFound = Math.round((Math.min(step, 20) / 20) * found);
    const destinationFound = step > 20 ? Math.round(((step - 20) / 20) * (found - 1840)) : 0;
    listeners.forEach((listener) =>
      listener({
        target: 1,
        targets: 2,
        label: "Documentos",
        stage: "lendo",
        side,
        sourceFound,
        destinationFound,
        destinationExpected: found - 1840,
        alreadyDone: 0,
        filesDone: 0,
        filesTotal: 0,
        bytesDone: 0,
        bytesTotal: 0,
        path: `Projetos/2026/pasta_${step}/planilha_${step * 37}.xlsx`,
        recent: [],
      }),
    );
    await new Promise((resolve) => setTimeout(resolve, 80));
  }
  const total = 1840;
  for (let done = 46; done <= total && !cancelled; done += 46) {
    const progress: Progress = {
      target: 1,
      targets: 2,
      label: "Documentos",
      stage: "copiando",
      side: "",
      sourceFound: found,
      destinationFound: found - total,
      destinationExpected: found - total,
      alreadyDone: found - total,
      filesDone: done,
      filesTotal: total,
      bytesDone: done * 1_300_000,
      bytesTotal: total * 1_300_000,
      path: `Contratos/2026/contrato_${done}.pdf`,
      recent: Array.from({ length: 46 }, (_, i) => ({
        path: `Contratos/2026/contrato_${done - 46 + i + 1}.pdf`,
        size: 1_100_000 + ((done + i) % 7) * 90_000,
      })),
    };
    listeners.forEach((listener) => listener(progress));
    await new Promise((resolve) => setTimeout(resolve, 60));
  }
  const now = Date.now();
  const report: Report = {
    started_at: new Date(now - 42_000).toISOString(),
    finished_at: new Date(now).toISOString(),
    source: "D:\\Documentos",
    destination: "E:\\Backup\\Documentos",
    mode: "sincronizar",
    dry_run: dryRun,
    cancelled,
    copied: [
      { path: "Contratos/2026/contrato_novo.pdf", size: 1_204_331 },
      { path: "Planilhas/orçamento outubro.xlsx", size: 88_120 },
    ],
    updated: [{ path: "Notas/reunião.docx", size: 40_221 }],
    adopted: [],
    changed_in_source: [],
    removed_from_source: [{ path: "Rascunhos/antigo.txt", size: 1_200 }],
    conflicts: [
      {
        path: "Notas/lista de tarefas.docx",
        kind: "changed_in_destination",
        source: { size: 20_480, mtime_ms: now - 86_400_000 },
        destination: { size: 22_016, mtime_ms: now - 3_600_000 },
      },
      {
        path: "Contratos/2025/aditivo.pdf",
        kind: "changed_in_both",
        source: { size: 310_000, mtime_ms: now - 7_200_000 },
        destination: { size: 298_000, mtime_ms: now - 5_400_000 },
      },
      {
        path: "Fotos do escritório/fachada.jpg",
        kind: "deleted_in_destination",
        source: { size: 2_400_000, mtime_ms: now - 30 * 86_400_000 },
        destination: null,
      },
    ],
    errors: [],
    left_out: [
      { path: "Projetos/site-loja/node_modules", reason: "npm, yarn ou pnpm (JavaScript)" },
      { path: "Projetos/site-loja/dist", reason: "o build do projeto JavaScript" },
      { path: "Projetos/conversor/bin", reason: "Visual Studio (.NET)" },
    ],
    left_out_forgotten: 0,
    unchanged: 1837,
    forgotten: 0,
    partials_removed: 0,
    bytes_copied: 1_332_672,
  };
  return [
    {
      label: "Documentos",
      source: "D:\\Documentos",
      destination: "E:\\Backup\\Documentos",
      report,
      reportPath: "E:\\Backup\\Documentos\\.backup-leve\\relatorios\\relatorio.html",
      error: null,
    },
    {
      label: "Clientes",
      source: "D:\\Projetos\\Clientes",
      destination: "E:\\Backup\\Clientes",
      report: null,
      reportPath: null,
      error: "a pasta de origem não existe: D:\\Projetos\\Clientes",
    },
  ];
}

export async function resolveConflicts(decisions: Decision[]): Promise<Outcome[]> {
  return decisions.map((decision) => ({
    path: decision.path,
    choice: decision.choice,
    ok: true,
    message: "feito",
  }));
}

export async function listReports(): Promise<ReportEntry[]> {
  return [
    { label: "Documentos", path: "r1.html", stamp: "2026-10-29_10-43-07" },
    { label: "Clientes", path: "r2.html", stamp: "2026-10-28_19-02-11" },
    { label: "Documentos", path: "r3.html", stamp: "2026-10-28_19-01-40" },
  ];
}
