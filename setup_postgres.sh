#!/bin/bash

# Script de setup do PostgreSQL para LogLine
echo "🛢️  Configurando PostgreSQL para LogLine..."

# Verificar se PostgreSQL está instalado
if ! command -v psql &> /dev/null; then
    echo "❌ PostgreSQL não encontrado"
    echo "💡 No macOS, instale com: brew install postgresql"
    exit 1
fi

# Verificar se PostgreSQL está rodando
if ! pg_isready -q; then
    echo "🚀 Iniciando PostgreSQL..."
    if command -v brew &> /dev/null; then
        brew services start postgresql
    else
        sudo systemctl start postgresql
    fi
    sleep 2
fi

# Criar banco de dados se não existir
DB_NAME="logline"
DB_USER="logline_user"
DB_PASS="logline_pass"

# Criar usuário e banco
psql postgres -c "CREATE USER $DB_USER WITH PASSWORD '$DB_PASS';" 2>/dev/null || echo "Usuário já existe"
psql postgres -c "CREATE DATABASE $DB_NAME OWNER $DB_USER;" 2>/dev/null || echo "Banco já existe"
psql postgres -c "GRANT ALL PRIVILEGES ON DATABASE $DB_NAME TO $DB_USER;" 2>/dev/null

# Executar migrations
DATABASE_URL="postgresql://$DB_USER:$DB_PASS@localhost:5432/$DB_NAME"
echo "🔧 Executando migrations..."

if [ -f "migrations/001_create_timeline_spans.sql" ]; then
    psql "$DATABASE_URL" -f migrations/001_create_timeline_spans.sql
    echo "✅ Migration executada com sucesso"
else
    echo "❌ Arquivo de migration não encontrado"
    echo "💡 Certifique-se de estar no diretório do projeto LogLine"
    exit 1
fi

echo ""
echo "✅ PostgreSQL configurado com sucesso!"
echo "🔗 URL de conexão: $DATABASE_URL"
echo ""
echo "💡 Para usar, configure:"
echo "   export DATABASE_URL='$DATABASE_URL'"
echo ""
echo "🧪 Teste com:"
echo "   ./target/release/logline timeline"