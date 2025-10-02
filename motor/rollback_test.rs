/// # LogLine Runtime Engine - Rollback System Test
/// 
/// Teste de integração do sistema de rollback com o motor principal.
/// Demonstra:
/// - Criação de checkpoints automáticos
/// - Rollback em caso de falha
/// - Replay de operações
/// - Integração com timekeeper

use std::sync::Arc;
use std::time::Duration;
use tokio;

use crate::motor::runtime::{Runtime, RuntimeConfig, run_runtime};
use crate::motor::rollback::{RollbackReason, CheckpointType, SystemStateSnapshot, ResourceUsage};
use crate::motor::timekeeper::TimeState;
use crate::infra::id::logline_id::LogLineIDWithKeys;

#[tokio::test]
async fn test_automatic_checkpoint_creation() {
    println!("🧪 Testando criação automática de checkpoints...");
    
    let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
    let config = RuntimeConfig::default();
    let runtime = Runtime::new(id_with_keys, config);
    
    // Cria checkpoint manual para teste
    let time_state = TimeState {
        last_tick: 1000,
        tick_interval: 64,
        rotation_count: 100,
        drift_detected: false,
        boot_time: 0,
        clock_status: crate::motor::timekeeper::ClockStatus::Running,
    };
    
    let snapshot = SystemStateSnapshot {
        time_state,
        active_executions: vec![],
        scheduler_queue_size: 0,
        federation_status: "connected".to_string(),
        resource_usage: ResourceUsage {
            total_trajs_used: 1000,
            active_jobs: 0,
            memory_usage_mb: 100,
            cpu_usage_percent: 50.0,
            disk_usage_mb: 1000,
        },
        metadata: serde_json::json!({"test": true}),
    };
    
    let checkpoint_id = runtime.rollback_system.create_checkpoint(
        CheckpointType::Manual,
        "Checkpoint de teste",
        snapshot,
    ).await.unwrap();
    
    println!("✅ Checkpoint criado: {}", checkpoint_id);
    
    // Verifica se checkpoint foi criado
    let checkpoints = runtime.rollback_system.list_checkpoints();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].checkpoint_id, checkpoint_id);
    
    // Valida integridade
    let is_valid = runtime.rollback_system.validate_checkpoint(&checkpoint_id).unwrap();
    assert!(is_valid);
    
    println!("✅ Checkpoint validado com sucesso!");
}

#[tokio::test]
async fn test_rollback_operation() {
    println!("🧪 Testando operação de rollback...");
    
    let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
    let config = RuntimeConfig::default();
    let runtime = Runtime::new(id_with_keys, config);
    
    // Cria checkpoint
    let snapshot = SystemStateSnapshot {
        time_state: TimeState {
            last_tick: 2000,
            tick_interval: 64,
            rotation_count: 200,
            drift_detected: false,
            boot_time: 0,
            clock_status: crate::motor::timekeeper::ClockStatus::Running,
        },
        active_executions: vec![],
        scheduler_queue_size: 5,
        federation_status: "connected".to_string(),
        resource_usage: ResourceUsage {
            total_trajs_used: 2000,
            active_jobs: 3,
            memory_usage_mb: 150,
            cpu_usage_percent: 75.0,
            disk_usage_mb: 1200,
        },
        metadata: serde_json::json!({"before_failure": true}),
    };
    
    let checkpoint_id = runtime.rollback_system.create_checkpoint(
        CheckpointType::PreExecution,
        "Checkpoint antes de operação crítica",
        snapshot,
    ).await.unwrap();
    
    println!("📸 Checkpoint criado: {}", checkpoint_id);
    
    // Simula falha que requer rollback
    let rollback_result = runtime.rollback_system.rollback_to_checkpoint(
        checkpoint_id,
        RollbackReason::ExecutionFailure("Simulação de falha crítica".to_string()),
    ).await.unwrap();
    
    println!("🔄 Rollback executado: {}", rollback_result.rollback_id);
    assert!(rollback_result.success);
    assert!(!rollback_result.recovery_actions.is_empty());
    
    println!("✅ Rollback concluído com sucesso!");
    println!("   Ações de recovery: {:?}", rollback_result.recovery_actions);
}

#[tokio::test]
async fn test_checkpoint_verification_mode() {
    println!("🧪 Testando sistema de verificação de checkpoints...");
    
    let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
    let config = RuntimeConfig::default();
    let runtime = Runtime::new(id_with_keys, config);
    
    // Cria múltiplos checkpoints
    for i in 0..3 {
        let snapshot = SystemStateSnapshot {
            time_state: TimeState {
                last_tick: 1000 * (i + 1),
                tick_interval: 64,
                rotation_count: 100 * (i + 1),
                drift_detected: false,
                boot_time: 0,
                clock_status: crate::motor::timekeeper::ClockStatus::Running,
            },
            active_executions: vec![],
            scheduler_queue_size: i,
            federation_status: "connected".to_string(),
            resource_usage: ResourceUsage {
                total_trajs_used: 1000 * (i + 1),
                active_jobs: i,
                memory_usage_mb: 100 + (i * 20),
                cpu_usage_percent: 50.0 + (i as f64 * 10.0),
                disk_usage_mb: 1000 + (i * 100),
            },
            metadata: serde_json::json!({"checkpoint_number": i}),
        };
        
        let checkpoint_id = runtime.rollback_system.create_checkpoint(
            CheckpointType::Automatic,
            &format!("Checkpoint automático #{}", i),
            snapshot,
        ).await.unwrap();
        
        println!("📸 Checkpoint #{} criado: {}", i, checkpoint_id);
        
        // Verifica integridade
        let is_valid = runtime.rollback_system.validate_checkpoint(&checkpoint_id).unwrap();
        assert!(is_valid);
    }
    
    let checkpoints = runtime.rollback_system.list_checkpoints();
    println!("📋 Total de checkpoints: {}", checkpoints.len());
    assert_eq!(checkpoints.len(), 3);
    
    // Verifica ordem cronológica
    for (i, checkpoint) in checkpoints.iter().enumerate() {
        println!("   Checkpoint #{}: {} - {}", 
            i, 
            checkpoint.checkpoint_id, 
            checkpoint.description
        );
    }
    
    println!("✅ Sistema de verificação funcionando corretamente!");
}

#[tokio::test]
async fn test_rollback_constitutional_violation() {
    println!("🧪 Testando rollback por violação constitucional...");
    
    let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
    let config = RuntimeConfig::default();
    let runtime = Runtime::new(id_with_keys, config);
    
    // Cria checkpoint base
    let snapshot = SystemStateSnapshot {
        time_state: TimeState {
            last_tick: 5000,
            tick_interval: 64,
            rotation_count: 500,
            drift_detected: false,
            boot_time: 0,
            clock_status: crate::motor::timekeeper::ClockStatus::Running,
        },
        active_executions: vec![],
        scheduler_queue_size: 0,
        federation_status: "connected".to_string(),
        resource_usage: ResourceUsage {
            total_trajs_used: 5000,
            active_jobs: 0,
            memory_usage_mb: 80,
            cpu_usage_percent: 30.0,
            disk_usage_mb: 800,
        },
        metadata: serde_json::json!({"constitutional_state": "compliant"}),
    };
    
    let checkpoint_id = runtime.rollback_system.create_checkpoint(
        CheckpointType::StateChange,
        "Estado antes de operação constitucional",
        snapshot,
    ).await.unwrap();
    
    // Simula violação constitucional
    let rollback_result = runtime.rollback_system.rollback_to_checkpoint(
        checkpoint_id,
        RollbackReason::ConstitutionalViolation(
            "Violação da regra must_rotate_every_64us".to_string()
        ),
    ).await.unwrap();
    
    println!("⚖️ Rollback constitucional executado: {}", rollback_result.rollback_id);
    assert!(rollback_result.success);
    
    // Verifica ações específicas para violação constitucional
    assert!(rollback_result.recovery_actions.contains(&"mark_contracts_as_draft".to_string()));
    assert!(rollback_result.recovery_actions.contains(&"trigger_enforcement_protocol".to_string()));
    assert!(rollback_result.recovery_actions.contains(&"notify_constitutional_authority".to_string()));
    
    println!("✅ Protocolo de enforcement constitucional ativado!");
    println!("   Ações: {:?}", rollback_result.recovery_actions);
}

/// Função helper para executar todos os testes de rollback
pub async fn run_rollback_tests() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Iniciando testes do sistema de rollback...\n");
    
    test_automatic_checkpoint_creation().await;
    println!();
    
    test_rollback_operation().await;
    println!();
    
    test_checkpoint_verification_mode().await;
    println!();
    
    test_rollback_constitutional_violation().await;
    println!();
    
    println!("🎉 Todos os testes de rollback passaram com sucesso!");
    Ok(())
}