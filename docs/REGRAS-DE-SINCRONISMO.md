# Regras de sincronismo

Este documento é a especificação do comportamento do Backup Leve.
O código em `crates/engine/src/decision.rs` implementa esta tabela.
Se o código e este documento não concordam, o erro está no código.

## Princípio número 1

O Backup Leve nunca apaga dado do usuário de forma definitiva.

- Quando um arquivo do destino é substituído, a versão antiga vai para `<destino>\.backup-leve\substituidos\<data>\`.
- Quando um arquivo da origem é substituído, a versão antiga vai para a Lixeira do Windows.
- Quando um arquivo some da origem, a cópia no destino fica. O relatório mostra o arquivo.

## Termos

| Termo | Sentido |
|---|---|
| Origem | A pasta que o usuário escolheu para fazer backup. |
| Destino | A pasta no disco de backup que recebe as cópias. |
| Índice | O registro do último sincronismo, arquivo por arquivo. Fica em `<destino>\.backup-leve\indice.sqlite`. |
| Retrato | O tamanho e a data de modificação de um arquivo em um momento. |
| Conflito | Um arquivo que o programa não altera. O usuário decide o que fazer. |

## Como o programa compara dois arquivos

Dois retratos são iguais quando:

1. O tamanho é igual.
2. A diferença entre as datas de modificação é de até 2 segundos.

Nota: a tolerância de 2 segundos existe porque discos FAT32 gravam a data com precisão de 2 segundos.

Nota: a comparação por conteúdo (hash BLAKE3) é uma opção do usuário ("verificação profunda"). Ela é lenta e não é o padrão.

Ao copiar, o programa grava no destino a mesma data de modificação da origem.
Por isso, origem e destino ficam com retratos iguais depois da cópia.

## Os dois modos

| Modo | O que faz com arquivo alterado na origem |
|---|---|
| Somente novos | Não copia. Coloca o arquivo na lista "alterados na origem". |
| Sincronizar | Copia a versão nova para o destino. A versão antiga do destino vai para `substituidos`. |

## Tabela de decisão

Para cada arquivo, o programa olha três coisas:

- **O** = o arquivo existe na origem?
- **D** = o arquivo existe no destino?
- **I** = o índice tem registro do arquivo?

| O | D | I | Situação | Ação |
|---|---|---|---|---|
| sim | não | não | Arquivo novo. | Copiar. |
| sim | sim | não | Primeira carga com dados que já estão no destino. | Se os retratos são iguais: registrar no índice, sem copiar. Se são diferentes: **conflito** "existe com conteúdo diferente". |
| sim | sim | sim | Arquivo já sincronizado. | Ver a tabela seguinte. |
| sim | não | sim | Alguém apagou o arquivo no destino. | **Conflito** "apagado no destino". |
| não | sim | sim | O usuário apagou o arquivo na origem. | Manter a cópia no destino. Listar no relatório. |
| não | não | sim | O arquivo sumiu dos dois lados. | Remover o registro do índice. |
| não | sim | não | Arquivo que só existe no destino e não é do programa. | Não fazer nada. |

### Arquivo já sincronizado (O, D e I existem)

O programa compara o retrato atual com o retrato do índice, nos dois lados.

| Origem mudou? | Destino mudou? | Ação |
|---|---|---|
| não | não | Nada. |
| sim | não | Modo Sincronizar: atualizar o destino. Modo Somente novos: listar em "alterados na origem". |
| não | sim | **Conflito** "alterado no destino". |
| sim | sim | Se a origem e o destino agora são iguais: registrar no índice. Se não: **conflito** "alterado nos dois lados". |

## Conflitos

Durante a execução, o programa não toca nos arquivos em conflito.
No fim, a tela de conflitos mostra a lista. Para cada arquivo, o usuário escolhe uma opção:

| Opção | O que acontece |
|---|---|
| A origem vence | A origem é copiada para o destino. A versão do destino vai para `substituidos`. |
| O destino vence | O destino é copiado para a origem. A versão da origem vai para a Lixeira do Windows. |
| Manter os dois | No destino ficam as duas versões. Cada nome recebe a data de modificação: `relatorio (origem 2026-10-28 14-30).docx` e `relatorio (destino 2026-10-29 09-12).docx`. A origem não muda. |
| Decidir depois | Nada muda. O conflito aparece de novo na próxima execução. |

O programa não precisa gravar os conflitos. A cada execução, ele compara de novo o estado atual com o índice, e o conflito aparece outra vez.
Por isso, depois de uma execução agendada (sem janela), a tela de conflitos aparece na próxima vez que o usuário abrir o programa.

Para o conflito "apagado no destino", as opções são:

- **Copiar de novo**: a origem é copiada para o destino.
- **Apagar na origem**: o arquivo da origem vai para a Lixeira do Windows.
- **Decidir depois**.

## Cópia segura

1. O programa copia para `<nome>.backup-leve-parcial`.
2. No fim da cópia, o programa grava a data de modificação da origem.
3. O programa renomeia o arquivo parcial para o nome final.
4. O programa grava o registro no índice.

Se a energia cai no meio, o arquivo final não fica corrompido.
Na próxima execução, o programa apaga os arquivos `.backup-leve-parcial` e continua de onde parou.

## Quando a execução para

O programa para antes de copiar qualquer arquivo nestes casos:

| Situação | Motivo |
|---|---|
| A pasta de origem não existe. | Sem origem, não há o que copiar. |
| O disco de destino não está conectado. | O programa não cria pastas em outro disco por engano. |
| A origem e o destino ficam um dentro do outro. | O backup copiaria a si mesmo sem fim. |
| O destino já recebe o backup de outra origem. | Dois backups no mesmo índice se misturam. |

Um erro em um só arquivo não para a execução. O arquivo vai para a lista de erros, e a próxima execução tenta de novo.

## Pontos abertos

- Discos FAT32 trocam a hora no horário de verão. Uma diferença de exatamente 1 hora com tamanho igual pode ser tratada como "igual". Decidir com testes reais.
- Arquivos abertos e bloqueados por outro programa (Outlook .pst, por exemplo): hoje a regra é listar como "não copiado" e tentar na próxima execução.
- Links simbólicos e junções: hoje a regra é não seguir.
