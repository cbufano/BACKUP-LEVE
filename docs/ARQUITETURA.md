# Arquitetura

## Visão geral

```
┌──────────────────────────┐      ┌──────────────────────────┐
│  Janela (React + Tauri)  │      │  Modo silencioso (CLI)   │
│  escolher pastas         │      │  chamado pelo Agendador  │
│  ver progresso           │      │  de Tarefas do Windows   │
│  resolver conflitos      │      │                          │
└────────────┬─────────────┘      └────────────┬─────────────┘
             │                                 │
             └────────────┬────────────────────┘
                          ▼
             ┌──────────────────────────┐
             │   Motor (crates/engine)  │
             │  1. varredura            │
             │  2. decisão              │
             │  3. cópia segura         │
             │  4. índice (SQLite)      │
             │  5. relatório            │
             └──────────────────────────┘
```

## Pastas do repositório

| Pasta | Conteúdo |
|---|---|
| `crates/engine` | Motor em Rust. Não depende da interface. |
| `app` | Aplicativo Tauri: `app/src-tauri` (Rust) e `app/src` (React). Ainda não criado. |
| `docs` | Especificação, arquitetura, decisões (ADR) e roteiro. |
| `.github` | CI, modelos de issue e de pull request. |
| `.claude` | Configuração do Claude Code para este projeto. |
| `temp` | Arquivos temporários de trabalho. Fica fora do Git. |

## Fluxo de uma execução

1. **Varredura.** O motor lê a árvore da origem e a árvore do destino.
   No Windows, a leitura de pasta já traz tamanho e data. Por isso, ler as duas árvores é mais rápido que consultar arquivo por arquivo.
2. **Decisão.** Para cada caminho, o motor aplica a tabela de [REGRAS-DE-SINCRONISMO.md](REGRAS-DE-SINCRONISMO.md).
   O resultado é um plano: copiar, atualizar, registrar, conflito, nada.
3. **Cópia.** O motor executa o plano com cópia segura (arquivo parcial e depois renomear).
   O índice recebe um registro a cada arquivo copiado.
4. **Relatório.** O motor grava um relatório JSON e um relatório HTML em `<destino>\.backup-leve\relatorios\`.
   O relatório lista: copiados, atualizados, conflitos, alterados na origem, removidos na origem, erros.

## Módulos do motor (planejados)

| Módulo | Função |
|---|---|
| `decision` | Tabela de decisão. Função pura, sem I/O. **Já existe.** |
| `scan` | Varredura das árvores, com filtros de exclusão. |
| `index` | Leitura e gravação do SQLite. |
| `copy` | Cópia segura, preservação de data, verificação opcional por hash. |
| `resolve` | Execução das escolhas do usuário na tela de conflitos. |
| `report` | Relatórios JSON e HTML. |
| `profile` | Leitura e gravação de `perfis.toml`. |

## Bibliotecas candidatas

| Necessidade | Biblioteca |
|---|---|
| Varredura | `walkdir` ou `jwalk` (paralela) |
| Índice | `rusqlite` com o recurso `bundled` |
| Hash | `blake3` |
| Lixeira do Windows | `trash` |
| Data de modificação | `filetime` |
| Configuração | `serde` + `toml` |
| Interface | `tauri` 2, `tauri-plugin-dialog`, `tauri-plugin-notification` |

A escolha final de cada biblioteca acontece na implementação de cada módulo.
