-- Migration 001: Create timeline_spans table for LogLine
-- Estrutura computável com integridade, proveniência e replay

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Tabela principal de spans computáveis
CREATE TABLE timeline_spans (
    id UUID PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    logline_id TEXT NOT NULL,
    author TEXT NOT NULL,
    title TEXT NOT NULL,
    payload JSONB DEFAULT '{}',
    contract_id TEXT,
    workflow_id TEXT,
    flow_id TEXT,
    caused_by UUID REFERENCES timeline_spans(id),
    signature TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'executed' CHECK (status IN ('executed', 'simulated', 'reverted', 'ghost')),
    verification_status TEXT DEFAULT 'verified' CHECK (verification_status IN ('verified', 'pending', 'failed')),
    delta_s FLOAT DEFAULT 0.0,
    replay_count INTEGER DEFAULT 0,
    replay_from UUID REFERENCES timeline_spans(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Índices para performance computável
CREATE INDEX idx_timeline_spans_logline_id ON timeline_spans(logline_id);
CREATE INDEX idx_timeline_spans_timestamp ON timeline_spans(timestamp DESC);
CREATE INDEX idx_timeline_spans_contract_id ON timeline_spans(contract_id);
CREATE INDEX idx_timeline_spans_workflow ON timeline_spans(workflow_id);
CREATE INDEX idx_timeline_spans_caused_by ON timeline_spans(caused_by);
CREATE INDEX idx_timeline_spans_replay_from ON timeline_spans(replay_from);
CREATE INDEX idx_timeline_spans_status ON timeline_spans(status);
CREATE INDEX idx_timeline_spans_verification ON timeline_spans(verification_status);

-- Índice para busca full-text computável
CREATE INDEX idx_timeline_spans_fts ON timeline_spans USING GIN(to_tsvector('portuguese', title || ' ' || coalesce(payload::text, '')));

-- Função para atualizar updated_at automaticamente
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Trigger para updated_at
CREATE TRIGGER update_timeline_spans_updated_at 
    BEFORE UPDATE ON timeline_spans 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

-- Função para garantir append-only (apenas INSERT permitido)
CREATE OR REPLACE FUNCTION enforce_append_only()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        -- Permitir apenas atualização de replay_count e updated_at
        IF OLD.id != NEW.id OR 
           OLD.timestamp != NEW.timestamp OR 
           OLD.logline_id != NEW.logline_id OR
           OLD.signature != NEW.signature THEN
            RAISE EXCEPTION 'Timeline spans são append-only - modificações não permitidas';
        END IF;
    END IF;
    
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Timeline spans são append-only - deletions não permitidas';
    END IF;
    
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

-- Trigger para append-only
CREATE TRIGGER timeline_spans_append_only
    BEFORE UPDATE OR DELETE ON timeline_spans
    FOR EACH ROW
    EXECUTE FUNCTION enforce_append_only();

-- View computável para consultas comuns
CREATE VIEW timeline_view AS
SELECT 
    ts.id,
    ts.timestamp,
    ts.logline_id,
    ts.author,
    ts.title,
    ts.contract_id,
    ts.workflow_id,
    ts.flow_id,
    ts.status,
    ts.verification_status,
    ts.delta_s,
    ts.replay_count,
    CASE WHEN ts.caused_by IS NOT NULL THEN 
        (SELECT title FROM timeline_spans WHERE id = ts.caused_by)
    ELSE NULL END as caused_by_title,
    CASE WHEN ts.replay_from IS NOT NULL THEN 
        (SELECT title FROM timeline_spans WHERE id = ts.replay_from)
    ELSE NULL END as replay_from_title,
    ts.created_at
FROM timeline_spans ts
ORDER BY ts.timestamp DESC;

-- Tabela para metadados da timeline (integridade computável)
CREATE TABLE timeline_metadata (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    total_spans BIGINT NOT NULL,
    signed_spans BIGINT NOT NULL,
    last_span_id UUID REFERENCES timeline_spans(id),
    integrity_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Função para calcular integridade da timeline
CREATE OR REPLACE FUNCTION calculate_timeline_integrity()
RETURNS TEXT AS $$
DECLARE
    integrity_data TEXT;
BEGIN
    SELECT string_agg(
        id::text || timestamp::text || signature, 
        '' ORDER BY timestamp
    ) INTO integrity_data
    FROM timeline_spans;
    
    RETURN encode(sha256(integrity_data::bytea), 'hex');
END;
$$ LANGUAGE plpgsql;