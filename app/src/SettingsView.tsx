import { useEffect, useState } from "react";
import { api, errorText } from "./api";
import type { Provider, Settings } from "./types";

interface Props {
  onBack: () => void;
  onSaved: (settings: Settings) => void;
}

const PROVIDERS: { id: Provider; name: string; model: string; keyHint: string; site: string }[] = [
  { id: "nenhum", name: "Nenhum", model: "", keyHint: "", site: "" },
  { id: "anthropic", name: "Anthropic (Claude)", model: "claude-haiku-5-5", keyHint: "sk-ant-...", site: "console.anthropic.com" },
  { id: "openai", name: "OpenAI (ChatGPT)", model: "gpt-4.1-mini", keyHint: "sk-...", site: "platform.openai.com" },
  { id: "grok", name: "xAI (Grok)", model: "grok-3-mini", keyHint: "xai-...", site: "console.x.ai" },
];

/** Program settings: AI service and the options of a new profile. */
export function SettingsView({ onBack, onSaved }: Props) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string } | null>(null);
  const [testing, setTesting] = useState(false);
  const [showKey, setShowKey] = useState(false);

  useEffect(() => {
    api.getSettings().then(setSettings, (error) => setMessage({ kind: "error", text: errorText(error) }));
  }, []);

  if (!settings) return <p className="muted">Lendo as configurações...</p>;

  const provider = PROVIDERS.find((item) => item.id === settings.provedor) ?? PROVIDERS[0];
  const key = settings.provedor === "nenhum" ? null : settings[settings.provedor];

  const update = (next: Settings) => {
    setSettings(next);
    setMessage(null);
  };

  const save = async () => {
    try {
      await api.saveSettings(settings);
      onSaved(settings);
      setMessage({ kind: "ok", text: "As configurações foram salvas." });
    } catch (error) {
      setMessage({ kind: "error", text: `As configurações não foram salvas: ${errorText(error)}` });
    }
  };

  const test = async () => {
    setTesting(true);
    setMessage(null);
    try {
      await api.testAi(settings);
      setMessage({ kind: "ok", text: `A chave funciona. O serviço ${provider.name} respondeu.` });
    } catch (error) {
      setMessage({ kind: "error", text: errorText(error) });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="settings">
      <div className="block-head">
        <h1>Configurações</h1>
        <button className="button quiet" onClick={onBack}>
          Voltar
        </button>
      </div>

      <section className="block">
        <h2>Inteligência artificial</h2>
        <p className="muted small">
          A IA lê os nomes das pastas e das terminações de arquivo que ocupam mais espaço. Depois, ela sugere o que um
          programa cria de novo. O conteúdo dos arquivos nunca sai do computador. Você aceita ou recusa cada sugestão.
        </p>

        <div className="provider-list" role="radiogroup" aria-label="Serviço de IA">
          {PROVIDERS.map((item) => (
            <label key={item.id} className={`provider ${settings.provedor === item.id ? "active" : ""}`}>
              <input
                type="radio"
                name="provedor"
                checked={settings.provedor === item.id}
                onChange={() => update({ ...settings, provedor: item.id })}
              />
              <span>{item.name}</span>
            </label>
          ))}
        </div>

        {key && settings.provedor !== "nenhum" && (
          <>
            <label className="field-label" htmlFor="ai-key">
              Chave de API
            </label>
            <div className="key-row">
              <input
                id="ai-key"
                className="text-input"
                type={showKey ? "text" : "password"}
                value={key.chave}
                placeholder={provider.keyHint}
                autoComplete="off"
                spellCheck={false}
                onChange={(event) =>
                  update({ ...settings, [settings.provedor]: { ...key, chave: event.target.value } })
                }
              />
              <button className="button quiet" onClick={() => setShowKey(!showKey)}>
                {showKey ? "Ocultar" : "Mostrar"}
              </button>
            </div>
            <p className="muted small">Crie a chave em {provider.site}.</p>

            <label className="field-label" htmlFor="ai-model">
              Modelo
            </label>
            <input
              id="ai-model"
              className="text-input"
              value={key.modelo}
              placeholder={provider.model}
              spellCheck={false}
              onChange={(event) =>
                update({ ...settings, [settings.provedor]: { ...key, modelo: event.target.value } })
              }
            />
            <p className="muted small">Deixe vazio para usar {provider.model}.</p>

            <div className="actions">
              <button className="button" onClick={test} disabled={testing || !key.chave.trim()}>
                {testing ? "Testando..." : "Testar a chave"}
              </button>
            </div>
            <p className="notice warn small">
              A chave fica gravada em texto simples no arquivo configuracoes.toml, na sua pasta de usuário (%APPDATA%\backup-leve).
              Não compartilhe esse arquivo.
            </p>
          </>
        )}
      </section>

      <section className="block">
        <h2>Novos perfis</h2>
        <label className="check">
          <input
            type="checkbox"
            checked={settings.novo_perfil_pular_recriaveis}
            onChange={(event) => update({ ...settings, novo_perfil_pular_recriaveis: event.target.checked })}
          />
          <span>
            <span className="check-title">Pular as pastas que um programa cria de novo</span>
            <span className="check-text">Por exemplo, node_modules, bin e obj do .NET, target e .venv.</span>
          </span>
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={settings.novo_perfil_pular_temporarios}
            onChange={(event) => update({ ...settings, novo_perfil_pular_temporarios: event.target.checked })}
          />
          <span>
            <span className="check-title">Pular os arquivos temporários</span>
            <span className="check-text">Por exemplo, *.tmp e os arquivos ~$ que o Word e o Excel deixam abertos.</span>
          </span>
        </label>
        <p className="muted small">Estas opções valem para os perfis que você criar depois. Cada perfil pode mudar as dele.</p>
      </section>

      {message && (
        <div className={`notice ${message.kind === "ok" ? "ok" : "error"}`} role="status">
          {message.text}
        </div>
      )}

      <div className="actions">
        <button className="button primary" onClick={save}>
          Salvar
        </button>
      </div>
    </div>
  );
}
