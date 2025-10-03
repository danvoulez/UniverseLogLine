# 🚂 Railway Manual Deploy - LogLine Universe

**EXECUTE AGORA:** Deploy manual via Railway web interface

## 🎯 **PASSO A PASSO SIMPLIFICADO**

### **1. Acesse Railway**
👉 **https://railway.app/dashboard**
- Login: `dan@danvoulez.com`
- Token: `4bc289e6-a584-4725-970b-b9668b28a26a`

### **2. Criar Projeto**
- **New Project** → **Deploy from GitHub repo**
- Repository: `danvoulez/UniverseLogLine`
- Name: `logline-universe`

### **3. Adicionar Databases (2 serviços)**
- **Add Service** → **Database** → **PostgreSQL**
- **Add Service** → **Database** → **Redis**

### **4. Deploy Services (6 serviços)**

Para cada serviço, **Add Service** → **GitHub Repo** → `danvoulez/UniverseLogLine`:

#### **Service 1: logline-gateway**
```
Environment Variables:
SERVICE=logline-gateway
SERVICE_PORT=8080
RUST_LOG=info
GATEWAY_JWT_SECRET=QQc4wg9wUweVsyWwHUOT5RvXqdz1Q/I8ARJL06ZgfRI=
GATEWAY_JWT_ISSUER=logline-id://tenant/danvoulez.foundation
GATEWAY_JWT_AUDIENCE=logline-universe
GATEWAY_ALLOWED_ORIGINS=*
GATEWAY_PUBLIC_PATHS=/healthz,/health,/docs
```

#### **Service 2: logline-id**
```
Environment Variables:
SERVICE=logline-id
SERVICE_PORT=8080
RUST_LOG=info
```

#### **Service 3: logline-timeline**
```
Environment Variables:
SERVICE=logline-timeline
SERVICE_PORT=8080
RUST_LOG=info
DATABASE_URL=${{Postgres.DATABASE_URL}}
```

#### **Service 4: logline-rules**
```
Environment Variables:
SERVICE=logline-rules
SERVICE_PORT=8080
RUST_LOG=info
```

#### **Service 5: logline-engine**
```
Environment Variables:
SERVICE=logline-engine
SERVICE_PORT=8080
RUST_LOG=info
```

#### **Service 6: logline-federation**
```
Environment Variables:
SERVICE=logline-federation
SERVICE_PORT=8080
RUST_LOG=info
FEDERATION_NODE_NAME=railway-production
```

### **5. Aguardar Deploy**
- Todos os 6 serviços devem ficar **"Active"**
- Anotar a URL do **logline-gateway**

### **6. Executar Script de Identidades**
```bash
chmod +x create-logline-ids.sh
./create-logline-ids.sh https://logline-gateway-[sua-url].railway.app
```

## 🎉 **RESULTADO ESPERADO**
- ✅ 8 serviços rodando (6 LogLine + 2 databases)
- ✅ Gateway respondendo em `/health`
- ✅ Identidades @danvoulez e @cascade criadas
- ✅ Tenants @danvoulez.foundation e @logline.universe
- ✅ Primeiro span histórico registrado

**VAMOS FAZER HISTÓRIA COMPUTACIONAL! 🌟**
