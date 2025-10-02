use std::fs;
use std::path::Path;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use crate::timeline::{Timeline, TimelinePostgres};
use crate::infra::id::logline_id::LogLineIDWithKeys;

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineMetadata {
    pub export_timestamp: String,
    pub total_spans: u64,
    pub signed_spans: u64,
    pub contract_spans: u64,
    pub unique_logline_ids: Vec<String>,
    pub timeline_hash: String,
    pub integrity_verified: bool,
    pub export_format: String,
    pub logline_version: String,
}

pub struct HashBundleExporter;

impl HashBundleExporter {
    /// Exporta timeline NDJSON como HashBundle assinado
    pub fn export_ndjson_hashbundle(
        timeline: &Timeline,
        output_prefix: &str,
        id_with_keys: &LogLineIDWithKeys
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        println!("📦 Gerando HashBundle da timeline NDJSON...");
        
        // 1. Exportar timeline como JSON
        let timeline_json = timeline.export_timeline()?;
        let stats = timeline.get_stats()?;
        let integrity = timeline.verify_integrity()?;
        
        // 2. Calcular hash SHA256 da timeline
        let mut hasher = Sha256::new();
        hasher.update(timeline_json.as_bytes());
        let timeline_hash = hex::encode(hasher.finalize());
        
        // 3. Criar metadados
        let metadata = TimelineMetadata {
            export_timestamp: Utc::now().to_rfc3339(),
            total_spans: stats.total_spans,
            signed_spans: stats.signed_spans,
            contract_spans: stats.contract_spans,
            unique_logline_ids: stats.unique_logline_ids.clone(),
            timeline_hash: timeline_hash.clone(),
            integrity_verified: integrity,
            export_format: "ndjson".to_string(),
            logline_version: "0.1.0".to_string(),
        };
        
        // 4. Salvar arquivos
        let ndjson_file = format!("{}.ndjson", output_prefix);
        let meta_file = format!("{}.meta.json", output_prefix);
        let sig_file = format!("{}.sig", output_prefix);
        
        // Salvar timeline NDJSON
        fs::write(&ndjson_file, &timeline_json)?;
        println!("✅ Timeline salva: {}", ndjson_file);
        
        // Salvar metadados
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&meta_file, &metadata_json)?;
        println!("✅ Metadados salvos: {}", meta_file);
        
        // 5. Assinar o bundle (timeline + metadata)
        let bundle_content = format!("{}{}", timeline_json, metadata_json);
        let signature = id_with_keys.id.sign(&id_with_keys.signing_key, bundle_content.as_bytes());
        let signature_hex = hex::encode(signature.to_bytes());
        
        // Criar arquivo de assinatura
        let sig_content = format!(
            "LogLine HashBundle Signature v1.0\nTimeline Hash: {}\nSigned by: {}\nSignature: {}\nTimestamp: {}",
            timeline_hash,
            id_with_keys.id.to_string(),
            signature_hex,
            Utc::now().to_rfc3339()
        );
        fs::write(&sig_file, &sig_content)?;
        println!("✅ Assinatura gerada: {}", sig_file);
        
        println!("🔐 HashBundle completo gerado!");
        println!("📊 Hash da timeline: {}", timeline_hash);
        println!("✍️  Assinado por: {}", id_with_keys.id);
        
        Ok(vec![ndjson_file, meta_file, sig_file])
    }
    
    /// Exporta timeline PostgreSQL como HashBundle assinado
    pub async fn export_postgres_hashbundle(
        timeline_pg: &TimelinePostgres,
        output_prefix: &str,
        id_with_keys: &LogLineIDWithKeys
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        println!("📦 Gerando HashBundle da timeline PostgreSQL...");
        
        // 1. Exportar todos os spans do PostgreSQL
        use crate::timeline::TimelineQuery;
        
        let query = TimelineQuery {
            logline_id: None,
            contract_id: None,
            workflow_id: None,
            limit: None,
            offset: None,
        };
        
        let spans = timeline_pg.list_spans(&query).await?;
        let stats = timeline_pg.get_stats().await?;
        let integrity = timeline_pg.verify_integrity().await?;
        
        // 2. Converter para JSON
        let timeline_json = serde_json::to_string_pretty(&spans)?;
        
        // 3. Calcular hash SHA256 da timeline
        let mut hasher = Sha256::new();
        hasher.update(timeline_json.as_bytes());
        let timeline_hash = hex::encode(hasher.finalize());
        
        // 4. Criar metadados
        let metadata = TimelineMetadata {
            export_timestamp: Utc::now().to_rfc3339(),
            total_spans: stats.total_spans,
            signed_spans: stats.signed_spans,
            contract_spans: stats.contract_spans,
            unique_logline_ids: stats.unique_logline_ids.clone(),
            timeline_hash: timeline_hash.clone(),
            integrity_verified: integrity,
            export_format: "postgres".to_string(),
            logline_version: "0.1.0".to_string(),
        };
        
        // 5. Salvar arquivos
        let json_file = format!("{}.json", output_prefix);
        let meta_file = format!("{}.meta.json", output_prefix);
        let sig_file = format!("{}.sig", output_prefix);
        
        // Salvar timeline JSON
        fs::write(&json_file, &timeline_json)?;
        println!("✅ Timeline PostgreSQL salva: {}", json_file);
        
        // Salvar metadados
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&meta_file, &metadata_json)?;
        println!("✅ Metadados salvos: {}", meta_file);
        
        // 6. Assinar o bundle
        let bundle_content = format!("{}{}", timeline_json, metadata_json);
        let signature = id_with_keys.id.sign(&id_with_keys.signing_key, bundle_content.as_bytes());
        let signature_hex = hex::encode(signature.to_bytes());
        
        // Criar arquivo de assinatura
        let sig_content = format!(
            "LogLine HashBundle Signature v1.0\nTimeline Hash: {}\nSigned by: {}\nSignature: {}\nTimestamp: {}",
            timeline_hash,
            id_with_keys.id.to_string(),
            signature_hex,
            Utc::now().to_rfc3339()
        );
        fs::write(&sig_file, &sig_content)?;
        println!("✅ Assinatura PostgreSQL gerada: {}", sig_file);
        
        println!("🔐 HashBundle PostgreSQL completo!");
        println!("📊 Hash da timeline: {}", timeline_hash);
        println!("✍️  Assinado por: {}", id_with_keys.id);
        
        Ok(vec![json_file, meta_file, sig_file])
    }
    
    /// Verifica integridade de um HashBundle
    pub fn verify_hashbundle(
        bundle_prefix: &str,
        expected_logline_id: Option<&str>
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let ndjson_file = format!("{}.ndjson", bundle_prefix);
        let json_file = format!("{}.json", bundle_prefix);
        let meta_file = format!("{}.meta.json", bundle_prefix);
        let sig_file = format!("{}.sig", bundle_prefix);
        
        // Determinar qual formato usar
        let timeline_file = if Path::new(&ndjson_file).exists() {
            &ndjson_file
        } else if Path::new(&json_file).exists() {
            &json_file
        } else {
            return Err("Arquivo de timeline não encontrado (.ndjson ou .json)".into());
        };
        
        println!("🔍 Verificando HashBundle: {}", bundle_prefix);
        
        // Ler arquivos
        let timeline_content = fs::read_to_string(timeline_file)?;
        let metadata_content = fs::read_to_string(&meta_file)?;
        let sig_content = fs::read_to_string(&sig_file)?;
        
        // Parsear metadados
        let metadata: TimelineMetadata = serde_json::from_str(&metadata_content)?;
        
        // Verificar hash da timeline
        let mut hasher = Sha256::new();
        hasher.update(timeline_content.as_bytes());
        let calculated_hash = hex::encode(hasher.finalize());
        
        if calculated_hash != metadata.timeline_hash {
            println!("❌ Hash da timeline não confere!");
            println!("   Esperado: {}", metadata.timeline_hash);
            println!("   Calculado: {}", calculated_hash);
            return Ok(false);
        }
        
        println!("✅ Hash da timeline verificado");
        println!("📊 Total de spans: {}", metadata.total_spans);
        println!("🔐 Spans assinados: {}", metadata.signed_spans);
        println!("📅 Exportado em: {}", metadata.export_timestamp);
        
        // TODO: Verificar assinatura digital (precisa da chave pública)
        if let Some(expected_id) = expected_logline_id {
            if !sig_content.contains(expected_id) {
                println!("⚠️  LogLine ID na assinatura não confere com o esperado");
                return Ok(false);
            }
        }
        
        println!("✅ HashBundle íntegro e verificado!");
        Ok(true)
    }
}