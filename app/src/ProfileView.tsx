import { useEffect, useState } from "react";
import { SkipSection } from "./SkipSection";
import { api } from "./api";
import { destinationFor, folderLabel } from "./profiles";
import type { Mode, Profile } from "./types";

interface Props {
  profile: Profile;
  problems: string[];
  connected: boolean | null;
  onChange: (profile: Profile) => void;
  onDelete: () => void;
  onRun: (dryRun: boolean) => void;
  onReports: () => void;
}

/** Remembers whether the "Pastas copiadas" frame is minimized. */
const FOLDERS_CLOSED_KEY = "pastas-copiadas-minimizado";

const modes:{ mode: Mode; title: string; text: string }[] = [
  {
    mode: "sincronizar",
    title: "Atualizar o backup",
    text: "A versão nova vai para o destino. A versão anterior fica guardada no próprio disco de backup.",
  },
  {
    mode: "somente-novos",
    title: "Copiar só arquivos novos",
    text: "O que já está no backup não muda. Os arquivos alterados aparecem no relatório.",
  },
];

export function ProfileView({ profile, problems, connected, onChange, onDelete, onRun, onReports }: Props) {
  const [name, setName] = useState(profile.nome);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [foldersClosed, setFoldersClosed] = useState(() => {
    try {
      return localStorage.getItem(FOLDERS_CLOSED_KEY) === "1";
    } catch {
      return false;
    }
  });
  const toggleFolders = (closed: boolean) => {
    setFoldersClosed(closed);
    try {
      localStorage.setItem(FOLDERS_CLOSED_KEY, closed ? "1" : "0");
    } catch {
      // The choice is only a convenience.
    }
  };
  useEffect(() => {
    setName(profile.nome);
    setConfirmDelete(false);
  }, [profile.nome]);

  const commitName = () => {
    if (name !== profile.nome) onChange({ ...profile, nome: name });
  };

  const pickDestination = async () => {
    const [folder] = await api.pickFolders(false);
    if (folder) onChange({ ...profile, destino: folder });
  };

  const addSources = async () => {
    const folders = await api.pickFolders(true);
    const fresh = folders.filter((folder) => !profile.origens.includes(folder));
    if (fresh.length) onChange({ ...profile, origens: [...profile.origens, ...fresh] });
  };

  const ready = problems.length === 0;

  return (
    <div className="profile">
      <div className="drive-label">
        <div className="drive-label-grip" aria-hidden="true" />
        <div className="drive-label-body">
          <label className="visually-hidden" htmlFor="profile-name">
            Nome do perfil
          </label>
          <input
            id="profile-name"
            className="drive-label-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            onBlur={commitName}
            onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
            spellCheck={false}
          />
          <div className="drive-label-destination">
            {profile.destino ? (
              <>
                <span className={`status-dot ${connected ? "on" : "off"}`} aria-hidden="true" />
                <span className="path">{profile.destino}</span>
                <span className="status-text">{connected ? "conectado" : "disco não encontrado"}</span>
              </>
            ) : (
              <span className="muted">Nenhum disco de destino escolhido</span>
            )}
          </div>
          <div className="drive-label-actions">
            <button className="button quiet" onClick={pickDestination}>
              {profile.destino ? "Trocar destino" : "Escolher destino"}
            </button>
            {profile.destino && connected && (
              <button className="button quiet" onClick={() => api.openFolder(profile.destino)}>
                Abrir destino
              </button>
            )}
          </div>
        </div>
      </div>

      <section className="block">
        <div className="block-head">
          <button
            className="collapse-toggle"
            aria-expanded={!foldersClosed}
            onClick={() => toggleFolders(!foldersClosed)}
            title={foldersClosed ? "Mostrar as pastas" : "Minimizar as pastas"}
          >
            <span className={`chevron ${foldersClosed ? "closed" : ""}`} aria-hidden="true">
              ▾
            </span>
            <h2>Pastas copiadas</h2>
            <span className="muted small">({profile.origens.length})</span>
          </button>
          <button className="button" onClick={addSources}>
            Adicionar pastas
          </button>
        </div>
        {foldersClosed ? null : profile.origens.length === 0 ? (
          <p className="empty">Nenhuma pasta ainda. Adicione as pastas que você quer copiar para o disco de backup.</p>
        ) : (
          <ul className="folders">
            {profile.origens.map((source) => (
              <li key={source}>
                <div className="folder-main">
                  <strong>{folderLabel(source)}</strong>
                  <span className="path">{source}</span>
                </div>
                {profile.destino && (
                  <span className="folder-goes">
                    vai para <span className="path">{destinationFor(profile, source)}</span>
                  </span>
                )}
                <button
                  className="button quiet"
                  onClick={() => onChange({ ...profile, origens: profile.origens.filter((s) => s !== source) })}
                  aria-label={`Remover ${source}`}
                >
                  Remover
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="block">
        <h2>Quando um arquivo muda na origem</h2>
        <div className="modes" role="radiogroup" aria-label="Quando um arquivo muda na origem">
          {modes.map(({ mode, title, text }) => (
            <label key={mode} className={`mode ${profile.modo === mode ? "selected" : ""}`}>
              <input
                type="radio"
                name="modo"
                checked={profile.modo === mode}
                onChange={() => onChange({ ...profile, modo: mode })}
              />
              <span className="mode-title">{title}</span>
              <span className="mode-text">{text}</span>
            </label>
          ))}
        </div>
      </section>

      <SkipSection profile={profile} onChange={onChange} />

      {!ready && (
        <div className="notice warn" role="status">
          <strong>Este perfil ainda não foi salvo.</strong>
          <ul>
            {problems.map((problem) => (
              <li key={problem}>{problem}</li>
            ))}
          </ul>
        </div>
      )}

      <footer className="actions">
        {confirmDelete ? (
          <span className="confirm">
            Excluir o perfil? Os arquivos do backup continuam no disco.
            <button className="button danger" onClick={onDelete}>
              Excluir
            </button>
            <button className="button quiet" onClick={() => setConfirmDelete(false)}>
              Cancelar
            </button>
          </span>
        ) : (
          <button className="button quiet" onClick={() => setConfirmDelete(true)}>
            Excluir perfil
          </button>
        )}
        <span className="spacer" />
        <button className="button quiet" onClick={onReports} disabled={!ready}>
          Relatórios anteriores
        </button>
        <button className="button" onClick={() => onRun(true)} disabled={!ready || !connected}>
          Simular
        </button>
        <button className="button primary" onClick={() => onRun(false)} disabled={!ready || !connected}>
          Sincronizar agora
        </button>
      </footer>
    </div>
  );
}
