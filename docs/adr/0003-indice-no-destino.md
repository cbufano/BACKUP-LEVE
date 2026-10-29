# ADR 0003 — Índice em SQLite no disco de destino

- Data: 2026-10-29
- Estado: aceito

## Contexto

Para saber se o destino foi alterado, o programa precisa lembrar como cada arquivo estava no último sincronismo.

## Decisão

- O índice é um arquivo SQLite em `<destino>\.backup-leve\indice.sqlite`.
- Para cada arquivo, o índice guarda: caminho relativo, retrato da origem (tamanho e data), retrato do destino e hash opcional.
- O programa grava o registro logo depois de cada cópia. Se a execução para no meio, a próxima continua de onde parou.
- No fim de cada execução, o programa guarda uma cópia do índice em `<destino>\.backup-leve\indice.anterior.sqlite`.
- Os relatórios de cada execução ficam em `<destino>\.backup-leve\relatorios\`.
- A configuração dos perfis (origens, destino, modo, horário) fica em `%APPDATA%\backup-leve\perfis.toml`.

## Consequências

- O disco de backup carrega a própria história. Ele funciona em outro computador.
- Vários perfis podem usar o mesmo disco de destino. Cada perfil tem a própria pasta de destino.
- Se o usuário apaga a pasta `.backup-leve`, a próxima execução trata tudo como primeira carga. Os arquivos iguais são só registrados, sem nova cópia.
