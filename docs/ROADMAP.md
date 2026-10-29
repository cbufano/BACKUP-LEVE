# Roteiro

## Fase 0 — Estrutura (feita)

- [x] Escolha da tecnologia ([ADR 0001](adr/0001-tauri-rust-react.md))
- [x] Especificação das regras ([REGRAS-DE-SINCRONISMO.md](REGRAS-DE-SINCRONISMO.md))
- [x] Estrutura do repositório para o GitHub
- [x] Tabela de decisão em código, com testes

## Fase 1 — Motor pela linha de comando (feita)

- [x] Varredura com filtros de exclusão (`Thumbs.db`, `desktop.ini`, `$RECYCLE.BIN`, `System Volume Information`)
- [x] Índice SQLite
- [x] Cópia segura com retomada
- [x] Relatório JSON e HTML
- [x] Comando `backup-leve sincronizar --origem <pasta> --destino <pasta>`
- [x] Testes com pastas temporárias reais

Resultado medido em 29/10/2026: primeira carga de 2003 arquivos (50 MB) em 2 s; segunda execução em 0,1 s.
O programa compilado tem 2,6 MB.

## Fase 2 — Interface

- [ ] Projeto Tauri em `app/`
- [ ] Tela de perfis: escolher pastas de origem e o destino
- [ ] Tela de progresso: arquivos, bytes, tempo restante
- [ ] Tela de conflitos com as quatro opções
- [ ] Tela de relatórios anteriores

## Fase 3 — Execução agendada

- [ ] Modo `--silencioso`
- [ ] Criar e remover tarefa no Agendador de Tarefas do Windows
- [ ] Notificação do Windows no fim
- [ ] Prioridade baixa de CPU e de disco

## Fase 4 — Publicação

- [ ] Instalador (NSIS ou MSI) gerado pelo CI
- [ ] Versão portátil (um `.exe` só)
- [ ] Assinatura de código
- [ ] Primeira versão pública (0.1.0)

## Depois

- Linux e macOS
- Modo serviço (só depois que o motor estiver estável)
- Espelho estrito (apagar no destino o que sumiu da origem, com confirmação)
