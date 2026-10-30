import { useEffect, useRef, useState } from "react";
import { api } from "./api";
import { formatBytes, formatClock, formatCount, formatSpeed, formatTimer } from "./format";
import type { Progress } from "./types";

interface Props {
  profileName: string;
  dryRun: boolean;
  progress: Progress | null;
  startedAt: number;
}

/** The current speed looks at the last 10 seconds. */
const SPEED_WINDOW_MS = 10_000;
/** Rows kept in the live list. The report has every file. */
const LIST_LIMIT = 500;

interface Row {
  key: string;
  path: string;
  size: number;
  at: number;
}

interface Tracker {
  target: number;
  /** Bytes of the folders already finished in this run. */
  bytesBefore: number;
  lastBytes: number;
  copyStartedAt: number;
  samples: { at: number; bytes: number }[];
}

export function RunView({ profileName, dryRun, progress, startedAt }: Props) {
  const [stopping, setStopping] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const [rows, setRows] = useState<Row[]>([]);
  const [filesCopied, setFilesCopied] = useState(0);
  const tracker = useRef<Tracker>({ target: 0, bytesBefore: 0, lastBytes: 0, copyStartedAt: 0, samples: [] });

  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!progress) return;
    const t = tracker.current;
    const at = Date.now();
    if (progress.target !== t.target) {
      t.bytesBefore += t.lastBytes;
      t.lastBytes = 0;
      t.target = progress.target;
    }
    t.lastBytes = progress.bytesDone;
    if (progress.stage === "copiando" && progress.filesTotal > 0 && !t.copyStartedAt) t.copyStartedAt = at;
    t.samples.push({ at, bytes: t.bytesBefore + progress.bytesDone });
    while (t.samples.length > 2 && at - t.samples[0].at > SPEED_WINDOW_MS) t.samples.shift();

    if (progress.recent.length) {
      const fresh = progress.recent
        .map((file, index) => ({ ...file, at, key: `${at}-${index}-${file.path}` }))
        .reverse();
      setRows((previous) => [...fresh, ...previous].slice(0, LIST_LIMIT));
      setFilesCopied((count) => count + progress.recent.length);
    }
  }, [progress]);

  const t = tracker.current;
  const copying = progress?.stage === "copiando";
  const copiedBytes = t.bytesBefore + (progress?.bytesDone ?? 0);
  const first = t.samples[0];
  const last = t.samples[t.samples.length - 1];
  const windowMs = first && last ? Math.max(now, last.at) - first.at : 0;
  const speed = windowMs > 1000 ? ((last.bytes - first.bytes) / windowMs) * 1000 : 0;
  const averageSpeed = t.copyStartedAt && now > t.copyStartedAt ? (copiedBytes / (now - t.copyStartedAt)) * 1000 : 0;
  const remainingBytes = progress ? Math.max(0, progress.bytesTotal - progress.bytesDone) : 0;
  const etaMs = copying && averageSpeed > 0 && remainingBytes > 0 ? (remainingBytes / averageSpeed) * 1000 : null;
  const perFolder = (progress?.targets ?? 1) > 1;

  const reading = !progress || progress.stage === "lendo";
  // While reading the destination, the index says about how many files to expect.
  const readingPercent =
    progress?.side === "destino" && progress.destinationExpected > 0
      ? Math.min(99, Math.round((progress.destinationFound / progress.destinationExpected) * 100))
      : null;
  const percent =
    readingPercent ??
    (progress && progress.bytesTotal > 0
      ? Math.min(100, Math.round((progress.bytesDone / progress.bytesTotal) * 100))
      : copying
        ? 100
        : 0);
  const unknownLength = reading && readingPercent === null;

  const stop = async () => {
    setStopping(true);
    await api.cancelRun();
  };

  return (
    <div className="run">
      <header className="run-head">
        <div>
          <h1>
            {dryRun ? "Simulando" : "Sincronizando"} “{profileName}”
          </h1>
          {progress && perFolder && (
            <p className="muted">
              Pasta {progress.target} de {progress.targets}: <strong>{progress.label}</strong>
            </p>
          )}
        </div>
        <button className="button" onClick={stop} disabled={stopping}>
          {stopping ? "Parando…" : "Parar"}
        </button>
      </header>

      <div
        className={`meter ${unknownLength ? "reading" : ""}`}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={unknownLength ? undefined : percent}
        aria-label={reading ? "Progresso da leitura" : "Progresso da cópia"}
      >
        <div className="meter-fill" style={{ width: unknownLength ? undefined : `${percent}%` }} />
      </div>

      <div className="meter-figures">
        {reading ? (
          <span>Lendo as pastas para saber o que copiar…</span>
        ) : progress.filesTotal === 0 ? (
          <span>{dryRun ? "Simulação: nada é copiado." : "Nada para copiar nesta pasta."}</span>
        ) : (
          <>
            <span>
              <strong>{formatCount(progress.filesDone)}</strong> de {formatCount(progress.filesTotal)} arquivos
            </span>
            <span>
              {formatBytes(progress.bytesDone)} de {formatBytes(progress.bytesTotal)}
            </span>
            <span className="meter-percent">{percent}%</span>
          </>
        )}
      </div>

      {copying && progress.alreadyDone > 0 && (
        <p className="resumed">
          {formatCount(progress.alreadyDone)} {progress.alreadyDone === 1 ? "arquivo já está" : "arquivos já estão"} no
          backup e não {progress.alreadyDone === 1 ? "vai ser copiado" : "vão ser copiados"} de novo. A contagem acima
          mostra só o que falta.
        </p>
      )}

      {reading && (
        <ol className="reading-steps" aria-live="polite">
          <ReadingStep
            name="Origem"
            state={progress?.side === "destino" ? "done" : "now"}
            found={progress?.sourceFound ?? 0}
          />
          <ReadingStep
            name="Destino"
            state={progress?.side === "destino" ? "now" : "next"}
            found={progress?.destinationFound ?? 0}
            expected={progress?.destinationExpected ?? 0}
          />
        </ol>
      )}

      <dl className="run-stats">
        <div>
          <dt>Início</dt>
          <dd>{formatClock(startedAt)}</dd>
        </div>
        <div>
          <dt>Tempo decorrido</dt>
          <dd>{formatTimer(now - startedAt)}</dd>
        </div>
        <div>
          <dt>Velocidade</dt>
          <dd>{copying && speed > 0 ? formatSpeed(speed) : "—"}</dd>
        </div>
        <div>
          <dt>Tempo restante</dt>
          <dd>{etaMs !== null ? formatTimer(etaMs) : copying && remainingBytes > 0 ? "calculando…" : "—"}</dd>
        </div>
        <div>
          <dt>Término previsto</dt>
          <dd>{etaMs !== null ? formatClock(now + etaMs) : "—"}</dd>
        </div>
      </dl>
      {perFolder && (
        <p className="muted small">Tempo restante e término previsto valem para a pasta atual. São aproximados.</p>
      )}

      {progress?.path && (
        <p className="current">
          <span className="muted">{copying ? "Copiando agora:" : "Lendo agora:"}</span>{" "}
          <span className="path">{progress.path}</span>
        </p>
      )}

      <details className="live" open>
        <summary>Arquivos copiados ({formatCount(filesCopied)})</summary>
        {rows.length === 0 ? (
          <p className="muted small">
            {dryRun ? "Na simulação, nada é copiado." : "A lista começa quando o primeiro arquivo terminar."}
          </p>
        ) : (
          <ol className="live-files" aria-live="off">
            {rows.map((row) => (
              <li key={row.key}>
                <span className="live-time">{formatClock(row.at)}</span>
                <span className="path">{row.path}</span>
                <span className="live-size">{formatBytes(row.size)}</span>
              </li>
            ))}
          </ol>
        )}
        {filesCopied > LIST_LIMIT && (
          <p className="muted small">A lista mostra os {LIST_LIMIT} mais recentes. O relatório tem todos.</p>
        )}
      </details>

      <p className="muted small">
        Parar é seguro. O que já foi copiado fica no índice, e a próxima sincronização continua daqui.
      </p>
    </div>
  );
}

function ReadingStep({
  name,
  state,
  found,
  expected = 0,
}: {
  name: string;
  state: "done" | "now" | "next";
  found: number;
  expected?: number;
}) {
  const files =
    state === "now" && expected > 0
      ? `${formatCount(found)} de ${formatCount(expected)} arquivos`
      : `${formatCount(found)} ${found === 1 ? "arquivo" : "arquivos"}`;
  return (
    <li className={`reading-step ${state}`}>
      <span className="step-mark" aria-hidden="true" />
      <span className="step-name">{name}</span>
      <span className="step-count">
        {state === "next" ? "aguardando" : state === "now" ? `lendo… ${files}` : files}
      </span>
    </li>
  );
}
