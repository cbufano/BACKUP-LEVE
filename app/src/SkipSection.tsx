import { useEffect, useState } from "react";
import { api } from "./api";
import type { Profile, RebuildableRule } from "./types";

interface Props {
  profile: Profile;
  onChange: (profile: Profile) => void;
}

/** "O que fica de fora": rebuildable folders and the user's own names. */
export function SkipSection({ profile, onChange }: Props) {
  const [rules, setRules] = useState<RebuildableRule[]>([]);
  const [names, setNames] = useState(() => profile.pular.join("\n"));

  useEffect(() => {
    api.rebuildableRules().then(setRules, () => setRules([]));
  }, []);
  useEffect(() => setNames(profile.pular.join("\n")), [profile.nome, profile.pular]);

  const commitNames = () => {
    const next = names
      .split("\n")
      .map((name) => name.trim())
      .filter(Boolean);
    if (next.join("\n") !== profile.pular.join("\n")) onChange({ ...profile, pular: next });
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
            Por exemplo, node_modules, dist, target e .venv. O programa só pula a pasta quando encontra ao lado dela o
            arquivo que a recria, como o package.json.
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

      <label className="field-label" htmlFor="skip-names">
        Outros nomes para pular
      </label>
      <textarea
        id="skip-names"
        className="skip-names"
        rows={3}
        value={names}
        placeholder={"Downloads\n*.tmp"}
        onChange={(event) => setNames(event.target.value)}
        onBlur={commitNames}
        spellCheck={false}
      />
      <p className="muted small">
        Escreva um nome por linha. O nome vale para arquivos e pastas, em qualquer lugar da origem. Use *.tmp para pular
        todos os arquivos que terminam em .tmp.
      </p>
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
