use clap::{Parser, Subcommand};
use std::fs;
use std::env;

// Use the crate modules instead of declaring them locally
use logline::motor;
use logline::infra;
use logline::timeline;
use logline::federation;
use logline::enforcement;

use motor::Engine;
use infra::id::logline_id::{LogLineID, LogLineKeyPair as LogLineIDWithKeys};
use timeline::{Timeline, TimelineQuery, TimelinePostgres, ReplayEngine, HashBundleExporter};

#[derive(Parser)]
#[command(name = "logline")]
#[command(about = "Sistema computável de trajetória viva", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Executa um contrato .lll
    Exec {
        /// Arquivo .lll para executar
        #[arg(short, long)]
        file: String,
        /// Modo simulação (não executa de fato)
        #[arg(short, long)]
        simulate: bool,
    },
    /// Registra um span manual
    Span {
        /// Mensagem do span
        message: String,
    },
    /// Mostra identidade atual
    Whoami,
    /// Cria nova identidade LogLine
    Init {
        /// Nome do nó
        node_name: String,
    },
    /// Faz login com identidade existente
    Login {
        /// LogLine ID para usar
        id: String,
    },
    /// Modo ghost temporário
    Ghost,
    /// Mostra timeline de spans
    Timeline {
        /// LogLine ID para filtrar (opcional)
        #[arg(short, long)]
        logline_id: Option<String>,
        /// Número de spans para mostrar
        #[arg(short, long, default_value = "10")]
        limit: i64,
    },
    /// Exporta timeline para JSON
    Export {
        /// Arquivo de destino (opcional, usa stdout se não especificado)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Busca spans por texto
    Search {
        /// Termo de busca
        term: String,
        /// Número máximo de resultados
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Verifica integridade da timeline
    Verify,
    /// Reexecuta um span por ID
    Replay {
        /// ID do span para reexecutar
        span_id: String,
        /// Usar PostgreSQL ao invés de NDJSON
        #[arg(long)]
        postgres: bool,
    },
    /// Exporta timeline como HashBundle assinado
    HashBundle {
        /// Prefixo dos arquivos de saída
        #[arg(short, long, default_value = "timeline_bundle")]
        output: String,
        /// Usar PostgreSQL ao invés de NDJSON
        #[arg(long)]
        postgres: bool,
    },
    /// Comandos de federação
    Federation {
        #[command(subcommand)]
        subcommand: federation::commands::FederationSub,
    },
    /// Multi-tenant operations
    #[command(subcommand)]
    MultiTenant(infra::cli::multi_tenant::MultiTenantCommand),
}

#[tokio::main]
async fn main() {
    // Carregar variáveis de ambiente do .env
    dotenvy::dotenv().ok();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Init { node_name } => {
            let id_with_keys = LogLineID::generate(&node_name);
            
            match id_with_keys.id.save_to_file(&id_with_keys.signing_key) {
                Ok(_) => {
                    println!("✅ LogLine ID criado: {}", id_with_keys.id);
                    println!("📁 Salvo em: ~/.logline/{}", node_name);
                },
                Err(e) => {
                    eprintln!("❌ Erro ao salvar LogLine ID: {}", e);
                    std::process::exit(1);
                }
            }
        },
        
        Commands::Whoami => {
            // Tenta carregar identidade atual
            if let Some(current_id) = get_current_identity() {
                println!("🆔 Identidade atual: {}", current_id.id);
                println!("📅 Criado em: {}", current_id.id.issued_at.format("%Y-%m-%d %H:%M:%S UTC"));
                println!("🔑 Chave pública: {}", hex::encode(&current_id.id.public_key[..8]));
            } else {
                println!("❌ Nenhuma identidade ativa");
                println!("💡 Use: logline init <nome-do-no> para criar uma nova identidade");
            }
        },
        
        Commands::Login { id } => {
            match LogLineID::from_string(&id) {
                Ok(logline_id) => {
                    println!("✅ Logado como: {}", logline_id);
                    // Aqui seria implementada a persistência da sessão
                },
                Err(e) => {
                    eprintln!("❌ Erro no login: {}", e);
                    std::process::exit(1);
                }
            }
        },
        
        Commands::Ghost => {
            let ghost_id = LogLineID::generate("ghost-temp");
            println!("👻 Modo Ghost ativado");
            println!("🆔 ID temporário: {}", ghost_id.id);
            println!("⚠️  Este ID será perdido ao sair do modo ghost");
        },
        
        Commands::Span { message } => {
            if let Some(current_id) = get_current_identity() {
                let engine = Engine::new().with_logline_id(current_id.id.to_string());
                let mut span = engine.create_span(&message, None);
                
                // Usar timeline NDJSON local
                match Timeline::new() {
                    Ok(timeline) => {
                        match timeline.append_signed_span(&mut span, &current_id) {
                            Ok(span_id) => {
                                println!("✅ Span registrado na timeline:");
                                println!("🆔 ID: {}", span_id);
                                println!("👤 Autor: {}", span.logline_id);
                                println!("📝 Mensagem: {}", span.title);
                                println!("⏰ Timestamp: {}", span.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
                                println!("🔐 Assinatura: {}", span.signature.as_ref().unwrap_or(&"none".to_string())[..16].to_string() + "...");
                            },
                            Err(e) => {
                                eprintln!("❌ Erro ao salvar na timeline: {}", e);
                                println!("📝 Span criado localmente: {}", span.title);
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("❌ Erro ao inicializar timeline: {}", e);
                        println!("📝 Span criado localmente: {}", span.title);
                    }
                }
            } else {
                eprintln!("❌ Nenhuma identidade ativa. Use 'logline init <nome>' primeiro.");
                std::process::exit(1);
            }
        },
        
        Commands::Exec { file, simulate } => {
            if let Some(current_id) = get_current_identity() {
                let engine = Engine::new().with_logline_id(current_id.id.to_string());
                
                match fs::read_to_string(&file) {
                    Ok(content) => {
                        match engine.parse_contract(&content) {
                            Ok(contract) => {
                                let mut result = if simulate {
                                    println!("🧪 Simulando contrato: {}", contract.id);
                                    engine.simulate(&contract)
                                } else {
                                    println!("⚡ Executando contrato: {}", contract.id);
                                    engine.execute(&contract)
                                };
                                
                                if result.success {
                                    // Salvar na timeline NDJSON se não for simulação
                                    if !simulate {
                                        match Timeline::new() {
                                            Ok(timeline) => {
                                                match timeline.append_signed_span(&mut result.span, &current_id) {
                                                    Ok(_) => {
                                                        println!("✅ {} (salvo na timeline)", result.message);
                                                    },
                                                    Err(e) => {
                                                        println!("✅ {} (erro ao salvar na timeline: {})", result.message, e);
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                println!("✅ {} (timeline indisponível: {})", result.message, e);
                                            }
                                        }
                                    } else {
                                        println!("✅ {}", result.message);
                                    }
                                    
                                    println!("📊 Mudanças de estado:");
                                    for change in &result.state_changes {
                                        println!("  • {}", change);
                                    }
                                    println!("🆔 Span ID: {}", result.span.id);
                                } else {
                                    eprintln!("❌ {}", result.message);
                                    std::process::exit(1);
                                }
                            },
                            Err(e) => {
                                eprintln!("❌ Erro ao parsear contrato: {}", e);
                                std::process::exit(1);
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("❌ Erro ao ler arquivo {}: {}", file, e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("❌ Nenhuma identidade ativa. Use 'logline init <nome>' primeiro.");
                std::process::exit(1);
            }
        },
        
        Commands::Timeline { logline_id, limit } => {
            match Timeline::new() {
                Ok(timeline) => {
                    let query = TimelineQuery {
                        logline_id: logline_id.clone(),
                        contract_id: None,
                        workflow_id: None,
                        limit: Some(limit as usize),
                        offset: None,
                    };
                    
                    match timeline.list_spans(&query) {
                        Ok(spans) => {
                            if spans.is_empty() {
                                println!("📭 Nenhum span encontrado");
                                if timeline.timeline_file.exists() {
                                    println!("💡 Arquivo timeline existe em: {}", timeline.timeline_file.display());
                                } else {
                                    println!("💡 Ainda nenhum span foi registrado");
                                }
                            } else {
                                println!("📜 Timeline NDJSON (últimos {} spans):", spans.len());
                                println!("📁 Arquivo: {}", timeline.timeline_file.display());
                                println!("{:=<100}", "");
                                for span in spans {
                                    println!("🆔 {}", span.id);
                                    println!("⏰ {}", span.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
                                    println!("👤 {}", span.logline_id);
                                    println!("📝 {}", span.title);
                                    if let Some(contract_id) = span.contract_id {
                                        println!("📋 Contrato: {}", contract_id);
                                    }
                                    if let Some(workflow_id) = span.workflow_id {
                                        println!("🔄 Workflow: {}", workflow_id);
                                    }
                                    println!("🔐 Status: {} | Verificação: {}", 
                                        span.status, 
                                        span.verification_status.as_ref().unwrap_or(&"unknown".to_string())
                                    );
                                    if let Some(signature) = &span.signature {
                                        println!("✍️  Assinatura: {}...", &signature[..16]);
                                    }
                                    println!("{:-<100}", "");
                                }
                                
                                // Mostrar estatísticas
                                match timeline.get_stats() {
                                    Ok(stats) => {
                                        println!("📊 Estatísticas da Timeline:");
                                        println!("   Total: {} spans | Assinados: {} | Contratos: {}", 
                                            stats.total_spans, stats.signed_spans, stats.contract_spans);
                                        println!("   Executados: {} | Simulados: {} | Ghost: {}", 
                                            stats.executed_spans, stats.simulated_spans, stats.ghost_spans);
                                        println!("   LogLine IDs únicos: {}", stats.unique_logline_ids.len());
                                    },
                                    Err(e) => {
                                        eprintln!("⚠️  Erro ao calcular estatísticas: {}", e);
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("❌ Erro ao consultar timeline: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("❌ Erro ao inicializar timeline: {}", e);
                    std::process::exit(1);
                }
            }
        },
        
        Commands::Export { output } => {
            match Timeline::new() {
                Ok(timeline) => {
                    match timeline.export_timeline() {
                        Ok(json) => {
                            if let Some(file_path) = output {
                                match std::fs::write(&file_path, &json) {
                                    Ok(_) => {
                                        println!("✅ Timeline exportada para: {}", file_path);
                                    },
                                    Err(e) => {
                                        eprintln!("❌ Erro ao escrever arquivo: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            } else {
                                println!("{}", json);
                            }
                        },
                        Err(e) => {
                            eprintln!("❌ Erro ao exportar timeline: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("❌ Erro ao inicializar timeline: {}", e);
                    std::process::exit(1);
                }
            }
        },
        
        Commands::Search { term, limit } => {
            match Timeline::new() {
                Ok(timeline) => {
                    match timeline.search_spans(&term, Some(limit)) {
                        Ok(spans) => {
                            if spans.is_empty() {
                                println!("🔍 Nenhum span encontrado para: '{}'", term);
                            } else {
                                println!("🔍 Encontrados {} spans para: '{}'", spans.len(), term);
                                println!("{:=<80}", "");
                                for span in spans {
                                    println!("🆔 {}", span.id);
                                    println!("⏰ {}", span.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
                                    println!("📝 {}", span.title);
                                    if let Some(contract_id) = span.contract_id {
                                        println!("📋 Contrato: {}", contract_id);
                                    }
                                    println!("{:-<80}", "");
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("❌ Erro na busca: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("❌ Erro ao inicializar timeline: {}", e);
                    std::process::exit(1);
                }
            }
        },
        
        Commands::Verify => {
            match Timeline::new() {
                Ok(timeline) => {
                    match timeline.verify_integrity() {
                        Ok(is_valid) => {
                            if is_valid {
                                println!("✅ Timeline íntegra - todos os spans estão assinados");
                            } else {
                                println!("⚠️  Timeline possui spans sem assinatura");
                            }
                            
                            // Mostrar estatísticas detalhadas
                            match timeline.get_stats() {
                                Ok(stats) => {
                                    println!("\n📊 Estatísticas Completas:");
                                    println!("Total de spans: {}", stats.total_spans);
                                    println!("Spans assinados: {}", stats.signed_spans);
                                    println!("Spans de contratos: {}", stats.contract_spans);
                                    println!("Spans executados: {}", stats.executed_spans);
                                    println!("Spans simulados: {}", stats.simulated_spans);
                                    println!("Spans ghost: {}", stats.ghost_spans);
                                    println!("LogLine IDs únicos: {}", stats.unique_logline_ids.len());
                                    println!("IDs: {:?}", stats.unique_logline_ids);
                                },
                                Err(e) => {
                                    eprintln!("⚠️  Erro ao calcular estatísticas: {}", e);
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("❌ Erro ao verificar integridade: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("❌ Erro ao inicializar timeline: {}", e);
                    std::process::exit(1);
                }
            }
        },
        
        Commands::Replay { span_id, postgres } => {
            if let Some(current_id) = get_current_identity() {
                let replay_engine = ReplayEngine::new(current_id.id.to_string());
                
                // Parse do UUID
                let uuid = match uuid::Uuid::parse_str(&span_id) {
                    Ok(uuid) => uuid,
                    Err(e) => {
                        eprintln!("❌ UUID inválido: {}", e);
                        std::process::exit(1);
                    }
                };
                
                if postgres {
                    // Usar PostgreSQL para replay
                    if let Ok(database_url) = std::env::var("LOGLINE_DATABASE_URL") {
                        match TimelinePostgres::new(&database_url).await {
                            Ok(timeline_pg) => {
                                match replay_engine.replay_span_from_postgres(uuid, &timeline_pg, &current_id).await {
                                    Ok(new_span_id) => {
                                        println!("✅ Replay PostgreSQL concluído!");
                                        println!("🆔 Novo span: {}", new_span_id);
                                    },
                                    Err(e) => {
                                        eprintln!("❌ Erro no replay PostgreSQL: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            },
                            Err(e) => {
                                eprintln!("❌ Erro ao conectar PostgreSQL: {}", e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("❌ LOGLINE_DATABASE_URL não configurada para PostgreSQL");
                        std::process::exit(1);
                    }
                } else {
                    // Usar NDJSON para replay
                    match Timeline::new() {
                        Ok(timeline) => {
                            match replay_engine.replay_span_from_ndjson(uuid, &timeline, &current_id).await {
                                Ok(new_span_id) => {
                                    println!("✅ Replay NDJSON concluído!");
                                    println!("🆔 Novo span: {}", new_span_id);
                                },
                                Err(e) => {
                                    eprintln!("❌ Erro no replay NDJSON: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("❌ Erro ao inicializar timeline NDJSON: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            } else {
                eprintln!("❌ Nenhuma identidade ativa. Use 'logline init <nome>' primeiro.");
                std::process::exit(1);
            }
        },
        
        Commands::HashBundle { output, postgres } => {
            if let Some(current_id) = get_current_identity() {
                if postgres {
                    // Exportar PostgreSQL como HashBundle
                    if let Ok(database_url) = std::env::var("LOGLINE_DATABASE_URL") {
                        match TimelinePostgres::new(&database_url).await {
                            Ok(timeline_pg) => {
                                match HashBundleExporter::export_postgres_hashbundle(&timeline_pg, &output, &current_id).await {
                                    Ok(files) => {
                                        println!("✅ HashBundle PostgreSQL gerado:");
                                        for file in files {
                                            println!("   📄 {}", file);
                                        }
                                    },
                                    Err(e) => {
                                        eprintln!("❌ Erro ao gerar HashBundle PostgreSQL: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            },
                            Err(e) => {
                                eprintln!("❌ Erro ao conectar PostgreSQL: {}", e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("❌ LOGLINE_DATABASE_URL não configurada para PostgreSQL");
                        std::process::exit(1);
                    }
                } else {
                    // Exportar NDJSON como HashBundle
                    match Timeline::new() {
                        Ok(timeline) => {
                            match HashBundleExporter::export_ndjson_hashbundle(&timeline, &output, &current_id) {
                                Ok(files) => {
                                    println!("✅ HashBundle NDJSON gerado:");
                                    for file in files {
                                        println!("   📄 {}", file);
                                    }
                                },
                                Err(e) => {
                                    eprintln!("❌ Erro ao gerar HashBundle NDJSON: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("❌ Erro ao inicializar timeline NDJSON: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            } else {
                eprintln!("❌ Nenhuma identidade ativa. Use 'logline init <nome>' primeiro.");
                std::process::exit(1);
            }
        },
        
        Commands::Federation { subcommand } => {
            use federation::commands::FederationSub;
            
            let result = match subcommand {
                FederationSub::Init => federation::commands::init().await,
                FederationSub::Trust { logline_id, ip, public_key } => {
                    federation::commands::trust(logline_id, ip, public_key).await
                },
                FederationSub::Sync => federation::commands::sync().await,
                FederationSub::Status => federation::commands::status().await,
                FederationSub::Serve => federation::commands::serve().await,
                FederationSub::Untrust { logline_id } => {
                    federation::commands::untrust(logline_id).await
                },
            };
            
            if let Err(e) = result {
                eprintln!("❌ Erro na federação: {}", e);
                std::process::exit(1);
            }
        },
        
        Commands::MultiTenant(cmd) => {
            use infra::cli::multi_tenant::{MultiTenantCliConfig, MultiTenantCommand};
            
            // Load multi-tenant configuration
            let mut config = match MultiTenantCliConfig::load() {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("❌ Erro ao carregar configuração multi-tenant: {}", e);
                    std::process::exit(1);
                }
            };
            
            let result = match cmd {
                MultiTenantCommand::Org(org_cmd) => {
                    infra::cli::multi_tenant_handlers::handle_org_command(org_cmd, &mut config).await
                },
                MultiTenantCommand::Tenant(tenant_cmd) => {
                    infra::cli::multi_tenant_handlers::handle_tenant_command(tenant_cmd, &mut config).await
                },
                MultiTenantCommand::Identity(identity_cmd) => {
                    infra::cli::multi_tenant_handlers::handle_identity_command(identity_cmd, &mut config).await
                },
                MultiTenantCommand::Federation(federation_cmd) => {
                    infra::cli::multi_tenant_handlers::handle_federation_command(federation_cmd, &mut config).await
                },
            };
            
            // Save configuration after any changes
            if let Err(e) = config.save() {
                eprintln!("⚠️  Erro ao salvar configuração: {}", e);
            }
            
            if let Err(e) = result {
                eprintln!("❌ Erro na operação multi-tenant: {}", e);
                std::process::exit(1);
            }
        },
    }
}

fn get_current_identity() -> Option<LogLineIDWithKeys> {
    // Por simplicidade, tenta carregar 'macmini-loja' como padrão
    // Futuramente seria baseado em configuração ou sessão
    LogLineID::load_from_file("macmini-loja").ok()
}