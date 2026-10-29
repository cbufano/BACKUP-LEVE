# ADR 0002 — Execução sob demanda, sem serviço

- Data: 2026-10-29
- Estado: aceito

## Contexto

O dono do projeto não quer um programa sempre ligado na máquina.
O usuário chama o programa, ele sincroniza e fecha.
A primeira carga é pesada. As seguintes são leves, porque só copiam o que mudou.

## Decisão

1. O programa não instala serviço do Windows e não fica na bandeja do sistema.
2. Execução manual: o usuário abre a janela e clica em "Sincronizar".
3. Execução agendada (opcional): o programa cria uma tarefa no **Agendador de Tarefas do Windows**.
   A tarefa chama `backup-leve.exe --perfil "<nome>" --silencioso` no horário escolhido.
4. No modo silencioso, o programa roda sem janela, com prioridade baixa de CPU e de disco.
   No fim, mostra uma notificação do Windows e fecha.
5. Se o disco de destino não está conectado, o programa registra o fato e fecha sem erro.

## Consequências

- Entre uma execução e outra, o programa não usa memória nem CPU.
- O programa não detecta alterações em tempo real. Isso é intencional.
- Um modo serviço pode vir no futuro, depois que o motor estiver estável.
