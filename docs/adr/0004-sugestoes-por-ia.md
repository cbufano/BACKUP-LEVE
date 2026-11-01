# ADR 0004 — Sugestões de nomes para pular por IA

- Data: 2026-11-01
- Estado: aceito

## Contexto

As regras fixas pulam as pastas recriáveis mais comuns. Cada pessoa tem outras: `Library` do Unity, `DerivedData` do Xcode, caches de ferramentas.
Uma IA reconhece esses nomes. Mas a IA erra, e o programa nunca pode perder dado do usuário.

## Decisão

- O usuário escolhe o serviço na engrenagem de configurações: Anthropic (Claude), OpenAI ou xAI (Grok). O padrão é nenhum.
- O programa envia à IA só um resumo: nomes de pasta, terminações, quantidades e tamanhos. Nunca envia o conteúdo de um arquivo.
- A IA só sugere. O usuário marca cada nome que aceita. O motor aplica os nomes como qualquer nome escolhido pelo usuário.
- A regra de decisão continua no motor. A IA não muda `decision.rs` nem as regras fixas.
- A chave fica em `%APPDATA%\backup-leve\configuracoes.toml`, em texto simples. A tela avisa o usuário.

## Consequências

- O backup funciona igual sem IA. A IA é opcional.
- Os nomes das pastas saem do computador quando o usuário pede uma sugestão.
- Trabalho futuro: guardar a chave no Gerenciador de Credenciais do Windows.
