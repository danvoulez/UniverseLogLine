use uuid::Uuid;
use std::fs;
use crate::motor::{Engine, types::Span};
use crate::timeline::{Timeline, TimelinePostgres};
use crate::infra::id::logline_id::LogLineIDWithKeys;

pub struct ReplayEngine {
    engine: Engine,
}

impl ReplayEngine {
    pub fn new(logline_id: String) -> Self {
        let engine = Engine::new().with_logline_id(logline_id);
        ReplayEngine { engine }
    }
    
    /// Replay de um span específico por ID
    pub async fn replay_span_from_ndjson(
        &self, 
        span_id: Uuid, 
        timeline: &Timeline,
        id_with_keys: &LogLineIDWithKeys
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        // Buscar span original
        let original_span = timeline.get_span(span_id)?
            .ok_or("Span não encontrado na timeline NDJSON")?;
        
        println!("🔁 Iniciando replay do span: {}", span_id);
        println!("📝 Título original: {}", original_span.title);
        
        // Se o span tem contract_id, reexecutar o contrato
        if let Some(contract_id) = &original_span.contract_id {
            println!("📋 Reexecutando contrato: {}", contract_id);
            
            // Tentar carregar arquivo .lll
            let contract_file = format!("{}.lll", contract_id);
            if let Ok(contract_content) = fs::read_to_string(&contract_file) {
                match self.engine.parse_contract(&contract_content) {
                    Ok(contract) => {
                        let mut result = self.engine.execute(&contract);
                        
                        // Marcar como replay
                        result.span.title = format!("🔁 Replay: {}", result.span.title);
                        result.span.caused_by = Some(span_id);
                        
                        // Salvar replay na timeline
                        let replay_span_id = timeline.append_signed_span(&mut result.span, id_with_keys)?;
                        
                        println!("✅ Replay executado com sucesso!");
                        println!("🆔 Novo span ID: {}", replay_span_id);
                        println!("🔗 Caused by: {}", span_id);
                        
                        return Ok(replay_span_id);
                    },
                    Err(e) => {
                        return Err(format!("Erro ao parsear contrato: {}", e).into());
                    }
                }
            } else {
                return Err(format!("Arquivo de contrato não encontrado: {}", contract_file).into());
            }
        } else {
            // Se não tem contrato, apenas criar um span de replay
            let mut replay_span = Span::new(
                original_span.logline_id.clone(),
                format!("🔁 Replay: {}", original_span.title)
            );
            
            replay_span.payload = original_span.payload.clone();
            replay_span.workflow_id = original_span.workflow_id.clone();
            replay_span.flow_id = original_span.flow_id.clone();
            replay_span.caused_by = Some(span_id);
            
            let replay_span_id = timeline.append_signed_span(&mut replay_span, id_with_keys)?;
            
            println!("✅ Replay simples executado!");
            println!("🆔 Novo span ID: {}", replay_span_id);
            
            return Ok(replay_span_id);
        }
    }
    
    /// Replay de um span usando PostgreSQL
    pub async fn replay_span_from_postgres(
        &self,
        span_id: Uuid,
        timeline_pg: &TimelinePostgres,
        id_with_keys: &LogLineIDWithKeys
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        // Buscar span original no PostgreSQL
        let original_span = timeline_pg.get_span_by_id(span_id).await?
            .ok_or("Span não encontrado na timeline PostgreSQL")?;
        
        println!("🔁 Iniciando replay PostgreSQL do span: {}", span_id);
        println!("📝 Título original: {}", original_span.title);
        
        // Se o span tem contract_id, reexecutar o contrato
        if let Some(contract_id) = &original_span.contract_id {
            println!("📋 Reexecutando contrato: {}", contract_id);
            
            // Tentar carregar arquivo .lll
            let contract_file = format!("{}.lll", contract_id);
            if let Ok(contract_content) = fs::read_to_string(&contract_file) {
                match self.engine.parse_contract(&contract_content) {
                    Ok(contract) => {
                        let mut result = self.engine.execute(&contract);
                        
                        // Marcar como replay
                        result.span.title = format!("🔁 Replay: {}", result.span.title);
                        result.span.caused_by = Some(span_id);
                        
                        // Salvar replay no PostgreSQL usando register_replay
                        let replay_span_id = timeline_pg.register_replay(span_id, &result.span).await?;
                        
                        println!("✅ Replay PostgreSQL executado com sucesso!");
                        println!("🆔 Novo span ID: {}", replay_span_id);
                        println!("🔗 Caused by: {}", span_id);
                        println!("📊 Replay count incrementado no span original");
                        
                        return Ok(replay_span_id);
                    },
                    Err(e) => {
                        return Err(format!("Erro ao parsear contrato: {}", e).into());
                    }
                }
            } else {
                return Err(format!("Arquivo de contrato não encontrado: {}", contract_file).into());
            }
        } else {
            // Se não tem contrato, apenas criar um span de replay
            let mut replay_span = Span::new(
                original_span.logline_id.clone(),
                format!("🔁 Replay: {}", original_span.title)
            );
            
            replay_span.payload = original_span.payload.clone();
            replay_span.workflow_id = original_span.workflow_id.clone();
            replay_span.flow_id = original_span.flow_id.clone();
            replay_span.caused_by = Some(span_id);
            
            let replay_span_id = timeline_pg.register_replay(span_id, &replay_span).await?;
            
            println!("✅ Replay PostgreSQL simples executado!");
            println!("🆔 Novo span ID: {}", replay_span_id);
            
            return Ok(replay_span_id);
        }
    }
}