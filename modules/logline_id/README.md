# LogLine ID - Módulo Computável

Sistema completo de identidade descentralizada para a rede LogLine, implementado como módulo computável com contratos, esquemas JSON, lógica em Rust e componentes de UI.

## 📁 Estrutura do Módulo

```
modules/logline_id/
├── schema/
│   └── logline_id.schema.json          # Esquema JSON Draft-07 para validação
├── contracts/
│   ├── identity.register.lll           # Contrato de registro de identidade
│   ├── ghost.claim.lll                # Contrato para identidades ghost
│   └── verify_logline_id_signature.lll # Contrato de verificação de assinatura
├── receipts/
│   └── identity_issued_receipt.schema.json # Esquema para recibos de auditoria
├── logic/
│   ├── mod.rs                         # Módulo principal em Rust
│   ├── signature.rs                   # Operações criptográficas ed25519
│   ├── ghost.rs                       # Gerenciamento de identidades ghost
│   └── federation.rs                  # Sincronização entre nós
├── ui/
│   ├── LogLineIDBadge.tsx             # Componente badge de identidade
│   ├── LogLineIDBadge.css             # Estilos do badge
│   ├── GhostAlert.tsx                 # Alertas de identidades ghost
│   ├── GhostAlert.css                 # Estilos dos alertas
│   ├── AuthCard.tsx                   # Card de autenticação
│   ├── AuthCard.css                   # Estilos do card
│   └── index.ts                       # Exportações principais
├── test_local.js                      # Runner de testes locais
└── README.md                          # Este arquivo
```

## 🚀 Características Principais

### ✅ Sistema de Identidade Completo
- **Registro descentralizado** de identidades com chaves ed25519
- **Validação por esquema JSON** Draft-07 compatível
- **Auditoria completa** com recibos criptográficos
- **Federação multi-nó** com sincronização automática

### 👻 Identidades Ghost
- **Identidades temporárias** para privacidade
- **Expiração automática** configurável
- **Herança de permissões** da identidade principal
- **Alertas visuais** em tempo real

### 🔐 Criptografia Robusta
- **Chaves ed25519** para alta segurança
- **Assinaturas verificáveis** off-chain e on-chain
- **Geração de pares de chaves** automática
- **Suporte a chaves protegidas por senha**

### 🎨 Interface Moderna
- **Componentes React/TypeScript** prontos para produção
- **3 variantes de badge**: minimal, compact, full
- **Sistema de alertas** para identidades ghost
- **Card de autenticação** completo com drag-and-drop

## 📋 Requisitos

### Desenvolvimento
- **Node.js** v16+ (para UI e testes)
- **Rust** 1.70+ (para lógica de backend)
- **OpenSSL** (para geração de chaves nos testes)

### Produção
- **LogLine Runtime** com suporte a contratos .lll
- **PostgreSQL** (opcional, para persistência)
- **React** 18+ (para componentes UI)

## 🧪 Testes Locais

Execute o teste completo do sistema:

```bash
cd modules/logline_id/
node test_local.js
```

### O que é testado:
1. ✅ **Geração de chaves** ed25519
2. ✅ **Carregamento de esquema** JSON
3. ✅ **Criação de identidade** com validação
4. ✅ **Validação contra esquema** JSON Draft-07
5. ✅ **Simulação de contrato** de registro
6. ✅ **Criação de identidade ghost** temporária
7. ✅ **Verificação de assinatura** criptográfica
8. ✅ **Geração de relatório** de testes

### Exemplo de execução:
```
🚀 Starting LogLine ID Module Tests
Testing alias: @danvoulez

📋 Generating test key pair...
✓ Private key: /path/to/test_key.pem
✓ Public key: /path/to/test_key.pub

📋 Loading LogLine ID schema...
✓ Schema loaded successfully

📋 Creating test identity...
✓ Identity created: /path/to/test_identity.json
  Alias: @danvoulez
  ID: logline_id_1703123456789

📋 Validating identity against schema...
✓ Identity validation passed

📋 Testing identity registration contract...
✓ Contract loaded successfully
✓ Registration receipt created: /path/to/registration_receipt.json
  Transaction ID: tx_1703123456789

📋 Testing ghost identity creation...
✓ Ghost identity created: /path/to/ghost_identity.json
  Ghost Alias: @danvoulez_ghost_1703123456789
  Expires: 2024-01-01T12:00:00.000Z

📋 Testing signature verification...
✓ Signature verification passed
  Message: LogLine ID Test - danvoulez - 1703123456789

📋 Generating test report...
✓ Test report generated: /path/to/test_report.json

==================================================
TEST SUMMARY
==================================================
Total Tests: 7
Passed: 7
Failed: 0
==================================================

✅ All tests completed!
🎉 All tests passed!
```

## 💻 Uso dos Componentes UI

### LogLine ID Badge

```tsx
import { LogLineIDBadge } from './ui';

// Badge minimal
<LogLineIDBadge 
  identity={identity}
  variant="minimal"
/>

// Badge completo com ações
<LogLineIDBadge 
  identity={identity}
  variant="full"
  onVerify={handleVerify}
  onRevoke={handleRevoke}
  expandable={true}
/>
```

### Ghost Alerts

```tsx
import { GhostAlerts } from './ui';

<GhostAlerts 
  alerts={ghostAlerts}
  position="top-right"
  onDismiss={handleDismiss}
  onRevoke={handleRevoke}
  maxVisible={5}
/>
```

### Authentication Card

```tsx
import { AuthCard } from './ui';

<AuthCard 
  mode="login"
  onAuth={handleAuth}
  onCancel={handleCancel}
  loading={isLoading}
  error={authError}
/>
```

## 🔧 Integração com LogLine Runtime

### Registro de Identidade

```rust
use logline_id::logic::{signature, identity};

// Carregar chave privada
let private_key = signature::load_private_key("path/to/key.pem")?;

// Criar identidade
let identity = identity::create_identity(
    "danvoulez".to_string(),
    private_key.public_key(),
    identity::OwnerType::Individual,
)?;

// Registrar na rede
let receipt = runtime.execute_contract(
    "identity.register.lll",
    &identity,
)?;
```

### Identidade Ghost

```rust
use logline_id::logic::ghost;

// Criar identidade ghost
let ghost = ghost::create_ghost_identity(
    &parent_identity,
    "temp_alias".to_string(),
    Duration::hours(24),
    "Private transaction".to_string(),
)?;

// Auto-expiração
ghost.schedule_expiration(&runtime)?;
```

### Verificação de Assinatura

```rust
use logline_id::logic::signature;

// Verificar assinatura
let is_valid = signature::verify_signature(
    &message,
    &signature_bytes,
    &identity.public_key,
)?;

if is_valid {
    println!("✅ Assinatura válida para @{}", identity.alias);
}
```

## 📊 Esquema de Dados

### Identidade Principal

```json
{
  "id": "logline_id_1703123456789",
  "alias": "danvoulez",
  "owner_type": "individual",
  "public_key": "ed25519:ABC123...",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z",
  "status": "active",
  "roles": ["user", "developer"],
  "capabilities": ["sign", "verify", "ghost_create"],
  "federation_level": "global",
  "metadata": {
    "display_name": "Dan Voulez",
    "bio": "LogLine developer"
  }
}
```

### Identidade Ghost

```json
{
  "id": "ghost_1703123456789",
  "alias": "danvoulez_temp_12345",
  "owner_type": "ghost",
  "parent_id": "logline_id_1703123456789",
  "expires_at": "2024-01-02T00:00:00Z",
  "purpose": "Private transaction",
  "trust_level": 75,
  "permissions": ["sign", "verify"]
}
```

## 🔐 Segurança

### Práticas Implementadas
- ✅ **Chaves ed25519** nunca trafegam pela rede
- ✅ **Assinaturas verificáveis** off-chain
- ✅ **Esquemas JSON** para validação rigorosa
- ✅ **Auditoria completa** com recibos criptográficos
- ✅ **Identidades ghost** com expiração automática
- ✅ **Validação de alias** contra squatting
- ✅ **Revogação segura** de identidades

### Considerações de Privacidade
- **Identidades ghost** para transações privadas
- **Metadata opcional** para informações pessoais
- **Federation levels** para controle de visibilidade
- **Chaves locais** nunca enviadas ao servidor

## 🌐 Integração em Produção

### 1. Configuração do Runtime
```rust
// Cargo.toml
[dependencies]
logline-id = { path = "modules/logline_id/logic" }
serde_json = "1.0"
ed25519-dalek = "2.0"
```

### 2. Inicialização do Módulo
```rust
use logline_id::LogLineIDModule;

let id_module = LogLineIDModule::new(config)?;
runtime.register_module("logline_id", id_module)?;
```

### 3. Deploy dos Contratos
```bash
# Deploy contratos para a rede
logline deploy modules/logline_id/contracts/identity.register.lll
logline deploy modules/logline_id/contracts/ghost.claim.lll  
logline deploy modules/logline_id/contracts/verify_logline_id_signature.lll
```

### 4. Configuração da UI
```tsx
import { LogLineIDUtils, defaultConfig } from 'logline-id/ui';

const config = {
  ...defaultConfig,
  apiEndpoint: 'https://api.logline.network',
  networkId: 'mainnet'
};
```

## 📈 Roadmap

### ✅ Fase 1 - Implementação Base (Concluída)
- [x] Esquemas JSON Draft-07
- [x] Contratos .lll para operações básicas
- [x] Lógica Rust para criptografia
- [x] Componentes UI React/TypeScript
- [x] Sistema de testes local

### 🔄 Fase 2 - Integração (Em Progresso)
- [ ] Integração com LogLine Runtime
- [ ] Persistência PostgreSQL
- [ ] API REST para operações
- [ ] WebSocket para atualizações em tempo real
- [ ] Testes de integração completos

### 🔮 Fase 3 - Recursos Avançados (Planejado)
- [ ] Multi-sig para identidades organizacionais
- [ ] Recuperação de chaves com threshold
- [ ] Integração com hardware wallets
- [ ] Métricas e observabilidade
- [ ] Mobile SDK (React Native)

## 🤝 Contribuição

Este módulo é parte do projeto LogLine e aceita contribuições:

1. **Issues**: Reporte bugs ou sugira melhorias
2. **Pull Requests**: Submeta código seguindo os padrões
3. **Testes**: Adicione testes para novos recursos
4. **Documentação**: Melhore a documentação

## 📜 Licença

Parte do projeto LogLine - Consulte a licença principal do projeto.

---

**Desenvolvido com ❤️ para a rede LogLine**

*Este módulo representa um ano de desenvolvimento e pesquisa em sistemas de identidade descentralizada. Obrigado por fazer parte desta jornada!*