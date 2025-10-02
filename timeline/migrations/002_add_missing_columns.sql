-- Migration 002: Add missing columns for FASE 2
-- Adiciona verification_status, delta_s, replay_count, replay_from

ALTER TABLE timeline_spans 
ADD COLUMN verification_status TEXT DEFAULT 'verified' CHECK (verification_status IN ('verified', 'pending', 'failed'));

ALTER TABLE timeline_spans 
ADD COLUMN delta_s FLOAT DEFAULT 0.0;

ALTER TABLE timeline_spans 
ADD COLUMN replay_count INTEGER DEFAULT 0;

ALTER TABLE timeline_spans 
ADD COLUMN replay_from UUID REFERENCES timeline_spans(id);

-- Adicionar índice para replay_from
CREATE INDEX idx_timeline_spans_replay_from ON timeline_spans(replay_from);

-- Adicionar índice para verification_status  
CREATE INDEX idx_timeline_spans_verification ON timeline_spans(verification_status);

-- Atualizar updated_at nos registros existentes
ALTER TABLE timeline_spans 
ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();