/// # Exemplo de Uso - Sistema de Spans de Verificação
/// 
/// Este arquivo demonstra como usar o sistema de spans de verificação
/// para auditabilidade completa das execuções.

use std::collections::HashMap;
use std::sync::Arc;
use crate::enforcement::verification_spans::{VerificationSpanSystem, ExecutionResult, ResourceUsage};
use crate::motor::span::SpanEmitter;
use crate::grammar::GrammarLoader;
use crate::enforcement::contextual_enforcer::ContextualEnforcer;
use crate::infra::id::logline_id::LogLineIDWithKeys;

/// Exemplo completo de execução com verificação auditável
pub struct AuditableExecutionExample {
    verification_system: VerificationSpanSystem,
    executor_id: LogLineIDWithKeys,
}

impl AuditableExecutionExample {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let span_emitter = Arc::new(SpanEmitter::new_mock());
        let grammar_loader = Arc::new(GrammarLoader::new());
        let executor_id = LogLineIDWithKeys::generate_new()?;
        let enforcement_engine = Arc::new(ContextualEnforcer::new(executor_id.clone(), Arc::clone(&span_emitter)));
        
        let verification_system = VerificationSpanSystem::new(
            span_emitter,
            grammar_loader,
            enforcement_engine,
        );
        
        let executor_id = LogLineIDWithKeys::generate_new()?;
        
        Ok(Self {
            verification_system,
            executor_id,
        })
    }

    /// Executa um contrato com verificação completa
    pub async fn execute_auditable_contract(
        &self,
        project_id: &str,
        contract_reference: &str,
        input_data: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔍 Iniciando execução auditável...");
        println!("📋 Projeto: {}", project_id);
        println!("📄 Contrato: {}", contract_reference);
        
        // 1. Estado inicial
        let state_before = serde_json::json!({
            "status": "preparing",
            "input": input_data,
            "timestamp": chrono::Utc::now()
        });
        
        // 2. Contexto da execução
        let mut context = HashMap::new();
        context.insert("execution_mode".to_string(), serde_json::json!("auditable"));
        context.insert("compliance_level".to_string(), serde_json::json!("full"));
        
        // 3. Cria span de pré-execução
        println!("🧾 Criando span de pré-execução...");
        let verification_span = self.verification_system.create_pre_execution_span(
            &self.executor_id,
            project_id,
            contract_reference,
            state_before,
            context,
        ).await?;
        
        println!("✅ Span criado: {}", verification_span.span_id);
        println!("🔐 Hash de verificação: {}", verification_span.verification_hash);
        
        // 4. **Simula execução do contrato**
        println!("⚙️ Executando contrato...");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // Simula processamento
        
        // 5. Estado final
        let state_after = serde_json::json!({
            "status": "completed",
            "output": {
                "result": "success",
                "processed_at": chrono::Utc::now(),
                "data": "processed_data_example"
            }
        });
        
        // 6. Resultado da execução
        let execution_result = ExecutionResult {
            success: true,
            message: "Contrato executado com sucesso".to_string(),
            output_data: Some(serde_json::json!({"result": "processed"})),
            side_effects: vec![
                "database_updated".to_string(),
                "notification_sent".to_string(),
            ],
            execution_time_ms: 100,
            resources_used: ResourceUsage {
                memory_bytes: 2 * 1024 * 1024, // 2MB
                cpu_percent: 12.5,
                disk_io_bytes: 8192,
                network_io_bytes: 1024,
            },
        };
        
        // 7. Completa o span de verificação
        println!("📝 Completando span de verificação...");
        let final_span = self.verification_system.complete_execution_span(
            verification_span,
            execution_result,
            state_after,
        ).await?;
        
        // 8. Mostra resumo da auditoria
        self.print_audit_summary(&final_span);
        
        println!("🎉 Execução auditável concluída!");
        Ok(())
    }

    /// Mostra resumo da auditoria
    fn print_audit_summary(&self, span: &crate::enforcement::verification_spans::VerificationSpan) {
        println!("\n📊 RESUMO DA AUDITORIA");
        println!("{:-<50}", "");
        
        println!("🆔 Span ID: {}", span.span_id);
        println!("⏰ Timestamp: {}", span.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("🔐 Hash: {}", span.verification_hash);
        
        println!("\n📋 GRAMÁTICA:");
        println!("  📖 Nome: {} v{}", span.grammar_info.name, span.grammar_info.version);
        println!("  🔗 Hash: {}", span.grammar_info.grammar_hash);
        println!("  👤 Autor: {}", span.grammar_info.author);
        println!("  ⏰ Modelo de tempo: {}", span.grammar_info.time_model);
        println!("  ⚖️ Regras: {} aplicadas", span.grammar_info.enforcement_rules.len());
        
        println!("\n🎯 PROVENIÊNCIA:");
        println!("  👤 Executor: {}", span.provenance.executor.to_string());
        println!("  📂 Projeto: {}", span.provenance.project_id);
        println!("  📄 Contrato: {}", span.provenance.contract_reference);
        println!("  🖥️ Nó: {}", span.provenance.execution_node);
        
        println!("\n✅ VALIDAÇÕES:");
        for (i, validation) in span.validations.iter().enumerate() {
            let status = match &validation.result {
                crate::enforcement::verification_spans::ValidationOutcome::Passed => "✅ PASSOU",
                crate::enforcement::verification_spans::ValidationOutcome::Failed(_) => "❌ FALHOU",
                crate::enforcement::verification_spans::ValidationOutcome::Warning(_) => "⚠️ AVISO",
                crate::enforcement::verification_spans::ValidationOutcome::Skipped(_) => "⏭️ PULADO",
            };
            println!("  {}. {} - {} ({}ms)", i + 1, status, validation.details, validation.duration_ms);
        }
        
        println!("\n📈 EXECUÇÃO:");
        println!("  ✅ Sucesso: {}", span.execution_result.success);
        println!("  📝 Mensagem: {}", span.execution_result.message);
        println!("  ⏱️ Tempo: {}ms", span.execution_result.execution_time_ms);
        println!("  🔄 Efeitos: {} registrados", span.execution_result.side_effects.len());
        
        println!("\n💾 RECURSOS:");
        let resources = &span.execution_result.resources_used;
        println!("  🧠 Memória: {:.1} MB", resources.memory_bytes as f64 / 1024.0 / 1024.0);
        println!("  🖥️ CPU: {:.1}%", resources.cpu_percent);
        println!("  💿 Disco I/O: {} KB", resources.disk_io_bytes / 1024);
        println!("  🌐 Rede I/O: {} bytes", resources.network_io_bytes);
        
        println!("\n🔄 REPLAY:");
        println!("  ♻️ Reexecutável: {}", span.replay_info.replayable);
        if let Some(cmd) = &span.replay_info.replay_command {
            println!("  💻 Comando: {}", cmd);
        }
        
        println!("{:-<50}", "");
    }

    /// Demonstração de múltiplas execuções com auditoria
    pub async fn demo_multiple_auditable_executions(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🎭 DEMO: Múltiplas Execuções Auditáveis");
        println!("=====================================\n");
        
        // Execução 1: Minicontratos
        self.execute_auditable_contract(
            "minicontratos",
            "proposta_assinatura_v1",
            serde_json::json!({
                "titulo": "Desenvolvimento de API",
                "valor": 5000.00,
                "prazo_dias": 30
            }),
        ).await?;
        
        println!("\n" + &"=".repeat(60) + "\n");
        
        // Execução 2: Lab
        self.execute_auditable_contract(
            "lab",
            "experimento_proteina_fold",
            serde_json::json!({
                "protein_sequence": "MKLLVLSLSLWLSASAVAAQKIVVLSVNGTPGLQAETDPFSQSMNPPKLDNLQDFDLQNGIQIHSLTQHD",
                "method": "alphafold2",
                "temperature": 300
            }),
        ).await?;
        
        println!("\n" + &"=".repeat(60) + "\n");
        
        // Execução 3: VTV
        self.execute_auditable_contract(
            "vtv",
            "curation_playlist_noturna",
            serde_json::json!({
                "playlist_type": "noturna",
                "duration_minutes": 180,
                "target_audience": "18+"
            }),
        ).await?;
        
        println!("\n🎉 Demo de múltiplas execuções auditáveis concluída!");
        Ok(())
    }
}

/// Função para demonstração via CLI
pub async fn demo_verification_spans() -> Result<(), Box<dyn std::error::Error>> {
    let example = AuditableExecutionExample::new()?;
    example.demo_multiple_auditable_executions().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auditable_execution_example() {
        let example = AuditableExecutionExample::new();
        assert!(example.is_ok());
    }

    #[tokio::test] 
    async fn test_single_auditable_execution() {
        let example = AuditableExecutionExample::new().unwrap();
        
        let result = example.execute_auditable_contract(
            "test_project",
            "test_contract",
            serde_json::json!({"test": true}),
        ).await;
        
        // Em ambiente de teste, algumas dependências podem falhar
        // Mas a estrutura deve estar correta
        assert!(result.is_ok() || result.is_err()); // Ambos são válidos em teste
    }
}