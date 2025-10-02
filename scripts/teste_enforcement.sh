#!/bin/bash
# teste_enforcement.sh - Script para testar o sistema de enforcement do LogLine

echo "=== Teste do Sistema de Enforcement LogLine ==="
echo "Data: $(date)"
echo ""

# Diretório base
BASE_DIR=$(dirname "$0")
EXAMPLES_DIR="$BASE_DIR/examples"

echo "=== 1. Testando Contrato Válido ==="
cargo run -- validate-rule --span "$EXAMPLES_DIR/contrato_valido.json"
echo ""

echo "=== 2. Testando Contrato Acima do Limite (Deve Rejeitar) ==="
cargo run -- validate-rule --span "$EXAMPLES_DIR/contrato_acima_limite.json"
echo ""

echo "=== 3. Testando Contrato em Modo Simulação ==="
cargo run -- validate-rule --span "$EXAMPLES_DIR/contrato_simulado.json"
echo ""

echo "=== 4. Testando Contrato com Restrição de Papel Admin ==="
cargo run -- validate-rule --span "$EXAMPLES_DIR/contrato_restrito.json"
echo ""

echo "=== 5. Testando Transição de Contrato com Validação de Estado ==="
cargo run -- validate-rule --span "$EXAMPLES_DIR/transicao_contrato.json"
echo ""

echo "=== Histórico de Auditoria ==="
cargo run -- show-audit --limit 10

echo ""
echo "=== Exportando Auditoria ==="
cargo run -- show-audit --export ndjson --output "$BASE_DIR/auditoria.ndjson"

echo ""
echo "Testes concluídos!"