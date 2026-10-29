# Backup Leve

Backup de pastas para um disco externo, leve e seguro.
Você escolhe as pastas, o programa copia, lembra o que copiou e avisa quando algo no disco de backup mudou.

> **Estado:** em desenvolvimento (pré-alfa). O motor funciona pela linha de comando. A janela ainda não existe. Ainda não use em dados importantes.

*English summary: lightweight, on-demand folder backup to an external drive. It keeps an index of every copied file, never overwrites silently, and asks the user to resolve files changed on the backup drive. Built with Rust + Tauri. Docs are in Portuguese for now.*

## O que o programa faz

- Copia as pastas escolhidas para o disco de destino.
- Guarda um índice de cada arquivo copiado. A segunda execução só copia o que mudou.
- Não copia de novo o que já existe igual no destino.
- Detecta arquivos alterados ou apagados no disco de destino. Esses arquivos não são tocados.
- No fim, mostra os conflitos. Para cada um, você escolhe:
  - **A origem vence**
  - **O destino vence**
  - **Manter os dois**, com a data no nome de cada versão
  - **Decidir depois**
- Nunca apaga dado de forma definitiva. Versões substituídas vão para uma pasta de segurança ou para a Lixeira.
- Grava um relatório de cada execução no disco de destino.

## O que o programa não faz

- Não fica rodando na máquina. Não é um serviço. Ele abre, sincroniza e fecha.
- Não monitora alterações em tempo real.
- Não envia nada para a internet.

## Como ele roda

| Forma | Como |
|---|---|
| Manual | Abra o programa e clique em **Sincronizar**. |
| Agendada | O programa cria uma tarefa no Agendador de Tarefas do Windows. No horário, ele roda sem janela, mostra uma notificação no fim e fecha. |

## Modos

| Modo | Arquivo alterado na origem |
|---|---|
| Somente novos | Não é copiado. Aparece na lista "alterados na origem". |
| Sincronizar | A versão nova vai para o destino. A versão antiga do destino vai para a pasta de segurança. |

As regras completas estão em [docs/REGRAS-DE-SINCRONISMO.md](docs/REGRAS-DE-SINCRONISMO.md).

## Tecnologia

- Motor em **Rust**
- Interface em **Tauri 2** + **React** + **TypeScript**
- Índice em **SQLite**, gravado no próprio disco de destino

O motivo da escolha está em [docs/adr/0001-tauri-rust-react.md](docs/adr/0001-tauri-rust-react.md).

## Compilar

Requisitos:

- [Rust](https://rustup.rs) estável
- [Node.js](https://nodejs.org) 20 ou mais novo (para a interface)
- Windows: Ferramentas de Build do Visual Studio com C++ e o WebView2 (já vem no Windows 11)

Passos:

1. Clone o repositório.
2. Rode os testes: `cargo test --workspace`.
3. Compile o programa: `cargo build --release`.

O programa fica em `target/release/backup-leve.exe`. A interface ainda não existe. Veja [app/README.md](app/README.md).

## Usar pela linha de comando

```
backup-leve sincronizar --origem "D:\Documentos" --destino "E:\Backup\Documentos"
```

| Opção | Efeito |
|---|---|
| `--modo sincronizar` | Copia arquivos novos e atualiza os alterados na origem. É o padrão. |
| `--modo somente-novos` | Copia só arquivos novos. Os alterados na origem aparecem no relatório. |
| `--simulacao` | Mostra o que seria feito. Não copia nada e não cria nada no destino. |

No fim, o programa mostra um resumo e grava o relatório em `<destino>\.backup-leve\relatorios\`.

| Código de saída | Sentido |
|---|---|
| 0 | Tudo certo. |
| 1 | A execução parou antes de copiar (por exemplo, disco de destino desconectado). |
| 3 | Há conflitos para decidir. |
| 4 | Alguns arquivos não foram copiados. A próxima execução tenta de novo. |

## Documentação

| Documento | Assunto |
|---|---|
| [docs/REGRAS-DE-SINCRONISMO.md](docs/REGRAS-DE-SINCRONISMO.md) | O que o programa faz com cada arquivo |
| [docs/ARQUITETURA.md](docs/ARQUITETURA.md) | Como o código está organizado |
| [docs/ROADMAP.md](docs/ROADMAP.md) | O que já existe e o que vem depois |
| [docs/adr/](docs/adr/) | Decisões de projeto |

## Contribuir

Leia [CONTRIBUTING.md](CONTRIBUTING.md). Issues em português ou em inglês são bem-vindas.

## Licença

[MIT](LICENSE)
