use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::motor::types::{Span, SpanStatus};
use crate::infra::id::logline_id::LogLineIDWithKeys;

#[derive(Debug, Clone)]
pub struct Timeline {
    data_dir: PathBuf,
    pub timeline_file: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineQuery {
    pub logline_id: Option<String>,
    pub contract_id: Option<String>,
    pub workflow_id: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimelineEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub logline_id: String,
    pub author: String,
    pub title: String,
    pub payload: serde_json::Value,
    pub contract_id: Option<String>,
    pub workflow_id: Option<String>,
    pub flow_id: Option<String>,
    pub caused_by: Option<Uuid>,
    pub signature: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    // Campos computáveis adicionais
    pub delta_s: Option<f64>, // Esforço acumulado
    pub replay_count: Option<u32>, // Quantas vezes foi reexecutado
    pub verification_status: Option<String>, // verified, pending, failed
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TimelineStats {
    pub total_spans: u64,
    pub signed_spans: u64,
    pub contract_spans: u64,
    pub executed_spans: u64,
    pub simulated_spans: u64,
    pub ghost_spans: u64,
    pub other_spans: u64,
    pub unique_logline_ids: Vec<String>,
}

impl Timeline {
    /// Cria nova instância da Timeline NDJSON
    pub fn new() -> Result<Self, std::io::Error> {
        let home_dir = dirs::home_dir().ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Diretório home não encontrado"
        ))?;
        
        let data_dir = home_dir.join(".logline").join("data");
        std::fs::create_dir_all(&data_dir)?;
        
        let timeline_file = data_dir.join("timeline.ndjson");
        
        Ok(Timeline {
            data_dir,
            timeline_file,
        })
    }
    
    /// Converte Span para TimelineEntry
    fn span_to_entry(&self, span: &Span) -> TimelineEntry {
        TimelineEntry {
            id: span.id,
            timestamp: span.timestamp,
            logline_id: span.logline_id.clone(),
            author: span.logline_id.clone(),
            title: span.title.clone(),
            payload: span.payload.clone(),
            contract_id: span.contract_id.clone(),
            workflow_id: span.workflow_id.clone(),
            flow_id: span.flow_id.clone(),
            caused_by: span.caused_by,
            signature: span.signature.clone(),
            status: match span.status {
                SpanStatus::Executed => "executed".to_string(),
                SpanStatus::Simulated => "simulated".to_string(),
                SpanStatus::Reverted => "reverted".to_string(),
                SpanStatus::Ghost => "ghost".to_string(),
            },
            created_at: Utc::now(),
            delta_s: None,
            replay_count: Some(0),
            verification_status: if span.signature.is_some() { 
                Some("verified".to_string()) 
            } else { 
                Some("pending".to_string()) 
            },
        }
    }
    
    /// Anexa um span à timeline (append-only NDJSON)
    pub fn append_span(&self, span: &Span) -> Result<Uuid, std::io::Error> {
        let entry = self.span_to_entry(span);
        let json_line = serde_json::to_string(&entry)?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.timeline_file)?;
        
        writeln!(file, "{}", json_line)?;
        file.sync_all()?;
        
        println!("✅ Span anexado à timeline NDJSON: {}", entry.id);
        println!("📁 Arquivo: {}", self.timeline_file.display());
        
        Ok(entry.id)
    }
    
    /// Assina um span antes de anexar à timeline
    pub fn append_signed_span(&self, span: &mut Span, id_with_keys: &LogLineIDWithKeys) -> Result<Uuid, std::io::Error> {
        // Serializa o span para assinatura (sem a assinatura)
        let mut span_for_signing = span.clone();
        span_for_signing.signature = None;
        let span_data = serde_json::to_string(&span_for_signing).unwrap();
        let signature = id_with_keys.id.sign(&id_with_keys.signing_key, span_data.as_bytes());
        
        // Adiciona assinatura ao span
        span.signature = Some(hex::encode(signature.to_bytes()));
        
        // Anexa à timeline
        self.append_span(span)
    }
    
    /// Lista spans com filtros opcionais
    pub fn list_spans(&self, query: &TimelineQuery) -> Result<Vec<TimelineEntry>, std::io::Error> {
        if !self.timeline_file.exists() {
            return Ok(vec![]);
        }
        
        let file = File::open(&self.timeline_file)?;
        let reader = BufReader::new(file);
        
        let mut entries: Vec<TimelineEntry> = Vec::new();
        
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<TimelineEntry>(&line) {
                Ok(entry) => {
                    // Aplicar filtros
                    let mut include = true;
                    
                    if let Some(ref logline_id) = query.logline_id {
                        if entry.logline_id != *logline_id {
                            include = false;
                        }
                    }
                    
                    if let Some(ref contract_id) = query.contract_id {
                        if entry.contract_id.as_ref() != Some(contract_id) {
                            include = false;
                        }
                    }
                    
                    if let Some(ref workflow_id) = query.workflow_id {
                        if entry.workflow_id.as_ref() != Some(workflow_id) {
                            include = false;
                        }
                    }
                    
                    if include {
                        entries.push(entry);
                    }
                },
                Err(e) => {
                    eprintln!("⚠️  Erro ao parsear linha da timeline: {}", e);
                    continue;
                }
            }
        }
        
        // Ordenar por timestamp (mais recente primeiro)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        // Aplicar limit e offset
        let start = query.offset.unwrap_or(0);
        let end = start + query.limit.unwrap_or(50);
        
        Ok(entries.into_iter().skip(start).take(end - start).collect())
    }
    
    /// Obtém um span específico por ID
    pub fn get_span(&self, span_id: Uuid) -> Result<Option<TimelineEntry>, std::io::Error> {
        if !self.timeline_file.exists() {
            return Ok(None);
        }
        
        let file = File::open(&self.timeline_file)?;
        let reader = BufReader::new(file);
        
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<TimelineEntry>(&line) {
                Ok(entry) => {
                    if entry.id == span_id {
                        return Ok(Some(entry));
                    }
                },
                Err(_) => continue,
            }
        }
        
        Ok(None)
    }
    
    /// Verifica integridade da timeline
    pub fn verify_integrity(&self) -> Result<bool, std::io::Error> {
        if !self.timeline_file.exists() {
            return Ok(true);
        }
        
        let file = File::open(&self.timeline_file)?;
        let reader = BufReader::new(file);
        
        let mut unsigned_count = 0;
        let mut total_count = 0;
        
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<TimelineEntry>(&line) {
                Ok(entry) => {
                    total_count += 1;
                    if entry.signature.is_none() || entry.signature.as_ref().unwrap().is_empty() {
                        unsigned_count += 1;
                    }
                },
                Err(_) => continue,
            }
        }
        
        println!("📊 Integridade da timeline: {}/{} spans assinados", total_count - unsigned_count, total_count);
        Ok(unsigned_count == 0)
    }
    
    /// Busca spans por texto no título
    pub fn search_spans(&self, search_term: &str, limit: Option<usize>) -> Result<Vec<TimelineEntry>, std::io::Error> {
        if !self.timeline_file.exists() {
            return Ok(vec![]);
        }
        
        let file = File::open(&self.timeline_file)?;
        let reader = BufReader::new(file);
        
        let mut entries: Vec<TimelineEntry> = Vec::new();
        let search_lower = search_term.to_lowercase();
        
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<TimelineEntry>(&line) {
                Ok(entry) => {
                    if entry.title.to_lowercase().contains(&search_lower) || 
                       entry.payload.to_string().to_lowercase().contains(&search_lower) {
                        entries.push(entry);
                    }
                },
                Err(_) => continue,
            }
        }
        
        // Ordenar por timestamp (mais recente primeiro)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        // Aplicar limit
        let limit = limit.unwrap_or(20);
        Ok(entries.into_iter().take(limit).collect())
    }
    
    /// Exporta timeline para JSON
    pub fn export_timeline(&self) -> Result<String, std::io::Error> {
        let query = TimelineQuery {
            logline_id: None,
            contract_id: None,
            workflow_id: None,
            limit: None,
            offset: None,
        };
        
        let entries = self.list_spans(&query)?;
        Ok(serde_json::to_string_pretty(&entries)?)
    }
    
    /// Calcula estatísticas da timeline
    pub fn get_stats(&self) -> Result<TimelineStats, std::io::Error> {
        if !self.timeline_file.exists() {
            return Ok(TimelineStats::default());
        }
        
        let file = File::open(&self.timeline_file)?;
        let reader = BufReader::new(file);
        
        let mut stats = TimelineStats::default();
        
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<TimelineEntry>(&line) {
                Ok(entry) => {
                    stats.total_spans += 1;
                    
                    if entry.signature.is_some() {
                        stats.signed_spans += 1;
                    }
                    
                    if entry.contract_id.is_some() {
                        stats.contract_spans += 1;
                    }
                    
                    match entry.status.as_str() {
                        "executed" => stats.executed_spans += 1,
                        "simulated" => stats.simulated_spans += 1,
                        "ghost" => stats.ghost_spans += 1,
                        _ => stats.other_spans += 1,
                    }
                    
                    // Contar LogLine IDs únicos
                    if !stats.unique_logline_ids.contains(&entry.logline_id) {
                        stats.unique_logline_ids.push(entry.logline_id);
                    }
                },
                Err(_) => continue,
            }
        }
        
        Ok(stats)
    }
    
    /// Anexa uma entry direto à timeline (usado pela federação)
    pub fn append_entry(&self, entry: &TimelineEntry) -> Result<(), std::io::Error> {
        let json_line = serde_json::to_string(entry)?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.timeline_file)?;
        
        writeln!(file, "{}", json_line)?;
        file.sync_all()?;
        
        Ok(())
    }

    /// Importa um span de fonte externa (federação)
    pub fn import_span(&mut self, span: serde_json::Value) -> Result<(), std::io::Error> {
        // Validar se span tem campos obrigatórios
        let span_id = span.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidData, 
                "Span deve ter campo 'id'"
            ))?;
            
        // Verificar se já existe
        if let Ok(uuid) = uuid::Uuid::parse_str(span_id) {
            if self.get_span(uuid)?.is_some() {
                return Ok(()); // Já existe, não importar
            }
        }
        
        // Converter para TimelineEntry
        let entry: TimelineEntry = serde_json::from_value(span)
            .map_err(|e| std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Erro ao converter span: {}", e)
            ))?;
            
        // Adicionar à timeline
        self.append_entry(&entry)
    }
}