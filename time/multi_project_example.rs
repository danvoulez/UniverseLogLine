/// # Exemplo de Uso - Clocks Específicos por Projeto
/// 
/// Este arquivo demonstra como cada projeto (minicontratos, lab, vtv)
/// pode ter seu próprio clock computável rodando simultaneamente,
/// cada um com tempo e regras diferentes.

use std::sync::Arc;
use tokio::time::Duration;
use crate::time::{ProjectClockManager, AdaptiveClock};
use crate::grammar::{GrammarLoader, LocalGrammar};
use crate::motor::span::SpanEmitter;
use crate::infra::id::logline_id::LogLineIDWithKeys;

/// Exemplo completo de sistema multi-clock
pub struct MultiProjectClockSystem {
    clock_manager: ProjectClockManager,
    grammar_loader: GrammarLoader,
}

impl MultiProjectClockSystem {
    pub fn new() -> Self {
        Self {
            clock_manager: ProjectClockManager::new(),
            grammar_loader: GrammarLoader::new(),
        }
    }

    /// Inicializa todos os clocks dos projetos
    pub async fn initialize_all_projects(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 Inicializando sistema multi-clock LogLine...");

        // 1. Minicontratos - Dias úteis brasileiros
        self.setup_minicontratos_clock().await?;

        // 2. LogLine Lab - Ciclos experimentais de 7 dias
        self.setup_lab_clock().await?;

        // 3. VoulezVous TV - Slots de 30 minutos
        self.setup_vtv_clock().await?;

        println!("✅ Todos os clocks inicializados!");
        self.print_status().await;

        Ok(())
    }

    /// Setup do clock dos minicontratos
    async fn setup_minicontratos_clock(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📋 Configurando clock dos Minicontratos...");

        // Carrega gramática dos minicontratos
        let grammar_content = include_str!("../grammar/grammar_minicontratos.lll");
        let grammar: LocalGrammar = serde_json::from_str(grammar_content)?;

        // Registra o clock
        self.clock_manager.register_project_clock(
            "minicontratos".to_string(),
            &grammar
        ).await?;

        println!("✅ Clock Minicontratos: Dias úteis brasileiros (9h-18h)");
        Ok(())
    }

    /// Setup do clock do Lab
    async fn setup_lab_clock(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔬 Configurando clock do LogLine Lab...");

        // Carrega gramática do lab
        let grammar_content = include_str!("../grammar/grammar_lab.lll");
        let grammar: LocalGrammar = serde_json::from_str(grammar_content)?;

        // Registra o clock
        self.clock_manager.register_project_clock(
            "lab".to_string(),
            &grammar
        ).await?;

        println!("✅ Clock Lab: Ciclos experimentais de 7 dias");
        Ok(())
    }

    /// Setup do clock da VTV
    async fn setup_vtv_clock(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📺 Configurando clock da VoulezVous TV...");

        // Carrega gramática da VTV
        let grammar_content = include_str!("../grammar/grammar_vtv.lll");
        let grammar: LocalGrammar = serde_json::from_str(grammar_content)?;

        // Registra o clock
        self.clock_manager.register_project_clock(
            "vtv".to_string(),
            &grammar
        ).await?;

        println!("✅ Clock VTV: Slots de 30 minutos (24h)");
        Ok(())
    }

    /// Mostra status de todos os clocks
    pub async fn print_status(&self) {
        println!("\n📊 STATUS DOS CLOCKS:");
        println!("{:-<50}", "");

        let summary = self.clock_manager.get_summary();
        println!("📈 Total de projetos: {}", summary.total_projects);
        println!("▶️  Projetos ativos: {}", summary.active_projects);
        println!("🔢 Total de ticks: {}", summary.total_ticks_processed);
        println!("⏰ Uptime desde: {}", summary.uptime_since.format("%Y-%m-%d %H:%M:%S"));

        println!("\n🕐 DETALHES POR PROJETO:");
        
        for project_id in self.clock_manager.list_active_projects() {
            if let Some(status) = self.clock_manager.get_project_status(&project_id) {
                println!("  {} 📌 {}: {} ticks | último: {} | tempo local: {}",
                    match project_id.as_str() {
                        "minicontratos" => "📋",
                        "lab" => "🔬",
                        "vtv" => "📺",
                        _ => "⚙️"
                    },
                    project_id,
                    status.tick_count,
                    status.last_tick.format("%H:%M:%S"),
                    status.current_local_time
                );
            }
        }
        println!("{:-<50}", "");
    }

    /// Roda monitoramento em tempo real
    pub async fn run_monitoring_loop(&self, duration_minutes: u64) -> Result<(), Box<dyn std::error::Error>> {
        println!("👁️  Iniciando monitoramento por {} minutos...", duration_minutes);
        
        let end_time = std::time::Instant::now() + Duration::from_secs(duration_minutes * 60);
        
        while std::time::Instant::now() < end_time {
            tokio::time::sleep(Duration::from_secs(30)).await; // Update a cada 30s
            
            print!("\x1B[2J\x1B[1;1H"); // Limpa tela
            println!("🕐 MONITORAMENTO EM TEMPO REAL - LogLine Multi-Clock");
            println!("⏱️  Restam: {:.1} minutos\n", 
                (end_time - std::time::Instant::now()).as_secs_f64() / 60.0
            );
            
            self.print_status().await;
        }

        println!("\n✅ Monitoramento concluído!");
        Ok(())
    }

    /// Para todos os clocks
    pub async fn shutdown_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🛑 Parando todos os clocks...");
        self.clock_manager.stop_all_clocks().await;
        println!("✅ Todos os clocks parados!");
        Ok(())
    }
}

/// Listener exemplo que reage a eventos temporais específicos de cada projeto
pub struct ProjectTemporalListener {
    project_id: String,
}

impl ProjectTemporalListener {
    pub fn new(project_id: String) -> Self {
        Self { project_id }
    }
}

#[async_trait::async_trait]
impl crate::time::adaptive_clock::TemporalListener for ProjectTemporalListener {
    async fn on_temporal_tick(&self, time_state: &crate::time::TimeState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.project_id.as_str() {
            "minicontratos" => {
                // Minicontratos: checa vencimentos a cada tick de dia útil
                if time_state.current_unit_value >= 1.0 {
                    println!("📋 Minicontratos: Checando vencimentos (dia útil #{:.0})", 
                        time_state.current_unit_value);
                }
            },
            "lab" => {
                // Lab: avança ciclo experimental
                if time_state.current_unit_value % 7.0 == 0.0 && time_state.current_unit_value > 0.0 {
                    println!("🔬 Lab: Ciclo experimental #{:.0} concluído!", 
                        time_state.current_unit_value / 7.0);
                }
            },
            "vtv" => {
                // VTV: troca de programação a cada slot
                if time_state.current_unit_value % 1.0 == 0.0 {
                    println!("📺 VTV: Mudança de slot #{:.0} ({}min)", 
                        time_state.current_unit_value, 
                        time_state.current_unit_value * 30.0
                    );
                }
            },
            _ => {}
        }
        Ok(())
    }

    async fn on_temporal_event(&self, event: &crate::time::TemporalEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🎯 {}: Evento temporal disparado: {}", self.project_id, event.event_id);
        Ok(())
    }

    async fn on_clock_status_change(&self, old_status: &crate::time::TemporalClockStatus, new_status: &crate::time::TemporalClockStatus) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🔄 {}: Status mudou de {:?} para {:?}", self.project_id, old_status, new_status);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_project_clock_initialization() {
        let mut system = MultiProjectClockSystem::new();
        
        // Este teste seria executado com gramáticas mock
        // let result = system.initialize_all_projects().await;
        // assert!(result.is_ok());
        
        // Por enquanto, apenas verifica que o sistema foi criado
        assert_eq!(system.clock_manager.list_active_projects().len(), 0);
    }

    #[tokio::test]
    async fn test_project_temporal_listener() {
        let listener = ProjectTemporalListener::new("test_project".to_string());
        assert_eq!(listener.project_id, "test_project");
    }
}

/// Função de demonstração para uso via CLI
pub async fn demo_multi_clock_system() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 DEMO: Sistema Multi-Clock LogLine");
    println!("=====================================\n");

    let mut system = MultiProjectClockSystem::new();
    
    // Inicializa todos os projetos
    system.initialize_all_projects().await?;
    
    // Monitora por 2 minutos
    system.run_monitoring_loop(2).await?;
    
    // Para tudo
    system.shutdown_all().await?;

    println!("\n🎉 Demo concluída!");
    Ok(())
}