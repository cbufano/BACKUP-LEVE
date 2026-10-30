import { useCallback, useEffect, useState } from "react";
import { api, errorText } from "./api";
import { driveOf, newProfileName, profileProblems } from "./profiles";
import { ProfileView } from "./ProfileView";
import { ReportsView } from "./ReportsView";
import { ResultView } from "./ResultView";
import { RunView } from "./RunView";
import type { Profile, Progress, TargetResult } from "./types";

type Screen =
  | { kind: "perfil" }
  | { kind: "executando"; dryRun: boolean; startedAt: number }
  | { kind: "resultado"; dryRun: boolean; results: TargetResult[]; startedAt: number; finishedAt: number }
  | { kind: "relatorios" };

export default function App() {
  const [profiles, setProfiles] = useState<Profile[] | null>(null);
  const [current, setCurrent] = useState(0);
  const [screen, setScreen] = useState<Screen>({ kind: "perfil" });
  const [progress, setProgress] = useState<Progress | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [connected, setConnected] = useState<Record<string, boolean>>({});

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
        pular_recriaveis: true,
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
        <div className="brand">
          <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
            <rect x="3" y="5" width="18" height="14" rx="3" fill="none" stroke="currentColor" strokeWidth="1.8" />
            <rect x="6" y="8" width="8" height="4" rx="1" fill="currentColor" />
            <circle cx="17" cy="15" r="1.3" fill="currentColor" />
          </svg>
          <span>Backup Leve</span>
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

        {profiles && profiles.length === 0 && (
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
