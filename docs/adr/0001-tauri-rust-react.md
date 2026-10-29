# ADR 0001 — Tauri 2 + Rust + React

- Data: 2026-10-29
- Estado: aceito

## Contexto

O programa precisa ser leve, ter uma interface agradável e ler milhões de arquivos sem travar.
Ele roda sob demanda: abre, sincroniza e fecha.
O projeto é open source e deve rodar no Windows primeiro. Linux e macOS vêm depois.

## Decisão

- Motor de backup em **Rust** (biblioteca `crates/engine`).
- Interface com **Tauri 2**, que usa o WebView2 que já vem no Windows 11.
- Tela em **React + TypeScript + Vite**, o mesmo conjunto que o dono do projeto já usa.

## Alternativas avaliadas

| Opção | Instalador | Memória em uso | Por que não |
|---|---|---|---|
| Tauri + Rust | ~5–10 MB | ~30–60 MB | Escolhida. |
| Go + Wails | ~10–15 MB | ~40–70 MB | Boa opção. Perde para o Rust em controle fino de I/O e em segurança de memória. |
| Electron | ~80–150 MB | ~150–300 MB | Pesado demais para um programa "leve". |
| .NET WPF / WinUI | ~50–150 MB com runtime | ~80–150 MB | Só Windows. Interface moderna dá mais trabalho. |
| Python + Qt | ~60–100 MB | ~80–150 MB | Empacotar é difícil. Varredura de muitos arquivos é lenta. |

Os números são estimativas de mercado, não medições deste projeto.

## Consequências

- O motor fica separado da tela. O mesmo motor serve a janela e a execução agendada sem janela.
- O motor pode ser testado sem a interface.
- Contribuidores precisam do Rust e do Node.js para compilar a interface.
