# Backup Leve — contexto para o Claude Code

## O projeto

Programa open source de backup de pastas para um disco de destino. Vai para o GitHub.
Motor em Rust (`crates/engine`). Linha de comando em `crates/cli`. Janela em Tauri 2 + React + TypeScript (`app/`).

## Leia antes de mexer no código

1. `docs/REGRAS-DE-SINCRONISMO.md` — a especificação. O código segue este documento.
2. `docs/ARQUITETURA.md` — módulos e fluxo.
3. `docs/ROADMAP.md` — fase atual e próximos passos.
4. `docs/adr/` — decisões já tomadas. Não reabrir sem pedido do dono.

## Regras que não mudam

- O programa nunca apaga dado do usuário de forma definitiva.
- Arquivo em conflito não é tocado durante a execução. O usuário decide no fim.
- O programa não roda como serviço. Ele abre, sincroniza e fecha (ADR 0002).
- Toda regra de decisão fica em `crates/engine/src/decision.rs`, sem I/O, com teste.
- Mudou uma regra: mude a especificação no mesmo commit.
- A interface não decide nada sobre arquivos.

## Convenções

- Nomes no código em inglês. Documentação e mensagens da interface em português.
- Documentação segue o ASD-STE100 aplicado ao português: frases curtas, voz ativa, um termo por sentido.
- Termos fixos: origem, destino, índice, retrato, conflito. Não trocar por sinônimos.
- Commits no padrão Conventional Commits.
- Arquivos temporários de trabalho vão para `temp/` (fora do Git).

## Comandos

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd app && npm run build                      # tipos e build da tela
cd app && npm run tauri build -- --no-bundle # gera backup-leve-janela.exe
```

- Rode os testes com `TMP` e `TEMP` apontando para `temp/testes`. Assim nada fica no Temp do Windows.
- Não abra a janela pela sessão do Claude. O dono abre e testa.
- Para revisar o visual sem a janela: `cd app && npm run dev` usa os dados de `app/src/mock.ts`. O endereço `http://localhost:1420/#executando` mostra uma sincronização de exemplo.

## Ambiente do dono

- Windows 11. Rust instalado pelo rustup em `%USERPROFILE%\.cargo\bin`.
- Ferramentas de Build do Visual Studio 2022 instaladas.
