import { useState } from "react";
import { api, errorText } from "./api";
import {
  choicesFor,
  conflictLabel,
  formatBytes,
  formatClock,
  formatCount,
  formatDuration,
  formatSnapshot,
  formatSpeed,
} from "./format";
import type { Choice, ConflictItem, FileItem, LeftOutItem, Outcome, TargetResult } from "./types";

interface Props {
  profileName: string;
  dryRun: boolean;
  results: TargetResult[];
  startedAt: number;
  finishedAt: number;
  onBack: () => void;
}

/** How many file names each collapsible list shows. */
const LIST_LIMIT = 200;

export function ResultView({ profileName, dryRun, results, startedAt, finishedAt, onBack }: Props) {
  const reports = results.flatMap((r) => (r.report ? [r.report] : []));
  const sum = (pick: (r: (typeof reports)[number]) => number) => reports.reduce((n, r) => n + pick(r), 0);
  const copied = sum((r) => r.copied.length);
  const updated = sum((r) => r.updated.length);
  const conflicts = sum((r) => r.conflicts.length);
  const failed = results.filter((r) => r.error).length + sum((r) => r.errors.length);
  const cancelled = reports.some((r) => r.cancelled);
  const bytesCopied = sum((r) => r.bytes_copied);
  const durationMs = finishedAt - startedAt;

  const title = cancelled
    ? "Sincronização interrompida"
    : dryRun
      ? "Simulação concluída"
      : "Sincronização concluída";

  const parts = [
    `${formatCount(copied)} ${dryRun ? "seriam copiados" : copied === 1 ? "arquivo copiado" : "arquivos copiados"}`,
    `${formatCount(updated)} ${dryRun ? "seriam atualizados" : updated === 1 ? "atualizado" : "atualizados"}`,
  ];
  if (conflicts) parts.push(`${formatCount(conflicts)} ${conflicts === 1 ? "conflito" : "conflitos"} para decidir`);
  if (failed) parts.push(`${formatCount(failed)} com erro`);

  return (
    <div className="result">
      <header className="result-head">
        <div>
          <h1>{title}</h1>
          <p className="lead">
            {profileName}: {parts.join(", ")}.
          </p>
          {dryRun && <p className="muted">Nada foi alterado. A simulação só mostra o que a sincronização faria.</p>}
          {cancelled && <p className="muted">O que já foi copiado está salvo. A próxima sincronização continua daqui.</p>}
        </div>
        <button className="button" onClick={onBack}>
          Voltar ao perfil
        </button>
      </header>

      <dl className="run-stats">
        <div>
          <dt>Início</dt>
          <dd>{formatClock(startedAt)}</dd>
        </div>
        <div>
          <dt>Fim</dt>
          <dd>{formatClock(finishedAt)}</dd>
        </div>
        <div>
          <dt>Duração</dt>
          <dd>{formatDuration(durationMs)}</dd>
        </div>
        <div>
          <dt>Total copiado</dt>
          <dd>{formatBytes(bytesCopied)}</dd>
        </div>
        <div>
          <dt>Velocidade média</dt>
          <dd>{bytesCopied > 0 && durationMs > 0 ? formatSpeed((bytesCopied / durationMs) * 1000) : "—"}</dd>
        </div>
      </dl>

      {results.map((result) => (
        <TargetSection key={result.source} result={result} />
      ))}
    </div>
  );
}

function TargetSection({ result }: { result: TargetResult }) {
  const report = result.report;
  return (
    <section className="target">
      <header className="target-head">
        <div>
          <h2>{result.label}</h2>
          <p className="path muted">
            {result.source} → {result.destination}
          </p>
        </div>
        {result.reportPath && (
          <button className="button quiet" onClick={() => api.openReport(result.reportPath!)}>
            Abrir relatório completo
          </button>
        )}
      </header>

      {result.error && (
        <p className="notice error" role="alert">
          Esta pasta não foi sincronizada: {result.error}.
        </p>
      )}

      {report && (
        <>
          <dl className="stats">
            <Stat label="Copiados" value={report.copied.length} />
            <Stat label="Atualizados" value={report.updated.length} />
            <Stat label="Sem mudança" value={report.unchanged} />
            {report.adopted.length > 0 && <Stat label="Já estavam iguais" value={report.adopted.length} />}
            {report.left_out.length > 0 && <Stat label="Pastas de fora" value={report.left_out.length} />}
            <Stat label="Conflitos" value={report.conflicts.length} tone={report.conflicts.length ? "warn" : undefined} />
            <Stat label="Erros" value={report.errors.length} tone={report.errors.length ? "error" : undefined} />
          </dl>

          {report.conflicts.length > 0 && (
            <Conflicts source={result.source} destination={result.destination} items={report.conflicts} />
          )}

          <FileList title="Alterados na origem, não copiados" note="O modo deste perfil copia só arquivos novos." items={report.changed_in_source} />
          <FileList title="Removidos na origem" note="A cópia continua no disco de backup." items={report.removed_from_source} />
          <FileList title="Copiados" items={report.copied} />
          <FileList title="Atualizados" note="A versão anterior está em .backup-leve\substituidos, no destino." items={report.updated} />
          <LeftOutList items={report.left_out} forgotten={report.left_out_forgotten} />
          {report.errors.length > 0 && (
            <details className="list">
              <summary>Erros ({formatCount(report.errors.length)})</summary>
              <p className="muted small">A próxima sincronização tenta copiar estes arquivos de novo.</p>
              <ul>
                {report.errors.slice(0, LIST_LIMIT).map((item) => (
                  <li key={item.path}>
                    <span className="path">{item.path}</span>
                    <span className="muted">{item.message}</span>
                  </li>
                ))}
              </ul>
            </details>
          )}
        </>
      )}
    </section>
  );
}

function Stat({ label, value, tone }: { label: string; value: number; tone?: "warn" | "error" }) {
  return (
    <div className={`stat ${tone ?? ""}`}>
      <dt>{label}</dt>
      <dd>{formatCount(value)}</dd>
    </div>
  );
}

function FileList({ title, note, items }: { title: string; note?: string; items: FileItem[] }) {
  if (items.length === 0) return null;
  return (
    <details className="list">
      <summary>
        {title} ({formatCount(items.length)})
      </summary>
      {note && <p className="muted small">{note}</p>}
      <ul>
        {items.slice(0, LIST_LIMIT).map((item) => (
          <li key={item.path}>
            <span className="path">{item.path}</span>
            <span className="muted">{formatBytes(item.size)}</span>
          </li>
        ))}
      </ul>
      {items.length > LIST_LIMIT && (
        <p className="muted small">
          Mostrando {LIST_LIMIT} de {formatCount(items.length)}. A lista completa está no relatório.
        </p>
      )}
    </details>
  );
}

function LeftOutList({ items, forgotten }: { items: LeftOutItem[]; forgotten: number }) {
  if (items.length === 0) return null;
  return (
    <details className="list">
      <summary>Pastas deixadas de fora ({formatCount(items.length)})</summary>
      <p className="muted small">Um programa cria estas pastas de novo. Por isso, elas não foram copiadas.</p>
      {forgotten > 0 && (
        <p className="muted small">
          {formatCount(forgotten)} {forgotten === 1 ? "arquivo destas pastas já estava" : "arquivos destas pastas já estavam"}{" "}
          no backup. As cópias continuam no disco de destino, mas o programa não acompanha mais esses arquivos.
        </p>
      )}
      <ul>
        {items.slice(0, LIST_LIMIT).map((item) => (
          <li key={item.path}>
            <span className="path">{item.path}</span>
            <span className="muted">{item.reason}</span>
          </li>
        ))}
      </ul>
      {items.length > LIST_LIMIT && (
        <p className="muted small">
          Mostrando {LIST_LIMIT} de {formatCount(items.length)}. A lista completa está no relatório.
        </p>
      )}
    </details>
  );
}

function Conflicts({ source, destination, items }: { source: string; destination: string; items: ConflictItem[] }) {
  const [choices, setChoices] = useState<Record<string, Choice | undefined>>({});
  const [outcomes, setOutcomes] = useState<Record<string, Outcome>>({});
  const [applying, setApplying] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const open = items.filter((item) => !outcomes[item.path]?.ok);
  const chosen = open.filter((item) => choices[item.path]);

  const chooseAll = (choice: Choice) => {
    const next = { ...choices };
    for (const item of open) {
      if (choicesFor(item.kind).some((c) => c.choice === choice)) next[item.path] = choice;
    }
    setChoices(next);
  };

  const apply = async () => {
    setApplying(true);
    setFailure(null);
    try {
      const decisions = chosen.map((item) => ({
        path: item.path,
        choice: choices[item.path]!,
        source: item.source,
        destination: item.destination,
      }));
      const result = await api.resolveConflicts(source, destination, decisions);
      setOutcomes((previous) => {
        const next = { ...previous };
        for (const outcome of result) next[outcome.path] = outcome;
        return next;
      });
    } catch (error) {
      setFailure(errorText(error));
    } finally {
      setApplying(false);
    }
  };

  return (
    <div className="conflicts">
      <div className="conflicts-head">
        <h3>
          {open.length === 0
            ? "Todos os conflitos foram resolvidos"
            : `${formatCount(open.length)} ${open.length === 1 ? "conflito" : "conflitos"} para decidir`}
        </h3>
        {open.length > 1 && (
          <div className="bulk">
            <span className="muted small">Para todos:</span>
            <button className="button quiet" onClick={() => chooseAll("source_wins")}>Origem vence</button>
            <button className="button quiet" onClick={() => chooseAll("destination_wins")}>Destino vence</button>
            <button className="button quiet" onClick={() => chooseAll("keep_both")}>Manter os dois</button>
          </div>
        )}
      </div>
      <p className="muted small">
        O programa não mexeu nestes arquivos. Escolha o que fazer com cada um. Sem escolha, o arquivo fica como está e
        aparece de novo na próxima sincronização.
      </p>

      <ul className="conflict-list">
        {items.map((item) => {
          const outcome = outcomes[item.path];
          const done = outcome?.ok;
          const slash = item.path.lastIndexOf("/");
          return (
            <li key={item.path} className={`conflict ${done ? "done" : ""}`}>
              <div className="conflict-file">
                <strong>{item.path.slice(slash + 1)}</strong>
                {slash > 0 && <span className="path muted">{item.path.slice(0, slash)}</span>}
              </div>
              <span className="tag">{conflictLabel[item.kind]}</span>
              <div className="versions">
                <span>
                  <span className="muted">Origem</span> {formatSnapshot(item.source)}
                </span>
                <span>
                  <span className="muted">Destino</span> {formatSnapshot(item.destination)}
                </span>
              </div>
              {done ? (
                <p className="outcome ok">Feito: {outcome.message}.</p>
              ) : (
                <div className="segmented" role="radiogroup" aria-label={`O que fazer com ${item.path}`}>
                  {choicesFor(item.kind).map(({ choice, label }) => (
                    <button
                      key={choice}
                      role="radio"
                      aria-checked={choices[item.path] === choice}
                      className={choices[item.path] === choice ? "on" : ""}
                      onClick={() => setChoices({ ...choices, [item.path]: choice })}
                    >
                      {label}
                    </button>
                  ))}
                  <button
                    role="radio"
                    aria-checked={!choices[item.path]}
                    className={!choices[item.path] ? "on later" : ""}
                    onClick={() => setChoices({ ...choices, [item.path]: undefined })}
                  >
                    Decidir depois
                  </button>
                </div>
              )}
              {outcome && !outcome.ok && <p className="outcome error">Não foi feito: {outcome.message}.</p>}
            </li>
          );
        })}
      </ul>

      {failure && <p className="notice error">{failure}</p>}
      {open.length > 0 && (
        <div className="conflicts-actions">
          <button className="button primary" disabled={chosen.length === 0 || applying} onClick={apply}>
            {applying
              ? "Aplicando…"
              : chosen.length === 0
                ? "Escolha uma opção acima"
                : `Aplicar ${chosen.length} ${chosen.length === 1 ? "escolha" : "escolhas"}`}
          </button>
        </div>
      )}
    </div>
  );
}
