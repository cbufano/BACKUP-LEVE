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
| `app` | Janela: `app/src-tauri` (Rust) e `app/src` (React). Veja `app/README.md`. |
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
   O índice recebe os arquivos copiados em grupos, uma vez por segundo.
4. **Relatório.** O motor grava um relatório JSON e um relatório HTML em `<destino>\.backup-leve\relatorios\`.
   O relatório lista: copiados, atualizados, conflitos, alterados na origem, removidos na origem, erros.

## Módulos do motor

| Módulo | Função | Estado |
|---|---|---|
| `decision` | Tabela de decisão. Função pura, sem I/O. | feito |
| `scan` | Varredura das árvores em paralelo, com filtros de exclusão. Não segue links nem junções. | feito |
| `index` | Leitura e gravação do SQLite (modo WAL, acesso exclusivo). | feito |
| `copy` | Cópia segura e preservação da data de modificação. | feito |
| `report` | Relatórios JSON e HTML. | feito |
| `job` | Uma execução completa: junta os módulos acima. | feito |
| `paths` | Caminhos relativos do índice e nomes reservados. | feito |
| `exclude` | Regras do que fica de fora: pastas recriáveis e nomes do usuário. Sem I/O. | feito |
| `resolve` | Execução das escolhas do usuário na tela de conflitos. | feito |
| `profile` | Leitura e gravação de `perfis.toml`. | feito |

A linha de comando fica em `crates/cli` e gera o programa `backup-leve.exe`.

## Detalhes da implementação

- **Chave de cada arquivo.** No Windows e no macOS, o nome do arquivo não diferencia maiúsculas de minúsculas. Por isso, a chave no índice é o caminho relativo em minúsculas.
- **Um destino, uma origem.** O índice grava a origem dele. Se outra origem tentar usar o mesmo destino, a execução para.
- **Conflitos não são gravados.** A cada execução, o motor compara o estado atual com o índice. Um conflito não resolvido aparece de novo sozinho.
- **Arquivos grandes.** A partir de 32 MB, o motor copia o arquivo em blocos de 4 MB e avisa o progresso a cada bloco. Assim, a velocidade e o tempo restante não param durante um vídeo grande, e o botão Parar funciona no meio do arquivo. Arquivos menores usam a cópia do sistema, que é mais rápida para muitos arquivos pequenos.
- **Leitura das pastas.** O motor lê a origem e depois o destino. Oito tarefas leem pastas ao mesmo tempo. Num disco de rede, cada pasta espera uma resposta do servidor, e as esperas acontecem juntas. Medição em 29/10/2026: 261 mil arquivos num disco de rede, de 69 s para 19 s; 1,7 milhão de arquivos no SSD, de 42 s para 7 s.
- **Contagem da leitura.** O motor avisa cada arquivo encontrado, e a tela mostra a contagem. No destino, o índice informa quantos arquivos esperar, e a tela mostra uma porcentagem. Se o usuário para durante a leitura, o motor não decide nada: uma leitura incompleta faria arquivos parecerem apagados.
- **Retomada.** Uma execução interrompida não tem estado próprio para retomar. O índice já é esse estado: cada arquivo copiado entra nele. Na execução seguinte, os arquivos do índice que não mudaram ficam como "Sem mudança", e a tela informa quantos são.
- **Índice em grupos.** Os arquivos copiados entram no índice em grupos, uma vez por segundo, e no fim da execução. Se o programa fecha de repente, os arquivos do último grupo ficam no destino sem registro. A próxima execução encontra esses arquivos iguais à origem e registra cada um sem copiar de novo ("registrados sem cópia").
- **Pastas de fora.** A leitura de uma pasta já traz os nomes dos arquivos dela. O motor usa esses nomes para decidir se uma subpasta é recriável (por exemplo, `node_modules` ao lado de `package.json`). Por isso, a regra não custa nenhuma consulta a mais ao disco. Só o `CACHEDIR.TAG` exige ler os primeiros bytes do arquivo. Medição em 29/10/2026, na pasta de projetos do dono: de 1,79 milhão de arquivos (93 GB) para 451 mil (34 GB); a leitura da origem levou 5 s.
- **Acesso exclusivo ao índice.** Durante a execução, só o Backup Leve abre o índice. O SQLite trava o arquivo uma vez e não usa o arquivo de memória compartilhada. Num disco de rede, isso evita várias consultas ao servidor a cada gravação. Uma segunda execução no mesmo destino recebe um erro, e não mistura os dados.

## Bibliotecas

| Necessidade | Biblioteca | Estado |
|---|---|---|
| Varredura | `std::fs`, com 8 tarefas | em uso |
| Índice | `rusqlite` com o recurso `bundled` | em uso |
| Data de modificação | `filetime` | em uso |
| Datas do relatório | `chrono` | em uso |
| Relatório JSON | `serde` + `serde_json` | em uso |
| Linha de comando | `clap` | em uso |
| Lixeira do sistema | `trash` | em uso |
| Perfis | `toml` + `dirs` | em uso |
| Janela | `tauri` 2, `tauri-plugin-dialog`, `tauri-plugin-opener` | em uso |
| Hash | `blake3` | planejado |
| Notificação do Windows | `tauri-plugin-notification` | planejado (Fase 3) |
