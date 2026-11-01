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

## Fase 2 — Interface (feita, falta o teste do dono)

- [x] Projeto Tauri em `app/`
- [x] Tela de perfis: escolher pastas de origem e o destino
- [x] Tela de progresso: arquivos, bytes e botão Parar
- [x] Estatísticas ao vivo: início, tempo decorrido, velocidade, tempo restante e término previsto
- [x] Lista ao vivo dos arquivos copiados, com o mais recente no topo
- [x] Resumo no fim: início, fim, duração, total copiado e velocidade média
- [x] Tela de conflitos com as quatro opções
- [x] Pastas que um programa cria de novo (`node_modules`, `target`, `.venv`...) ficam de fora, e o usuário pode pular outros nomes
- [x] Tela de relatórios anteriores
- [x] Arquivos temporários (`*.tmp`, `~$*`...) ficam de fora, com opção para desligar
- [x] Botão "Backup completo (100%)"
- [x] Engrenagem de configurações: serviço de IA (Anthropic, OpenAI, Grok), chave e modelo
- [x] Sugestões da IA de nomes para pular ([ADR 0004](adr/0004-sugestoes-por-ia.md))
- [ ] Teste do dono na janela real
- [ ] Ícone próprio (hoje é o ícone padrão do Tauri)

O programa com janela tem 6,7 MB.

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
