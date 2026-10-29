# Histórico de mudanças

Este arquivo segue o formato [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/).
As versões seguem o [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado

- Comando `backup-leve sincronizar` com os modos `sincronizar` e `somente-novos` e a opção `--simulacao`.
- Motor completo: varredura, índice SQLite no destino, cópia segura com arquivo parcial, relatórios JSON e HTML.
- Versões substituídas guardadas em `.backup-leve/substituidos/`.
- Recusa de origem e destino um dentro do outro, de destino de outra origem e de disco de destino desconectado.
- Estrutura do repositório e documentação.
- Especificação das regras de sincronismo.
- Tabela de decisão do motor (`crates/engine/src/decision.rs`) com testes.
