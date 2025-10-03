# 🚂 Railway Deployment Guide - LogLine Universe

**Email**: dan@danvoulez.com  
**Token**: 4bc289e6-a584-4725-970b-b9668b28a26a  
**Strategy**: LogLine ID First - Deploy with all identities pre-planned

---

## 🎯 **DEPLOYMENT SEQUENCE**

### **Step 1: Create Railway Project**

1. **Go to**: https://railway.app/dashboard
2. **Login** with your account (dan@danvoulez.com)
3. **Create New Project**:
   - Name: `logline-universe`
   - Description: `Distributed logging and identity system with computational memory`
   - Repository: `danvoulez/UniverseLogLine`

### **Step 2: Setup Databases First**

#### **PostgreSQL Database**
1. **Add Service** → **Database** → **PostgreSQL**
2. **Name**: `logline-postgres`
3. **Plan**: Hobby (free tier)
4. **Auto-generate**: Database credentials
5. **Note the DATABASE_URL** (will be auto-provided)

#### **Redis Cache**
1. **Add Service** → **Database** → **Redis**
2. **Name**: `logline-redis`
3. **Plan**: Hobby (free tier)
4. **Note the REDIS_URL** (will be auto-provided)

### **Step 3: Deploy Core Services (6 services)**

For each service, create a new **Service** → **GitHub Repo**:

#### **Service 1: LogLine Gateway** 🚪
```
Name: logline-gateway
Repository: danvoulez/UniverseLogLine
Build Command: (empty - uses Dockerfile)
Start Command: (empty - uses Dockerfile)

Environment Variables:
SERVICE=logline-gateway
SERVICE_PORT=8080
RUST_LOG=info
GATEWAY_BIND=0.0.0.0:8080
GATEWAY_JWT_SECRET=QQc4wg9wUweVsyWwHUOT5RvXqdz1Q/I8ARJL06ZgfRI=
GATEWAY_JWT_ISSUER=logline-id://tenant/danvoulez.foundation
GATEWAY_JWT_AUDIENCE=logline-universe
GATEWAY_ALLOWED_ORIGINS=*
GATEWAY_PUBLIC_PATHS=/healthz,/health,/docs
ENGINE_URL=http://logline-engine.railway.internal:8080
RULES_URL=http://logline-rules.railway.internal:8080
TIMELINE_URL=http://logline-timeline.railway.internal:8080
ID_URL=http://logline-id.railway.internal:8080
FEDERATION_URL=http://logline-federation.railway.internal:8080

Port: 8080 (Railway will auto-assign external port)
```

#### **Service 2: LogLine Identity** 🆔
```
Name: logline-id
Repository: danvoulez/UniverseLogLine

Environment Variables:
SERVICE=logline-id
SERVICE_PORT=8080
RUST_LOG=info

Port: 8080
```

#### **Service 3: LogLine Timeline** 📊
```
Name: logline-timeline
Repository: danvoulez/UniverseLogLine

Environment Variables:
SERVICE=logline-timeline
SERVICE_PORT=8080
RUST_LOG=info
TIMELINE_DATABASE_URL=${{logline-postgres.DATABASE_URL}}

Port: 8080
```

#### **Service 4: LogLine Rules** 📋
```
Name: logline-rules
Repository: danvoulez/UniverseLogLine

Environment Variables:
SERVICE=logline-rules
SERVICE_PORT=8080
RUST_LOG=info

Port: 8080
```

#### **Service 5: LogLine Engine** ⚙️
```
Name: logline-engine
Repository: danvoulez/UniverseLogLine

Environment Variables:
SERVICE=logline-engine
SERVICE_PORT=8080
RUST_LOG=info
RULES_URL=http://logline-rules.railway.internal:8080
TIMELINE_WS_URL=ws://logline-timeline.railway.internal:8080/ws/service

Port: 8080
```

#### **Service 6: LogLine Federation** 🌐
```
Name: logline-federation
Repository: danvoulez/UniverseLogLine

Environment Variables:
SERVICE=logline-federation
SERVICE_PORT=8080
RUST_LOG=info
FEDERATION_NODE_NAME=railway-production
FEDERATION_NETWORK_KEY=logline-universe-alpha

Port: 8080
```

### **Step 4: Configure Service Dependencies**

In Railway, set up **Service Dependencies**:
1. **logline-timeline** depends on **logline-postgres**
2. **logline-gateway** depends on all other services
3. **logline-engine** depends on **logline-rules** and **logline-timeline**

### **Step 5: Verify Deployment**

After all services deploy, check health endpoints:
```
https://logline-gateway-[random].railway.app/health
https://logline-id-[random].railway.app/health
https://logline-timeline-[random].railway.app/health
https://logline-rules-[random].railway.app/health
https://logline-engine-[random].railway.app/health
https://logline-federation-[random].railway.app/health
```

### **Step 6: Create LogLine IDs**

Once the gateway is live, use the onboarding API to create identities:

#### **Create Dan's Identity**
```bash
curl -X POST https://logline-gateway-[your-url].railway.app/v1/onboarding/identity \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Daniel Amarilho",
    "handle": "danvoulez",
    "email": "dan@danvoulez.com",
    "ghost": false
  }'
```

#### **Create Dan's Foundation Tenant**
```bash
curl -X POST https://logline-gateway-[your-url].railway.app/v1/onboarding/tenant \
  -H "Content-Type: application/json" \
  -d '{
    "name": "danvoulez-foundation",
    "display_name": "Dan Voulez Foundation"
  }'
```

#### **Assign Identity to Tenant**
```bash
curl -X POST https://logline-gateway-[your-url].railway.app/v1/onboarding/assign \
  -H "Content-Type: application/json" \
  -d '{
    "identity_handle": "danvoulez",
    "tenant_name": "danvoulez-foundation"
  }'
```

### **Step 7: Create First Spans**

Create the historic first spans:

#### **Bootstrap Span**
```bash
curl -X POST https://logline-gateway-[your-url].railway.app/v1/onboarding/run \
  -H "Content-Type: application/json" \
  -d '{
    "command": "LogLine Universe deployed to Railway on October 3, 2025 - The first computational memory system is now live!"
  }'
```

---

## 🎯 **EXPECTED RESULTS**

After successful deployment:

### **URLs You'll Get:**
- **Gateway**: `https://logline-gateway-[random].railway.app`
- **Identity**: `https://logline-id-[random].railway.app`
- **Timeline**: `https://logline-timeline-[random].railway.app`
- **Rules**: `https://logline-rules-[random].railway.app`
- **Engine**: `https://logline-engine-[random].railway.app`
- **Federation**: `https://logline-federation-[random].railway.app`

### **Database Connections:**
- **PostgreSQL**: Auto-connected to timeline service
- **Redis**: Available for caching and pub/sub

### **LogLine IDs Created:**
- ✅ `logline-id://person/danvoulez`
- ✅ `logline-id://tenant/danvoulez-foundation`
- ✅ All service agents automatically registered

---

## 🚨 **TROUBLESHOOTING**

### **Build Failures:**
- Check that `SERVICE` environment variable is set correctly
- Verify Dockerfile builds locally: `docker build --build-arg SERVICE=logline-gateway .`

### **Database Connection Issues:**
- Ensure `TIMELINE_DATABASE_URL` uses the Railway PostgreSQL variable
- Check that timeline service depends on postgres service

### **Service Communication:**
- Use Railway internal URLs: `http://service-name.railway.internal:8080`
- Verify all services are deployed and healthy

### **Environment Variables:**
- Double-check all required variables are set
- Ensure JWT secret is exactly: `QQc4wg9wUweVsyWwHUOT5RvXqdz1Q/I8ARJL06ZgfRI=`

---

## 🎉 **SUCCESS CRITERIA**

✅ **All 6 services deployed and healthy**  
✅ **PostgreSQL and Redis connected**  
✅ **Gateway responds to health checks**  
✅ **First LogLine ID created for Dan**  
✅ **First tenant created**  
✅ **First span recorded in timeline**  

**When complete, you'll have the world's first production LogLine Universe!** 🌟

---

**Ready to start? Go to https://railway.app/dashboard and begin with Step 1!** 🚀
