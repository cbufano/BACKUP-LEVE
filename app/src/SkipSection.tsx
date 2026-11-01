import { useEffect, useState } from "react";
import { api, errorText } from "./api";
import type { Profile, RebuildableRule, Suggestion } from "./types";

interface Props {
  profile: Profile;
  onChange: (profile: Profile) => void;
}

/** "O que fica de fora": rebuildable folders, temporary files, the user's names and AI suggestions. */
export function SkipSection({ profile, onChange }: Props) {
  const [rules, setRules] = useState<RebuildableRule[]>([]);
  const [temporary, setTemporary] = useState<string[]>([]);
  const [names, setNames] = useState(() => profile.pular.join("\n"));
  const [asking, setAsking] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<Suggestion[] | null>(null);
  const [chosen, setChosen] = useState<Set<string>>(new Set());

  useEffect(() => {
    api.rebuildableRules().then(setRules, () => setRules([]));
    api.temporaryNames().then(setTemporary, () => setTemporary([]));
  }, []);
  useEffect(() => setNames(profile.pular.join("\n")), [profile.nome, profile.pular]);
  useEffect(() => {
    setSuggestions(null);
    setAiError(null);
  }, [profile.nome]);

  const commitNames = () => {
    const next = names
      .split("\n")
      .map((name) => name.trim())
      .filter(Boolean);
    if (next.join("\n") !== profile.pular.join("\n")) onChange({ ...profile, pular: next });
  };

  const everything = !profile.pular_recriaveis && !profile.pular_temporarios && profile.pular.length === 0;

  const askAi = async () => {
    setAsking(true);
    setAiError(null);
    setSuggestions(null);
    try {
      const found = await api.suggestSkips(profile);
      setSuggestions(found);
      setChosen(new Set(found.map((item) => item.nome)));
    } catch (error) {
      setAiError(errorText(error));
    } finally {
      setAsking(false);
    }
  };

  const addChosen = () => {
    if (!suggestions) return;
    const current = new Set(profile.pular.map((name) => name.toLowerCase()));
    const added = suggestions
      .map((item) => item.nome)
      .filter((name) => chosen.has(name) && !current.has(name.toLowerCase()));
    if (added.length > 0) onChange({ ...profile, pular: [...profile.pular, ...added] });
    setSuggestions(null);
  };

  return (
    <section className="block">
      <h2>O que fica de fora</h2>
      <label className="check">
        <input
          type="checkbox"
          checked={profile.pular_recriaveis}
          onChange={(event) => onChange({ ...profile, pular_recriaveis: event.target.checked })}
        />
        <span>
          <span className="check-title">Pular as pastas que um programa cria de novo</span>
          <span className="check-text">
            Por exemplo, node_modules, dist, target, .venv e as pastas bin e obj do .NET. O programa só pula a pasta
            quando encontra ao lado dela o arquivo que a recria, como o package.json ou o .csproj.
          </span>
        </span>
      </label>

      {profile.pular_recriaveis && rules.length > 0 && (
        <details className="list rules">
          <summary>Ver as {rules.length + 1} regras</summary>
          <table>
            <thead>
              <tr>
                <th>Pasta</th>
                <th>Só quando ao lado existe</th>
                <th>Quem cria de novo</th>
              </tr>
            </thead>
            <tbody>
              {rules.map((rule) => (
                <tr key={`${rule.pasta}-${rule.quem_recria}`}>
                  <td className="path">{rule.pasta}</td>
                  <td>{needsText(rule)}</td>
                  <td>{rule.quem_recria}</td>
                </tr>
              ))}
              <tr>
                <td className="path">qualquer nome</td>
                <td>CACHEDIR.TAG dentro da pasta</td>
                <td>o programa que marcou a pasta como cache</td>
              </tr>
            </tbody>
          </table>
        </details>
      )}

      <label className="check">
        <input
          type="checkbox"
          checked={profile.pular_temporarios}
          onChange={(event) => onChange({ ...profile, pular_temporarios: event.target.checked })}
        />
        <span>
          <span className="check-title">Pular os arquivos temporários</span>
          <span className="check-text">
            {temporary.length > 0 ? temporary.join("  ") : "*.tmp, ~$* e outros"}. O arquivo ~$ é a trava que o Word
            e o Excel deixam ao lado de um documento aberto.
          </span>
        </span>
      </label>

      <div className="full-backup">
        <span className="muted small">
          {everything
            ? "Backup completo: o programa copia todos os arquivos."
            : "Quer copiar tudo, sem pular nada? Use o backup completo."}
        </span>
        {everything ? (
          <button
            className="button quiet"
            onClick={() => onChange({ ...profile, pular_recriaveis: true, pular_temporarios: true })}
          >
            Voltar a pular
          </button>
        ) : (
          <button
            className="button"
            onClick={() => onChange({ ...profile, pular_recriaveis: false, pular_temporarios: false, pular: [] })}
          >
            Backup completo (100%)
          </button>
        )}
      </div>

      <label className="field-label" htmlFor="skip-names">
        Outros nomes para pular
      </label>
      <textarea
        id="skip-names"
        className="skip-names"
        rows={3}
        value={names}
        placeholder={"Downloads\n*.log"}
        onChange={(event) => setNames(event.target.value)}
        onBlur={commitNames}
        spellCheck={false}
      />
      <p className="muted small">
        Escreva um nome por linha. O nome vale para arquivos e pastas, em qualquer lugar da origem. Use *.log para
        pular os arquivos que terminam em .log. Use Cópia de* para pular os que começam com "Cópia de".
      </p>

      <div className="full-backup">
        <span className="muted small">A IA lê os nomes das pastas de origem e sugere o que um programa cria de novo.</span>
        <button className="button" onClick={askAi} disabled={asking || profile.origens.length === 0}>
          {asking ? "Lendo as pastas..." : "Sugerir com IA"}
        </button>
      </div>
      {aiError && (
        <p className="notice error small" role="alert">
          {aiError}
        </p>
      )}
      {suggestions && suggestions.length === 0 && (
        <p className="notice ok small">A IA não encontrou mais nada para pular.</p>
      )}
      {suggestions && suggestions.length > 0 && (
        <div className="suggestions">
          <strong>Sugestões da IA. Marque o que deve ficar de fora:</strong>
          {suggestions.map((item) => (
            <label key={item.nome} className="check">
              <input
                type="checkbox"
                checked={chosen.has(item.nome)}
                onChange={(event) => {
                  const next = new Set(chosen);
                  if (event.target.checked) next.add(item.nome);
                  else next.delete(item.nome);
                  setChosen(next);
                }}
              />
              <span>
                <span className="check-title suggestion-name">{item.nome}</span>
                <span className="check-text">{item.motivo}</span>
              </span>
            </label>
          ))}
          <div className="key-row">
            <button className="button primary" onClick={addChosen} disabled={chosen.size === 0}>
              Adicionar os marcados
            </button>
            <button className="button quiet" onClick={() => setSuggestions(null)}>
              Descartar
            </button>
          </div>
        </div>
      )}

      <p className="muted small">
        Uma pasta que já está no backup continua no disco de destino. O programa só deixa de acompanhar essa pasta.
      </p>
    </section>
  );
}

/** `package.json e package-lock.json ou yarn.lock`. */
function needsText(rule: RebuildableRule): string {
  if (rule.precisa.length === 0) return "sempre";
  return rule.precisa.map((group) => group.join(" ou ")).join(" e ");
}
