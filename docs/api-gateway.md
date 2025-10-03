# LogLine API Gateway

O serviço **logline-gateway** centraliza o tráfego REST e WebSocket do LogLine Universe,
funcionando como ponto único de entrada para clientes externos e coordenando a
comunicação entre os microserviços internos.

## Objetivos

- Expor uma fachada HTTP única com roteamento / proxy para engine, rules, timeline,
  identidade e federation.
- Encaminhar mensagens em tempo real entre clientes externos e a malha de serviços
  usando o envelope `ServiceMessage`.
- Monitorar a saúde dos peers e publicar um estado agregado no endpoint `/healthz`.
- Detectar desconexões da malha e restabelecer conexões automaticamente.

## Visão Geral

```mermaid
graph LR
    subgraph Clients
        A[REST Client]
        B[WebSocket Client]
    end

    subgraph Gateway
        G1[REST Proxy]
        G2[WS Hub]
        G3[Health & Discovery]
    end

    subgraph Services
        S1[logline-engine]
        S2[logline-rules]
        S3[logline-timeline]
        S4[logline-id]
        S5[logline-federation]
    end

    A -->|/engine, /rules, ...| G1
    B -->|/ws| G2
    G1 -->|HTTP| S1
    G1 -->|HTTP| S2
    G1 -->|HTTP| S3
    G1 -->|HTTP| S4
    G1 -->|HTTP| S5
    G2 <-->|ServiceMessage WS| S1
    G2 <-->|ServiceMessage WS| S2
    G2 <-->|ServiceMessage WS| S3
    G3 -->|/healthz| Clients
```

## Exemplos de uso

### Proxy REST

```bash
curl -X POST \
  "http://localhost:8070/rules/tenants/acme/rules" \
  -H "Content-Type: application/json" \
  -d '{"rule_id":"notify","definition":{...}}'
```

O gateway encaminha a requisição para `logline-rules`, preservando método, headers
(Public/Host é reescrito) e corpo.

### Hub WebSocket

1. Conecte-se ao gateway:

```bash
wscat -c ws://localhost:8070/ws
```

2. Envie um envelope `ServiceMessage`:

```json
{"event":"span_created","payload":{"type":"span_created","span_id":"abc","span":{"name":"demo"}}}
```

O gateway roteará automaticamente a mensagem para `logline-timeline` e `logline-rules`
e distribuirá o envelope para outros clientes conectados.

## Health check e reconexão

- O endpoint `/healthz` consulta o `/health` de cada serviço interno via `reqwest` e
  consolida o resultado em JSON (status `ok` ou `degraded`).
- A malha WebSocket utiliza `ServiceMeshClient` do `logline-core` para manter
  conexões persistentes. Caso um peer caia, o cliente realiza _backoff_ exponencial
  e reconecta automaticamente, emitindo `ConnectionLost` para os consumidores.

## Executando localmente

```bash
cargo run -p logline-gateway
```

Variáveis de ambiente relevantes:

| Variável            | Descrição                              | Default                  |
|---------------------|----------------------------------------|--------------------------|
| `GATEWAY_BIND`      | Endereço de escuta do gateway          | `0.0.0.0:8070`           |
| `ENGINE_URL`        | Base HTTP do logline-engine            | `http://127.0.0.1:8090`  |
| `RULES_URL`         | Base HTTP do logline-rules             | `http://127.0.0.1:8081`  |
| `TIMELINE_URL`      | Base HTTP do logline-timeline          | `http://127.0.0.1:8082`  |
| `ID_URL`            | Base HTTP do logline-id                | `http://127.0.0.1:8083`  |
| `FEDERATION_URL`    | Base HTTP do logline-federation        | `http://127.0.0.1:8084`  |
| `*_WS_URL`          | (Opcional) URL WebSocket específica    | Derivada automaticamente |

Ao iniciar, o gateway registrará os peers ativos e publicará logs usando `tracing`.
