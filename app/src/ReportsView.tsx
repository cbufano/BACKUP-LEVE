import { useEffect, useState } from "react";
import { api, errorText } from "./api";
import { formatStamp } from "./format";
import type { ReportEntry } from "./types";

interface Props {
  profileName: string;
  onBack: () => void;
}

export function ReportsView({ profileName, onBack }: Props) {
  const [entries, setEntries] = useState<ReportEntry[] | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    api.listReports(profileName).then(setEntries, (error) => setFailure(errorText(error)));
  }, [profileName]);

  return (
    <div className="reports">
      <header className="result-head">
        <div>
          <h1>Relatórios anteriores</h1>
          <p className="lead">{profileName}</p>
        </div>
        <button className="button" onClick={onBack}>
          Voltar ao perfil
        </button>
      </header>

      {failure && <p className="notice error">{failure}</p>}
      {entries && entries.length === 0 && (
        <p className="empty">
          Ainda não há relatórios. Cada sincronização grava um relatório no disco de backup.
        </p>
      )}
      {entries && entries.length > 0 && (
        <ul className="report-list">
          {entries.map((entry) => (
            <li key={entry.path}>
              <span className="report-date">{formatStamp(entry.stamp)}</span>
              <span>{entry.label}</span>
              <button className="button quiet" onClick={() => api.openReport(entry.path)}>
                Abrir
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
