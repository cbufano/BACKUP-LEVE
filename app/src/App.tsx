import { useCallback, useEffect, useState } from "react";
import { api, errorText } from "./api";
import { driveOf, newProfileName, profileProblems } from "./profiles";
import { ProfileView } from "./ProfileView";
import { ReportsView } from "./ReportsView";
import { ResultView } from "./ResultView";
import { RunView } from "./RunView";
import { SettingsView } from "./SettingsView";
import type { Profile, Progress, Settings, TargetResult } from "./types";

type Screen =
  | { kind: "perfil" }
  | { kind: "executando"; dryRun: boolean; startedAt: number }
  | { kind: "resultado"; dryRun: boolean; results: TargetResult[]; startedAt: number; finishedAt: number }
  | { kind: "relatorios" }
  | { kind: "configuracoes" };

export default function App() {
  const [profiles, setProfiles] = useState<Profile[] | null>(null);
  const [current, setCurrent] = useState(0);
  const [screen, setScreen] = useState<Screen>({ kind: "perfil" });
  const [progress, setProgress] = useState<Progress | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [connected, setConnected] = useState<Record<string, boolean>>({});
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    api.getSettings().then(setSettings, () => setSettings(null));
  }, []);

  useEffect(() => {
    api.listProfiles().then(setProfiles, (error) => {
      setProfiles([]);
      setNotice(`Os perfis não foram lidos: ${errorText(error)}`);
    });
  }, []);

  useEffect(() => {
    let active = true;
    let off: (() => void) | undefined;
    api
      .onProgress((next) => active && setProgress(next))
      .then((unlisten) => (active ? (off = unlisten) : unlisten()));
    return () => {
      active = false;
      off?.();
    };
  }, []);

  // A disk can be plugged in while the window is open: check again on focus.
  const refreshDrives = useCallback(async (list: Profile[]) => {
    const entries = await Promise.all(
      list
        .filter((profile) => profile.destino)
        .map(async (profile) => [profile.destino, await api.folderExists(driveOf(profile.destino))] as const),
    );
    setConnected(Object.fromEntries(entries));
  }, []);

  useEffect(() => {
    if (!profiles) return;
    refreshDrives(profiles);
    const onFocus = () => refreshDrives(profiles);
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [profiles, refreshDrives]);

  /** Shows the list at once and saves the profiles that are complete. */
  const persist = async (list: Profile[]) => {
    setProfiles(list);
    const complete = list.filter((_, index) => profileProblems(list, index).length === 0);
    try {
      await api.saveProfiles(complete);
      setNotice(null);
    } catch (error) {
      setNotice(`Os perfis não foram salvos: ${errorText(error)}`);
    }
  };

  const addProfile = () => {
    if (!profiles) return;
    const next = [...profiles, {
        nome: newProfileName(profiles),
        destino: "",
        origens: [],
        modo: "sincronizar" as const,
        pular_recriaveis: settings?.novo_perfil_pular_recriaveis ?? true,
        pular_temporarios: settings?.novo_perfil_pular_temporarios ?? true,
        pular: [],
      }];
    setCurrent(next.length - 1);
    setScreen({ kind: "perfil" });
    persist(next);
  };

  const run = async (dryRun: boolean) => {
    if (!profiles) return;
    const profile = profiles[current];
    const startedAt = Date.now();
    setProgress(null);
    setScreen({ kind: "executando", dryRun, startedAt });
    try {
      const results = await api.runProfile(profile.nome, dryRun);
      setScreen({ kind: "resultado", dryRun, results, startedAt, finishedAt: Date.now() });
    } catch (error) {
      setNotice(errorText(error));
      setScreen({ kind: "perfil" });
    }
  };

  // Browser preview only (`npm run dev`, sample data): `#executando` starts a sample run.
  useEffect(() => {
    if (import.meta.env.DEV && api.isPreview && profiles && window.location.hash === "#executando") {
      window.location.hash = "";
      run(false);
    }
  }, [profiles]); // eslint-disable-line react-hooks/exhaustive-deps

  const busy = screen.kind === "executando";
  const profile = profiles?.[current];

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand-row">
          <div className="brand">
            <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
              <rect x="3" y="5" width="18" height="14" rx="3" fill="none" stroke="currentColor" strokeWidth="1.8" />
              <rect x="6" y="8" width="8" height="4" rx="1" fill="currentColor" />
              <circle cx="17" cy="15" r="1.3" fill="currentColor" />
            </svg>
            <span>Backup Leve</span>
          </div>
          <button
            className={`gear ${screen.kind === "configuracoes" ? "active" : ""}`}
            title="Configurações"
            aria-label="Configurações"
            disabled={busy}
            onClick={() => setScreen({ kind: "configuracoes" })}
          >
            <svg
              viewBox="0 0 24 24"
              width="20"
              height="20"
              aria-hidden="true"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
        </div>
        <nav aria-label="Perfis">
          <ul className="profile-list">
            {profiles?.map((item, index) => (
              <li key={index}>
                <button
                  className={index === current ? "active" : ""}
                  aria-current={index === current ? "page" : undefined}
                  disabled={busy}
                  onClick={() => {
                    setCurrent(index);
                    setScreen({ kind: "perfil" });
                  }}
                >
                  <span className="profile-list-name">{item.nome || "Sem nome"}</span>
                  <span className="profile-list-destination">
                    {item.destino && (
                      <span className={`status-dot ${connected[item.destino] ? "on" : "off"}`} aria-hidden="true" />
                    )}
                    {item.destino || "sem destino"}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </nav>
        <button className="button new-profile" onClick={addProfile} disabled={busy || !profiles}>
          Novo perfil
        </button>
      </aside>

      <main className="main">
        {notice && (
          <div className="notice error banner" role="alert">
            <span>{notice}</span>
            <button className="button quiet" onClick={() => setNotice(null)}>
              Fechar
            </button>
          </div>
        )}

        {screen.kind === "configuracoes" && (
          <SettingsView onBack={() => setScreen({ kind: "perfil" })} onSaved={setSettings} />
        )}

        {profiles && profiles.length === 0 && screen.kind !== "configuracoes" && (
          <div className="welcome">
            <h1>Nenhum perfil ainda</h1>
            <p className="lead">
              Um perfil diz quais pastas copiar e para qual disco. Depois, basta abrir o programa e clicar em
              Sincronizar.
            </p>
            <button className="button primary" onClick={addProfile}>
              Criar perfil
            </button>
          </div>
        )}

        {profile && profiles && screen.kind === "perfil" && (
          <ProfileView
            profile={profile}
            problems={profileProblems(profiles, current)}
            connected={profile.destino ? (connected[profile.destino] ?? null) : null}
            onChange={(next) => persist(profiles.map((item, index) => (index === current ? next : item)))}
            onDelete={() => {
              setCurrent(0);
              persist(profiles.filter((_, index) => index !== current));
            }}
            onRun={run}
            onReports={() => setScreen({ kind: "relatorios" })}
          />
        )}
        {profile && screen.kind === "executando" && (
          <RunView
            key={screen.startedAt}
            profileName={profile.nome}
            dryRun={screen.dryRun}
            progress={progress}
            startedAt={screen.startedAt}
          />
        )}
        {profile && screen.kind === "resultado" && (
          <ResultView
            profileName={profile.nome}
            dryRun={screen.dryRun}
            results={screen.results}
            startedAt={screen.startedAt}
            finishedAt={screen.finishedAt}
            onBack={() => setScreen({ kind: "perfil" })}
          />
        )}
        {profile && screen.kind === "relatorios" && (
          <ReportsView profileName={profile.nome} onBack={() => setScreen({ kind: "perfil" })} />
        )}
      </main>
    </div>
  );
}
