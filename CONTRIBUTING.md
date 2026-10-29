# Como contribuir

Obrigado pelo interesse. Este é um programa de backup. Um erro aqui pode fazer alguém perder arquivos.
Por isso, a regra mais importante é: **toda mudança de comportamento começa na especificação**.

## Antes de começar

1. Leia [docs/REGRAS-DE-SINCRONISMO.md](docs/REGRAS-DE-SINCRONISMO.md).
2. Procure uma issue aberta sobre o assunto. Se não existe, abra uma.
3. Para mudança grande, espere uma resposta na issue antes de escrever o código.

## Ambiente

1. Instale o Rust estável com o [rustup](https://rustup.rs).
2. Instale o Node.js 20 ou mais novo (só para a interface).
3. No Windows, instale as Ferramentas de Build do Visual Studio com C++.

## Antes de abrir o pull request

Rode os três comandos. Os três precisam passar:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Regras do código

- O motor (`crates/engine`) não depende da interface.
- A tabela de decisão (`decision.rs`) não faz I/O. Toda regra nova recebe um teste.
- Se você muda uma regra, mude também `docs/REGRAS-DE-SINCRONISMO.md` no mesmo pull request.
- Nenhum código apaga arquivo do usuário de forma definitiva.
- Nomes no código em inglês. Documentação em português.

## Mensagens de commit

Use o padrão [Conventional Commits](https://www.conventionalcommits.org/pt-br/):

```
feat(engine): adiciona varredura com filtros
fix(copy): preserva a data de modificação no exFAT
docs: explica a tela de conflitos
```

## Conduta

Este projeto segue o [Código de Conduta](CODE_OF_CONDUCT.md).
