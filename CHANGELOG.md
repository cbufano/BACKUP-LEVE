# Histórico de mudanças

Este arquivo segue o formato [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/).
As versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado

- Janela do programa (Tauri + React): perfis, progresso com botão Parar, resultado, tela de conflitos e relatórios anteriores. Tema claro e escuro.
- Perfis em `%APPDATA%\backup-leve\perfis.toml`, com várias pastas de origem por perfil.
- Resolução de conflitos: origem vence, destino vence, manter os dois, apagar na origem. Versões substituídas na origem vão para a Lixeira.
- Opção de parar a sincronização, também no meio de um arquivo grande. O relatório marca a execução como interrompida.
- Tela de progresso com início, tempo decorrido, velocidade, tempo restante, término previsto e a lista ao vivo dos arquivos copiados.
- Resumo no fim da sincronização: início, fim, duração, total copiado e velocidade média.
- Cópia em blocos para arquivos a partir de 32 MB, com progresso dentro do arquivo.
- Progresso da leitura das pastas: a tela mostra quantos arquivos o programa já leu na origem e no destino. O botão Parar também funciona durante a leitura.
- Na retomada, a tela informa quantos arquivos já estão no backup e não vão ser copiados de novo.
- Pastas que um programa cria de novo ficam de fora: `node_modules`, `dist`, `.next`, `target`, `.venv`, `__pycache__` e outras. O programa só pula a pasta quando encontra o arquivo que a recria. A opção vem ligada e pode ser desligada no perfil.
- Nomes para pular no perfil, como `Downloads` ou `*.tmp`. Na linha de comando: `--pular` e `--copiar-recriaveis`.
- O relatório lista as pastas deixadas de fora.

- Comando `backup-leve sincronizar` com os modos `sincronizar` e `somente-novos` e a opção `--simulacao`.
- Motor completo: varredura, índice SQLite no destino, cópia segura com arquivo parcial, relatórios JSON e HTML.
- Versões substituídas guardadas em `.backup-leve/substituidos/`.
- Recusa de origem e destino um dentro do outro, de destino de outra origem e de disco de destino desconectado.
- Estrutura do repositório e documentação.
- Especificação das regras de sincronismo.
- Tabela de decisão do motor (`crates/engine/src/decision.rs`) com testes.

### Alterado

- Leitura das pastas em paralelo: cerca de 3,7 vezes mais rápida num disco de rede e 6 vezes mais rápida no SSD.
- O índice grava os arquivos copiados em grupos, uma vez por segundo, com acesso exclusivo. Menos consultas ao servidor num disco de rede.
