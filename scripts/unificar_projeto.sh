#!/bin/bash
# Script para unificação dos projetos LogLine ID
# Este script auxilia na migração dos arquivos das pastas A e B para a pasta unificada

# Definir pastas de origem e destino
PASTA_A="/Users/voulezvous/lets make LogLine ID"
PASTA_B="/Users/voulezvous/B_lets make LogLine ID"  # Ajuste conforme necessário
PASTA_UNIFICADA="/Users/voulezvous/logline_id_unificado"

# Criar diretório de log
mkdir -p "$PASTA_UNIFICADA/logs"
LOG_FILE="$PASTA_UNIFICADA/logs/migracao_$(date +%Y%m%d_%H%M%S).log"

# Função para logar mensagens
log() {
    echo "[$(date +"%Y-%m-%d %H:%M:%S")] $1" | tee -a "$LOG_FILE"
}

# Iniciar log
log "Iniciando processo de migração LogLine ID"
log "Pasta A: $PASTA_A"
log "Pasta B: $PASTA_B"
log "Pasta Unificada: $PASTA_UNIFICADA"

# 1. Migrar arquivos de código da pasta B (base primária)
log "Migrando arquivos de código da Pasta B..."

# Copiar todos os arquivos .rs da pasta B
find "$PASTA_B" -name "*.rs" | while read file; do
    rel_path="${file#$PASTA_B/}"
    target_dir=$(dirname "$PASTA_UNIFICADA/$rel_path")
    
    # Criar diretório de destino se não existir
    mkdir -p "$target_dir"
    
    # Copiar o arquivo
    cp "$file" "$PASTA_UNIFICADA/$rel_path"
    log "Copiado: $rel_path"
done

# 2. Migrar arquivos .lll da pasta B
log "Migrando arquivos .lll da Pasta B..."
find "$PASTA_B" -name "*.lll" | while read file; do
    rel_path="${file#$PASTA_B/}"
    target_dir=$(dirname "$PASTA_UNIFICADA/$rel_path")
    
    # Criar diretório de destino se não existir
    mkdir -p "$target_dir"
    
    # Copiar o arquivo
    cp "$file" "$PASTA_UNIFICADA/$rel_path"
    log "Copiado: $rel_path"
done

# 3. Migrar schemas JSON da pasta B
log "Migrando schemas JSON da Pasta B..."
find "$PASTA_B" -name "*.schema.json" | while read file; do
    # Colocar todos os schemas na pasta schema/
    filename=$(basename "$file")
    cp "$file" "$PASTA_UNIFICADA/schema/$filename"
    log "Copiado schema: $filename"
done

# 4. Migrar arquivos UI da pasta B
log "Migrando arquivos de UI da Pasta B..."
find "$PASTA_B" -path "*/ui/*" -name "*.tsx" -o -name "*.ts" -o -name "*.css" | while read file; do
    rel_path="${file#$PASTA_B/}"
    target_dir=$(dirname "$PASTA_UNIFICADA/$rel_path")
    
    # Criar diretório de destino se não existir
    mkdir -p "$target_dir"
    
    # Copiar o arquivo
    cp "$file" "$PASTA_UNIFICADA/$rel_path"
    log "Copiado: $rel_path"
done

# 5. Migrar arquivos de código da pasta A que não existem em B
log "Migrando arquivos exclusivos da Pasta A..."

# Copiar arquivos .rs da pasta A se não existirem em B
find "$PASTA_A" -name "*.rs" | while read file; do
    rel_path="${file#$PASTA_A/}"
    b_file="$PASTA_B/$rel_path"
    target_file="$PASTA_UNIFICADA/$rel_path"
    
    # Se o arquivo não existe na pasta B, copiá-lo
    if [ ! -f "$b_file" ]; then
        target_dir=$(dirname "$target_file")
        mkdir -p "$target_dir"
        cp "$file" "$target_file"
        log "Copiado arquivo exclusivo da Pasta A: $rel_path"
    # Se existe na pasta B com conteúdo diferente, copiá-lo com sufixo _legacy
    elif ! cmp -s "$file" "$b_file"; then
        target_dir=$(dirname "$target_file")
        mkdir -p "$target_dir"
        base_name=$(basename "$rel_path" .rs)
        cp "$file" "$target_dir/${base_name}_legacy.rs"
        log "Copiado com renomeação: $rel_path -> ${base_name}_legacy.rs"
    fi
done

# 6. Migrar arquivos de teste da pasta A
log "Migrando arquivos de teste da Pasta A..."
find "$PASTA_A" -path "*/tests/*" -name "*.rs" | while read file; do
    filename=$(basename "$file")
    cp "$file" "$PASTA_UNIFICADA/test/$filename"
    log "Copiado teste: $filename"
done

# 7. Migrar exemplos da pasta A
log "Migrando exemplos da Pasta A..."
find "$PASTA_A" -path "*/examples/*" -name "*.json" | while read file; do
    filename=$(basename "$file")
    cp "$file" "$PASTA_UNIFICADA/examples/$filename"
    log "Copiado exemplo: $filename"
done

# 8. Migrar arquivos .lll da pasta A que não existem em B
log "Migrando arquivos .lll exclusivos da Pasta A..."
find "$PASTA_A" -name "*.lll" | while read file; do
    rel_path="${file#$PASTA_A/}"
    b_file="$PASTA_B/$rel_path"
    target_file="$PASTA_UNIFICADA/$rel_path"
    
    # Se o arquivo não existe na pasta B, copiá-lo
    if [ ! -f "$b_file" ]; then
        target_dir=$(dirname "$target_file")
        mkdir -p "$target_dir"
        cp "$file" "$target_file"
        log "Copiado arquivo .lll exclusivo da Pasta A: $rel_path"
    fi
done

# 9. Migrar documentação da pasta A
log "Migrando documentação da Pasta A..."
find "$PASTA_A" -name "*.md" -not -path "*/target/*" | while read file; do
    filename=$(basename "$file")
    if [ ! -f "$PASTA_UNIFICADO/docs/$filename" ]; then
        cp "$file" "$PASTA_UNIFICADA/docs/$filename"
        log "Copiada documentação: $filename"
    fi
done

# 10. Migrar Cargo.toml da pasta B
log "Migrando Cargo.toml da Pasta B..."
if [ -f "$PASTA_B/Cargo.toml" ]; then
    cp "$PASTA_B/Cargo.toml" "$PASTA_UNIFICADA/Cargo.toml"
    log "Copiado Cargo.toml da Pasta B"
fi

# 11. Migrar migrações SQL
log "Migrando migrações SQL..."
find "$PASTA_B" -path "*/migrations/*.sql" | while read file; do
    filename=$(basename "$file")
    cp "$file" "$PASTA_UNIFICADA/migrations/$filename"
    log "Copiada migração SQL: $filename"
done

# 12. Migrar scripts específicos
log "Migrando scripts..."
find "$PASTA_A" -name "*.sh" -not -path "*/target/*" | while read file; do
    filename=$(basename "$file")
    cp "$file" "$PASTA_UNIFICADA/scripts/$filename"
    log "Copiado script: $filename"
done

log "Migração concluída! Verifique o log em $LOG_FILE para detalhes."
log "Próximos passos recomendados:"
log "1. Revisar estrutura unificada"
log "2. Resolver conflitos manuais pendentes"
log "3. Testar compilação e execução"
log "4. Atualizar documentação conforme necessário"