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
| Manter os dois | No destino ficam as duas versões. A versão do destino recebe a data de modificação no nome: `relatorio (destino 2026-10-29 09-12).docx`. A versão da origem é copiada com o nome original. A origem não muda. |
| Decidir depois | Nada muda. O conflito aparece de novo na próxima execução. |

O programa não precisa gravar os conflitos. A cada execução, ele compara de novo o estado atual com o índice, e o conflito aparece outra vez.
Por isso, depois de uma execução agendada (sem janela), a tela de conflitos aparece na próxima vez que o usuário abrir o programa.

Nota: em "Manter os dois", a cópia da origem fica com o nome original. Assim, o backup continua a seguir o arquivo da origem nas próximas execuções. A versão com data fica fora do índice e o programa não mexe mais nela.

Antes de aplicar uma escolha, o programa confere o arquivo de novo. Se o arquivo mudou depois da verificação, o programa não faz nada e mostra o motivo.

Cada lote de escolhas fica registrado em `<destino>\.backup-leve\relatorios\decisoes_<data>.json`.

Para o conflito "apagado no destino", as opções são:

- **Copiar de novo**: a origem é copiada para o destino.
- **Apagar na origem**: o arquivo da origem vai para a Lixeira do Windows.
- **Decidir depois**.

## O que fica de fora

Algumas pastas são criadas de novo por um programa. Exemplo: o `npm install` cria a pasta `node_modules` a partir do `package.json`.
Essas pastas são grandes e têm muitos arquivos. O backup fica mais rápido sem elas.

### Pastas recriáveis

A opção "Pular as pastas que um programa cria de novo" vem ligada em todo perfil.
O programa pula uma pasta só quando encontra, na mesma pasta-mãe, o arquivo que a recria.
Uma pasta `node_modules` sem `package.json` ao lado é do usuário. O programa copia essa pasta.

| Pasta | Só quando ao lado existe | Quem cria de novo |
|---|---|---|
| `node_modules`, `.next`, `.nuxt`, `.svelte-kit`, `.angular`, `.astro`, `.turbo`, `.parcel-cache`, `.vite`, `.expo` | `package.json` | O gerenciador de pacotes JavaScript ou o framework. |
| `dist`, `build`, `out`, `.output` | `package.json` e um arquivo de trava (`package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `bun.lockb` ou `bun.lock`) | O build do projeto JavaScript. |
| `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache` | (sempre) | Python e as ferramentas dele. |
| `.venv`, `venv` | `requirements.txt`, `pyproject.toml`, `setup.py` ou `Pipfile` | O pip, a partir da lista de pacotes. |
| `target` | `Cargo.toml` | O cargo (Rust). |
| `build`, `.gradle` | `build.gradle`, `build.gradle.kts`, `settings.gradle` ou `settings.gradle.kts` | O Gradle (Android). |
| `bin`, `obj` | um arquivo `*.csproj`, `*.vbproj` ou `*.fsproj` | O Visual Studio (.NET). |
| `.vs` | um arquivo `*.sln` | O Visual Studio. |
| `.dart_tool`, `build` | `pubspec.yaml` | O Flutter. |
| `Pods` | `Podfile` | O CocoaPods (iOS). |
| Qualquer nome | Um arquivo `CACHEDIR.TAG` válido dentro da pasta | O programa que marcou a pasta como cache. |

Nota: a pasta `dist` só fica de fora com o arquivo de trava. Um plugin do WordPress pode ter `package.json` e precisar da pasta `dist` que veio com ele.

Nota: a pasta `vendor` não está na lista. Os plugins do WordPress guardam nela código necessário.

Nota: um ambiente `.venv` sem lista de pacotes fica no backup, mesmo com `CACHEDIR.TAG`. Sem a lista, ninguém sabe o que instalar de novo.

### Arquivos temporários

A opção "Pular os arquivos temporários" vem ligada em todo perfil.
Um programa cria esses arquivos enquanto trabalha e não precisa deles depois.

| Nome | O que é |
|---|---|
| `*.tmp`, `*.temp` | Arquivo temporário. |
| `~$*` | Trava que o Word e o Excel deixam ao lado de um documento aberto. |
| `*.crdownload` | Download do Chrome que não terminou. |
| `*.swp`, `*.swo` | Cópia de trabalho do Vim. |
| `*.pyc`, `*.pyo` | Código Python compilado. |
| `.DS_Store` | Dados de pasta do macOS. |

### Backup completo

O botão "Backup completo (100%)" desliga as duas opções e apaga a lista de nomes do perfil.
Assim o programa copia todos os arquivos. O botão "Voltar a pular" liga as duas opções de novo.

### Sugestões da IA

O botão "Sugerir com IA" pede a um serviço de IA nomes para pular. Veja o [ADR 0004](adr/0004-sugestoes-por-ia.md).

1. O programa lê as origens e faz um resumo: as 60 pastas e as 60 terminações que ocupam mais espaço.
2. O resumo tem só nomes, quantidades e tamanhos. O conteúdo dos arquivos não sai do computador.
3. O que o perfil já pula não entra no resumo.
4. A IA devolve nomes com o motivo. O programa descarta nome inválido e nome que o perfil já pula.
5. O usuário marca os nomes que aceita. Só os nomes marcados entram na lista do perfil.

Nota: a IA nunca decide sozinha. Uma sugestão sem a marca do usuário não muda o backup.

### Nomes escolhidos pelo usuário

O perfil aceita outros nomes para pular, um por linha. Exemplos: `Downloads`, `*.log` e `Cópia de*`.
`*.log` pula os nomes que terminam em `.log`. `Cópia de*` pula os nomes que começam com `Cópia de`.
O nome vale para arquivos e pastas, em qualquer lugar da origem. A comparação não diferencia maiúsculas de minúsculas.
Um nome com `/` ou `\` não vale.

### Como as pastas de fora entram na comparação

1. A origem decide quais pastas ficam de fora. A leitura do destino também pula essas pastas.
2. Uma pasta que só as regras do destino deixam de fora fica de fora nos dois lados.
3. Os registros do índice dentro de uma pasta de fora saem do índice.
4. As cópias que já estão no destino ficam lá. O programa não apaga nada e não mostra conflito.
5. O relatório lista as pastas de fora e diz quem cria cada uma de novo.

Se o usuário desliga a opção depois, a pasta volta a ser copiada. As cópias antigas iguais à origem são registradas sem cópia.

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

O usuário pode parar a execução a qualquer momento:

| Momento | O que acontece |
|---|---|
| Durante a leitura das pastas | O programa não decide nada sobre os arquivos. O índice fica como estava. |
| Durante a cópia | O arquivo pela metade é apagado. Os arquivos já copiados ficam no índice. |

Nos dois casos, o relatório marca a execução como interrompida. A próxima execução copia só o que falta.

Se o programa fecha de repente (queda de energia, por exemplo), até um segundo de cópias fica sem registro no índice. A próxima execução encontra esses arquivos iguais à origem e registra cada um sem copiar de novo.

## Pontos abertos

- Discos FAT32 trocam a hora no horário de verão. Uma diferença de exatamente 1 hora com tamanho igual pode ser tratada como "igual". Decidir com testes reais.
- Arquivos abertos e bloqueados por outro programa (Outlook .pst, por exemplo): hoje a regra é listar como "não copiado" e tentar na próxima execução.
- Links simbólicos e junções: hoje a regra é não seguir.
