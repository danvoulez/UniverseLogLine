# 🚀 Railway Manual Setup Guide - LogLine Universe

Since the API requires workspace context, here's the manual setup process:

## Step 1: Create Project on Railway Web Interface

1. Go to [railway.app](https://railway.app)
2. Login with your account
3. Click **"New Project"**
4. Name it: **`logline-universe`**
5. Description: **"LogLine Universe - Distributed logging and identity system"**

## Step 2: Add Database Services

### PostgreSQL Database
1. In your project, click **"Add Service"**
2. Select **"Database"** → **"PostgreSQL"**
3. Railway will provision a PostgreSQL 15 instance
4. Note the connection details

### Redis Cache
1. Click **"Add Service"** again  
2. Select **"Database"** → **"Redis"**
3. Railway will provision a Redis 7 instance

## Step 3: Deploy the 4 Microservices

For each service, follow these steps:

### 3.1 Deploy logline-id Service
1. Click **"Add Service"** → **"GitHub Repo"**
2. Select: **`danvoulez/UniverseLogLine`**
3. **Service Name**: `logline-id`
4. **Root Directory**: `.` (root)
5. **Dockerfile**: `Dockerfile.id`
6. **Port**: `8079`

### 3.2 Deploy logline-timeline Service  
1. Click **"Add Service"** → **"GitHub Repo"**
2. Select: **`danvoulez/UniverseLogLine`**
3. **Service Name**: `logline-timeline`
4. **Root Directory**: `.` (root)
5. **Dockerfile**: `Dockerfile.timeline`
6. **Port**: `8080`

### 3.3 Deploy logline-rules Service
1. Click **"Add Service"** → **"GitHub Repo"**
2. Select: **`danvoulez/UniverseLogLine`**
3. **Service Name**: `logline-rules`
4. **Root Directory**: `.` (root)
5. **Dockerfile**: `Dockerfile.rules`
6. **Port**: `8081`

### 3.4 Deploy logline-engine Service
1. Click **"Add Service"** → **"GitHub Repo"**
2. Select: **`danvoulez/UniverseLogLine`**
3. **Service Name**: `logline-engine`
4. **Root Directory**: `.` (root)
5. **Dockerfile**: `Dockerfile.engine`
6. **Port**: `8082`

## Step 4: Configure Environment Variables

In your Railway project settings, add these shared variables:

```bash
# Core Configuration
LOGLINE_ENV=production
LOGLINE_NODE_NAME=logline-production

# Service Bindings
LOGLINE_HTTP_BIND=0.0.0.0:8080
LOGLINE_WS_BIND=0.0.0.0:8081

# Database URLs (Railway auto-populates these)
DATABASE_URL=${{Postgres.DATABASE_URL}}
REDIS_URL=${{Redis.REDIS_URL}}
```

## Step 5: Service-Specific Environment Variables

### For logline-timeline:
```bash
TIMELINE_DATABASE_URL=${{Postgres.DATABASE_URL}}
TIMELINE_HTTP_BIND=0.0.0.0:8080
```

### For logline-rules:
```bash
RULES_DATABASE_URL=${{Postgres.DATABASE_URL}}
RULES_HTTP_BIND=0.0.0.0:8081
```

### For logline-engine:
```bash
ENGINE_DATABASE_URL=${{Postgres.DATABASE_URL}}
ENGINE_REDIS_URL=${{Redis.REDIS_URL}}
ENGINE_HTTP_BIND=0.0.0.0:8082
```

### For logline-id:
```bash
ID_HTTP_BIND=0.0.0.0:8079
```

## Step 6: Run Database Migrations

Once PostgreSQL is running:

1. Go to PostgreSQL service → **"Connect"** → **"Query"**
2. Copy and paste the content from each migration file:
   - `migrations/001_create_timeline_spans.sql`
   - `migrations/002_implement_multi_tenant_infrastructure.sql`
   - `migrations/003_multi_tenant_timeline_integration.sql`
3. Execute them in order

## Step 7: Verify Deployment

### Check Service Status
All services should show **"Active"** status in Railway dashboard.

### Test Endpoints
Once deployed, you'll get URLs like:
- **ID Service**: `https://logline-id-production.up.railway.app`
- **Timeline**: `https://logline-timeline-production.up.railway.app`
- **Rules**: `https://logline-rules-production.up.railway.app`
- **Engine**: `https://logline-engine-production.up.railway.app`

### Health Checks
Test each service:
```bash
curl https://your-service-url/health
```

## 🎯 Expected Result

After setup, you'll have:
- ✅ **6 services running**: PostgreSQL, Redis, ID, Timeline, Rules, Engine
- ✅ **Complete database schema** with multi-tenancy
- ✅ **All 4 microservices** deployed and communicating
- ✅ **Production-ready infrastructure** on Railway

## 📊 Service Architecture

```
┌─────────────────┐    ┌─────────────────┐
│   PostgreSQL    │    │      Redis      │
│   (Database)    │    │    (Cache)      │
└─────────┬───────┘    └─────────┬───────┘
          │                      │
          └──────────┬───────────┘
                     │
    ┌────────────────┼────────────────┐
    │                │                │
┌───▼───┐    ┌───▼───┐    ┌───▼───┐    ┌───▼───┐
│  ID   │    │Timeline│    │ Rules │    │Engine │
│ :8079 │    │ :8080  │    │ :8081 │    │ :8082 │
└───────┘    └───────┘    └───────┘    └───────┘
```

## 🆘 Troubleshooting

### Build Failures
- Check Dockerfile syntax
- Verify all dependencies in Cargo.toml
- Check Railway build logs

### Connection Issues
- Verify DATABASE_URL is set correctly
- Check service-to-service networking
- Verify environment variables

### Migration Issues
- Run migrations in correct order
- Check PostgreSQL version (needs 12+)
- Verify database permissions

---

**Total Deployment Time**: ~10-15 minutes
**Services**: 6 (2 databases + 4 microservices)
**Ports**: 8079 (ID), 8080 (Timeline), 8081 (Rules), 8082 (Engine)
