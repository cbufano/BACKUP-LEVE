# app — interface Tauri

Esta pasta recebe o aplicativo Tauri 2 na Fase 2 do [roteiro](../docs/ROADMAP.md).

## Como criar

1. Instale o Rust estável: `rustup default stable`.
2. Nesta pasta, rode: `npm create tauri-app@latest . -- --template react-ts`.
3. Adicione `"app/src-tauri"` em `members` no `Cargo.toml` da raiz.
4. Em `app/src-tauri/Cargo.toml`, adicione a dependência `backup-leve-engine = { path = "../../crates/engine" }`.

## Regra

A interface não decide nada sobre arquivos. Toda regra fica no motor (`crates/engine`).
A interface só mostra o estado e envia as escolhas do usuário.
