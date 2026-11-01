//! Calls to the AI services. The AI gets a summary with names and sizes, never the
//! content of a file. It only suggests names to skip; the user accepts or not.

use std::path::PathBuf;
use std::time::Duration;

use backup_leve_engine::exclude::{is_valid_name, REBUILDABLE, TEMPORARY};
use backup_leve_engine::profile::folder_label;
use backup_leve_engine::settings::Provider;
use backup_leve_engine::survey::{self, Summary};
use backup_leve_engine::Exclusions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// A name to skip: exact, `*.ext` or `inicio*`.
    pub nome: String,
    /// Why the name can stay out, in Portuguese.
    pub motivo: String,
}

const INSTRUCTIONS: &str = "Você ajuda um programa de backup de pastas. \
Você recebe um resumo das pastas de origem: os nomes de pasta e as terminações de arquivo que ocupam mais espaço, \
com quantidade, número de arquivos e bytes. \
Indique só o que um programa cria de novo sozinho ou o que é temporário: cache, build, pacotes baixados, \
arquivos de log de compilação, arquivos temporários. Exemplos: Library e Temp de um projeto Unity, \
.gradle, DerivedData do Xcode, *.obj e *.pdb de compilação, .cache. \
Nunca indique documentos, fotos, vídeos, código-fonte, planilhas ou bancos de dados do usuário. Na dúvida, não indique. \
O programa já pula estas pastas e arquivos, então não repita: {ja}. \
Cada nome vale para arquivos e pastas em qualquer lugar da origem. Use o nome exato da pasta, \
*.ext para uma terminação ou inicio* para um começo. Nunca use / nem \\. \
Responda só com JSON, sem texto em volta, no formato \
{\"sugestoes\":[{\"nome\":\"...\",\"motivo\":\"frase curta em português\"}]}. No máximo 20 sugestões.";

/// Reads the sources and asks the AI. Returns only valid names that are not in `current`.
pub async fn suggest(
    provider: Provider,
    key: String,
    model: String,
    sources: Vec<PathBuf>,
    exclusions: Exclusions,
    current: Vec<String>,
) -> Result<Vec<Suggestion>, String> {
    let summaries = tauri::async_runtime::spawn_blocking(move || {
        sources
            .iter()
            .map(|source| (folder_label(source), survey::summarize(source, &exclusions)))
            .collect::<Vec<(String, Summary)>>()
    })
    .await
    .map_err(|err| err.to_string())?;
    if summaries.iter().all(|(_, s)| s.files_read == 0) {
        return Err("as pastas de origem estão vazias ou não foram lidas".into());
    }

    let mut known: Vec<String> = REBUILDABLE
        .iter()
        .map(|rule| rule.folder.to_owned())
        .collect();
    known.extend(TEMPORARY.iter().map(|name| (*name).to_owned()));
    known.extend(current.iter().cloned());
    known.sort();
    known.dedup();
    let system = INSTRUCTIONS.replace("{ja}", &known.join(", "));
    let user = serde_json::to_string(&json!({ "origens": summaries
        .iter()
        .map(|(label, summary)| json!({ "pasta": label, "resumo": summary }))
        .collect::<Vec<_>>() }))
    .map_err(|err| err.to_string())?;

    let text = ask(provider, &key, &model, &system, &user).await?;
    let mut out = parse(&text)?;
    let taken: Vec<String> = known.iter().map(|name| name.to_lowercase()).collect();
    out.retain(|s| is_valid_name(&s.nome) && !taken.contains(&s.nome.trim().to_lowercase()));
    for s in &mut out {
        s.nome = s.nome.trim().to_owned();
    }
    out.dedup_by(|a, b| a.nome.eq_ignore_ascii_case(&b.nome));
    Ok(out)
}

/// Sends a short question to check the key.
pub async fn test(provider: Provider, key: &str, model: &str) -> Result<String, String> {
    ask(
        provider,
        key,
        model,
        "Responda só com a palavra OK.",
        "Teste de conexão.",
    )
    .await
}

async fn ask(
    provider: Provider,
    key: &str,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|err| err.to_string())?;
    let request = match provider {
        Provider::Nenhum => return Err("escolha um serviço de IA nas configurações".into()),
        Provider::Openai | Provider::Grok => {
            let url = if provider == Provider::Openai {
                "https://api.openai.com/v1/chat/completions"
            } else {
                "https://api.x.ai/v1/chat/completions"
            };
            client.post(url).bearer_auth(key).json(&json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": user },
                ],
            }))
        }
        Provider::Anthropic => client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": model,
                "max_tokens": 2000,
                "system": system,
                "messages": [{ "role": "user", "content": user }],
            })),
    };
    let response = request
        .send()
        .await
        .map_err(|err| format!("o serviço de IA não respondeu: {err}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|err| format!("resposta inválida do serviço de IA: {err}"))?;
    if !status.is_success() {
        let message = body
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("sem detalhes");
        return Err(format!(
            "o serviço de IA recusou o pedido ({status}): {message}"
        ));
    }
    let text = match provider {
        Provider::Anthropic => body.pointer("/content/0/text"),
        _ => body.pointer("/choices/0/message/content"),
    };
    text.and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "o serviço de IA respondeu sem texto".into())
}

/// Reads the JSON in the answer, also when the AI puts text around it.
fn parse(text: &str) -> Result<Vec<Suggestion>, String> {
    #[derive(Deserialize)]
    struct Answer {
        sugestoes: Vec<Suggestion>,
    }
    let start = text.find('{');
    let end = text.rfind('}');
    let (Some(start), Some(end)) = (start, end) else {
        return Err("a IA não respondeu no formato esperado".into());
    };
    if end < start {
        return Err("a IA não respondeu no formato esperado".into());
    }
    serde_json::from_str::<Answer>(&text[start..=end])
        .map(|answer| answer.sugestoes)
        .map_err(|_| "a IA não respondeu no formato esperado".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reads_json_with_text_around() {
        let text = "Aqui está:\n```json\n{\"sugestoes\":[{\"nome\":\"Library\",\"motivo\":\"cache do Unity\"}]}\n```";
        let out = parse(text).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].nome, "Library");
        assert!(parse("sem json").is_err());
    }
}
