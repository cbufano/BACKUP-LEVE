# app — janela do Backup Leve

Aplicativo Tauri 2. A parte em Rust fica em `src-tauri/`. A tela, em React + TypeScript, fica em `src/`.

## Regra

A janela não decide nada sobre arquivos. Toda regra fica no motor (`crates/engine`).
A janela só mostra o estado e envia as escolhas do usuário.

## Comandos

Rode os comandos nesta pasta (`app/`).

| Comando | Efeito |
|---|---|
| `npm install` | Instala as dependências da tela. Rode uma vez. |
| `npm run tauri dev` | Abre a janela em modo de desenvolvimento. |
| `npm run tauri build -- --no-bundle` | Gera `target/release/backup-leve-janela.exe`, sem instalador. |
| `npm run dev` | Abre só a tela no navegador, com dados de exemplo. Serve para trabalhar no visual sem disco de backup. |

## Pré-visualização no navegador

Fora do Tauri, `src/api.ts` usa os dados de `src/mock.ts`.
Abra `http://localhost:1420/#executando` para ver uma sincronização de exemplo e a tela de conflitos.

## Arquivos

| Arquivo | Conteúdo |
|---|---|
| `src-tauri/src/lib.rs` | Comandos que a tela chama: perfis, sincronizar, parar, resolver conflitos, relatórios. |
| `src/api.ts` | Ponte entre a tela e os comandos em Rust. |
| `src/App.tsx` | Lista de perfis e troca de telas. |
| `src/ProfileView.tsx` | Tela do perfil: destino, pastas e modo. |
| `src/RunView.tsx` | Tela de progresso. |
| `src/ResultView.tsx` | Resultado da sincronização e tela de conflitos. |
| `src/ReportsView.tsx` | Relatórios anteriores. |
| `src/styles.css` | Cores, fontes e layout, com tema claro e escuro. |
