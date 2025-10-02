# LogLine Network OS — Arquitetura Final

Este workspace materializa a arquitetura final descrita no documento "ChatGPT-Projeto LogLine OS": o bundle V3 (camadas/enzimas/campo magnético), o onboarding plug-and-play V3.1+ e as extensões avançadas V4.

## Estrutura
- `modules/`: 64 módulos `.lll` (identidade, rede, enzimas, consenso, onboarding, compliance, etc.).
- `manifests/`: manifestos `project` para os bundles V3, V3.1, V3.1+, V4 e versões intermediárias.
- `scripts/instant_run.sh`: instalador one-liner sugerido no documento.
- `docs/architecture.mmd`: diagrama Mermaid da topologia em camadas.

## Uso previsto
1. Certifique-se de ter o binário `logline` instalado e no `PATH`.
2. Aponte o diretório de módulos, por exemplo: `export LOGLINE_MODULE_PATH=$(pwd)/modules`.
3. Faça o deploy do bundle desejado, ex.: `logline deploy manifests/logline_orchestration_network_v4.lll --target local`.
4. Rode os ciclos de orquestração ou APIs conforme os módulos (`logline run orchestration_multi.global_cycle --tenant_id voulezvous`, etc.).

Os manifestos respeitam as dependências entre bundles, de forma que V4 incorpora V3 + V3.1. Ajuste paths/conexões conforme a sua instalação real do `logline`.

## Empacotamento
Use `scripts/build_bundle.py` para gerar pacotes `.lll.zip` com as dependências do manifesto escolhido:

```bash
./scripts/build_bundle.py logline_orchestration_network_v4
# opcional: ./scripts/build_bundle.py logline_onboarding_v31_plus --no-extra
```

Para embutir o executável e criar um pacote instalável, forneça o caminho do binário:

```bash
./scripts/build_bundle.py logline_orchestration_network_v4 \
  --binary /usr/local/bin/logline \
  --output dist/logline_orchestration_network_v4_with_bin.lll.zip
```

Os arquivos são salvos em `dist/`. Cada pacote inclui o manifesto alvo, dependências, módulos necessários e (por padrão) extras como `instant_run.sh`, perfis `staging/prod` e o diagrama.

## Instalação do bundle
Após extrair o pacote `.lll.zip`, rode `installer/install.sh` apontando o destino (padrão `/opt/logline`). O script copia binário, manifestos, módulos, docs e perfis:

```bash
unzip logline_orchestration_network_v4.lll.zip
cd logline_orchestration_network_v4
./installer/install.sh /opt/logline
```

Depois disso, garanta que `/opt/logline/bin` esteja no `PATH` e exporte `LOGLINE_MODULE_PATH=/opt/logline/modules` antes de usar o CLI.
