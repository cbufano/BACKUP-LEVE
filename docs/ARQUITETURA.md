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
| `crates/cli` | Linha de comando `backup-leve`. |
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

## Módulos do motor

| Módulo | Função | Estado |
|---|---|---|
| `decision` | Tabela de decisão. Função pura, sem I/O. | feito |
| `scan` | Varredura das árvores, com filtros de exclusão. Não segue links nem junções. | feito |
| `index` | Leitura e gravação do SQLite (modo WAL). | feito |
| `copy` | Cópia segura e preservação da data de modificação. | feito |
| `report` | Relatórios JSON e HTML. | feito |
| `job` | Uma execução completa: junta os módulos acima. | feito |
| `paths` | Caminhos relativos do índice e nomes reservados. | feito |
| `resolve` | Execução das escolhas do usuário na tela de conflitos. | Fase 2 |
| `profile` | Leitura e gravação de `perfis.toml`. | Fase 2 |

A linha de comando fica em `crates/cli` e gera o programa `backup-leve.exe`.

## Detalhes da implementação

- **Chave de cada arquivo.** No Windows e no macOS, o nome do arquivo não diferencia maiúsculas de minúsculas. Por isso, a chave no índice é o caminho relativo em minúsculas.
- **Um destino, uma origem.** O índice grava a origem dele. Se outra origem tentar usar o mesmo destino, a execução para.
- **Conflitos não são gravados.** A cada execução, o motor compara o estado atual com o índice. Um conflito não resolvido aparece de novo sozinho.
- **Índice após cada cópia.** O registro entra no índice logo depois de cada cópia. Se a energia cai, os registros perdidos são refeitos na próxima execução como "registrados sem cópia", sem copiar de novo.

## Bibliotecas

| Necessidade | Biblioteca | Estado |
|---|---|---|
| Varredura | `walkdir` | em uso |
| Índice | `rusqlite` com o recurso `bundled` | em uso |
| Data de modificação | `filetime` | em uso |
| Datas do relatório | `chrono` | em uso |
| Relatório JSON | `serde` + `serde_json` | em uso |
| Linha de comando | `clap` | em uso |
| Hash | `blake3` | planejado |
| Lixeira do Windows | `trash` | planejado |
| Configuração | `toml` | planejado |
| Interface | `tauri` 2, `tauri-plugin-dialog`, `tauri-plugin-notification` | planejado |
