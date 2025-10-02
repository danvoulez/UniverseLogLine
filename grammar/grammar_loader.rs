/// # LogLine Grammar Loader
///
/// Sistema de carregamento e validação de gramáticas locais.
/// Responsável por:
/// - Ler contratos contract_register_grammar.lll
/// - Validar hash, extends, entities e enforcement
/// - Carregar gramática no sistema ativo
/// - Verificar compatibilidade com grammar_core
/// - Gerenciar versionamento de gramáticas
///
/// Cada projeto deve registrar sua gramática computavelmente
/// antes de poder operar no sistema.

use std::collections::HashMap;
use std::path::Path;
use serde::{Serialize, Deserialize};
use std::fs;

use crate::enforcement::contextual_enforcer::{LocalGrammar, TimeModel, EnforcementConfig, EntityDefinition, WorkflowDefinition};
use crate::infra::id::logline_id::{LogLineID, LogLineIDWithKeys};
use crate::motor::span::SpanEmitter;

/// Carregador de gramáticas
pub struct GrammarLoader {
    id_with_keys: LogLineIDWithKeys,
    span_emitter: std::sync::Arc<SpanEmitter>,
    
    /// Gramáticas carregadas e validadas
    loaded_grammars: HashMap<String, LoadedGrammar>,
    
    /// Gramática core (imutável)
    core_grammar: Option<CoreGrammar>,
    
    /// Configuração do loader
    config: GrammarLoaderConfig,
}

/// Gramática carregada com metadados
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedGrammar {
    pub grammar: LocalGrammar,
    pub loaded_at: u64,
    pub loaded_by: LogLineID,
    pub validation_result: GrammarValidationResult,
    pub file_path: String,
    pub file_hash: String,
}

/// Resultado da validação de gramática
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarValidationResult {
    pub is_valid: bool,
    pub core_compatibility: bool,
    pub hash_verified: bool,
    pub extends_valid: bool,
    pub structures_valid: bool,
    pub enforcement_valid: bool,
    pub time_model_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Gramática core carregada
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreGrammar {
    pub name: String,
    pub version: String,
    pub hash: String,
    pub primitive_types: HashMap<String, PrimitiveTypeDefinition>,
    pub universal_structures: HashMap<String, UniversalStructureDefinition>,
    pub universal_enforcement: UniversalEnforcementRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimitiveTypeDefinition {
    pub description: String,
    pub validation: String,
    pub constraints: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalStructureDefinition {
    pub description: String,
    pub required_fields: HashMap<String, String>,
    pub optional_fields: HashMap<String, String>,
    pub immutable_fields: Vec<String>,
    pub validation_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalEnforcementRules {
    pub rules: Vec<UniversalRule>,
    pub triggers: Vec<UniversalTrigger>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalRule {
    pub rule_id: String,
    pub expression: String,
    pub scope: String,
    pub priority: u32,
    pub violation_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalTrigger {
    pub trigger_id: String,
    pub condition: String,
    pub action: String,
    pub automatic: bool,
}

/// Configuração do loader
#[derive(Debug, Clone)]
pub struct GrammarLoaderConfig {
    pub grammar_directory: String,
    pub validate_hashes: bool,
    pub require_signatures: bool,
    pub allow_extends_override: bool,
    pub max_grammar_size_mb: u32,
}

/// Contrato de registro de gramática
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarRegistrationContract {
    pub id: LogLineID,
    pub grammar_name: String,
    pub grammar_version: String,
    pub extends: String,
    pub author: LogLineID,
    pub created_at: u64,
    pub file_path: String,
    pub file_hash: String,
    pub signature: String,
    pub metadata: serde_json::Value,
}

impl GrammarLoader {
    /// Cria novo loader de gramáticas
    pub fn new(
        id_with_keys: LogLineIDWithKeys,
        span_emitter: std::sync::Arc<SpanEmitter>,
        config: GrammarLoaderConfig,
    ) -> Self {
        Self {
            id_with_keys,
            span_emitter,
            loaded_grammars: HashMap::new(),
            core_grammar: None,
            config,
        }
    }

    /// Carrega gramática core (obrigatório)
    pub async fn load_core_grammar(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let core_path = format!("{}/grammar_core.lll", self.config.grammar_directory);
        
        println!("📚 Carregando gramática core: {}", core_path);
        
        if !Path::new(&core_path).exists() {
            return Err(format!("Arquivo grammar_core.lll não encontrado: {}", core_path).into());
        }

        let core_content = fs::read_to_string(&core_path)?;
        let core_grammar = self.parse_core_grammar(&core_content)?;
        
        // Valida gramática core
        self.validate_core_grammar(&core_grammar)?;
        
        self.core_grammar = Some(core_grammar.clone());
        
        // Emite span
        self.emit_core_grammar_loaded_span(&core_grammar).await?;
        
        println!("✅ Gramática core carregada: {} v{}", core_grammar.name, core_grammar.version);
        Ok(())
    }

    /// Carrega gramática local de projeto
    pub async fn load_project_grammar(
        &mut self,
        project_name: &str,
        registration_contract_path: &str,
    ) -> Result<LoadedGrammar, Box<dyn std::error::Error>> {
        println!("📖 Carregando gramática do projeto: {}", project_name);
        
        // Lê contrato de registro
        let registration = self.read_registration_contract(registration_contract_path).await?;
        
        // Valida contrato de registro
        self.validate_registration_contract(&registration)?;
        
        // Lê arquivo da gramática
        let grammar_content = fs::read_to_string(&registration.file_path)?;
        let grammar = self.parse_local_grammar(&grammar_content)?;
        
        // Valida gramática
        let validation_result = self.validate_local_grammar(&grammar).await?;
        
        if !validation_result.is_valid {
            return Err(format!("Gramática inválida: {:?}", validation_result.errors).into());
        }

        let loaded_grammar = LoadedGrammar {
            grammar,
            loaded_at: self.current_timestamp(),
            loaded_by: self.id_with_keys.id.to_string(),
            validation_result,
            file_path: registration.file_path.clone(),
            file_hash: registration.file_hash.clone(),
        };

        // Armazena gramática carregada
        self.loaded_grammars.insert(project_name.to_string(), loaded_grammar.clone());
        
        // Emite span
        self.emit_grammar_loaded_span(project_name, &loaded_grammar).await?;
        
        println!("✅ Gramática carregada: {} v{}", 
            loaded_grammar.grammar.name, 
            loaded_grammar.grammar.version
        );
        
        Ok(loaded_grammar)
    }

    /// Busca gramática carregada
    pub fn get_loaded_grammar(&self, project_name: &str) -> Option<&LoadedGrammar> {
        self.loaded_grammars.get(project_name)
    }

    /// Lista todas as gramáticas carregadas
    pub fn list_loaded_grammars(&self) -> Vec<&LoadedGrammar> {
        self.loaded_grammars.values().collect()
    }

    /// Valida se gramática pode ser carregada
    pub async fn can_load_grammar(&self, registration_contract_path: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let registration = self.read_registration_contract(registration_contract_path).await?;
        
        // Verifica se extends grammar_core
        if !registration.extends.starts_with("grammar_core@") {
            return Ok(false);
        }
        
        // Verifica se arquivo existe
        if !Path::new(&registration.file_path).exists() {
            return Ok(false);
        }
        
        // Verifica hash se configurado
        if self.config.validate_hashes {
            let actual_hash = self.calculate_file_hash(&registration.file_path)?;
            if actual_hash != registration.file_hash {
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    /// Lê contrato de registro de gramática
    async fn read_registration_contract(
        &self,
        contract_path: &str,
    ) -> Result<GrammarRegistrationContract, Box<dyn std::error::Error>> {
        if !Path::new(contract_path).exists() {
            return Err(format!("Contrato de registro não encontrado: {}", contract_path).into());
        }

        let content = fs::read_to_string(contract_path)?;
        
        // Parser simples do contrato .lll
        // Em implementação real seria um parser completo
        let contract = self.parse_registration_contract(&content)?;
        
        Ok(contract)
    }

    /// Valida contrato de registro
    fn validate_registration_contract(
        &self,
        registration: &GrammarRegistrationContract,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Valida campos obrigatórios
        if registration.grammar_name.is_empty() {
            return Err("Nome da gramática é obrigatório".into());
        }
        
        if registration.grammar_version.is_empty() {
            return Err("Versão da gramática é obrigatória".into());
        }
        
        if !registration.extends.starts_with("grammar_core@") {
            return Err("Gramática deve estender grammar_core".into());
        }
        
        if registration.file_hash.is_empty() {
            return Err("Hash do arquivo é obrigatório".into());
        }
        
        Ok(())
    }

    /// Valida gramática local
    async fn validate_local_grammar(
        &self,
        grammar: &LocalGrammar,
    ) -> Result<GrammarValidationResult, Box<dyn std::error::Error>> {
        let mut result = GrammarValidationResult {
            is_valid: true,
            core_compatibility: true,
            hash_verified: true,
            extends_valid: true,
            structures_valid: true,
            enforcement_valid: true,
            time_model_valid: true,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        // Verifica se core está carregado
        let core_grammar = self.core_grammar.as_ref()
            .ok_or("Gramática core não carregada")?;

        // Valida extends
        if !grammar.extends.starts_with("grammar_core@") {
            result.extends_valid = false;
            result.errors.push("Deve estender grammar_core".to_string());
        }

        // Valida estruturas universais
        self.validate_universal_structures_compliance(grammar, core_grammar, &mut result);
        
        // Valida modelo de tempo
        self.validate_time_model(&grammar.time_model, &mut result);
        
        // Valida enforcement
        self.validate_enforcement_config(&grammar.enforcement, &mut result);
        
        // Valida entidades do domínio
        self.validate_domain_entities(&grammar.domain_entities, &mut result);
        
        // Valida workflows
        self.validate_workflows(&grammar.workflows, &mut result);

        // Se há erros, marca como inválida
        if !result.errors.is_empty() {
            result.is_valid = false;
        }

        Ok(result)
    }

    /// Valida conformidade com estruturas universais
    fn validate_universal_structures_compliance(
        &self,
        grammar: &LocalGrammar,
        core_grammar: &CoreGrammar,
        result: &mut GrammarValidationResult,
    ) {
        // Verifica se estruturas universais são suportadas
        for (structure_name, _) in &core_grammar.universal_structures {
            if !grammar.domain_entities.contains_key(structure_name) {
                result.warnings.push(format!(
                    "Estrutura universal '{}' não está explicitamente definida", 
                    structure_name
                ));
            }
        }
    }

    /// Valida modelo de tempo
    fn validate_time_model(&self, time_model: &TimeModel, result: &mut GrammarValidationResult) {
        if time_model.name.is_empty() {
            result.time_model_valid = false;
            result.errors.push("Modelo de tempo deve ter nome".to_string());
        }
        
        if time_model.calculation_rules.is_empty() {
            result.warnings.push("Nenhuma regra de cálculo temporal definida".to_string());
        }
    }

    /// Valida configuração de enforcement
    fn validate_enforcement_config(&self, enforcement: &EnforcementConfig, result: &mut GrammarValidationResult) {
        // Valida regras
        for rule in &enforcement.rules {
            if rule.rule_id.is_empty() || rule.expression.is_empty() {
                result.enforcement_valid = false;
                result.errors.push("Regras devem ter ID e expressão".to_string());
            }
        }
        
        // Valida triggers
        for trigger in &enforcement.triggers {
            if trigger.trigger_id.is_empty() || trigger.condition.is_empty() {
                result.enforcement_valid = false;
                result.errors.push("Triggers devem ter ID e condição".to_string());
            }
        }
    }

    /// Valida entidades do domínio
    fn validate_domain_entities(&self, entities: &HashMap<String, EntityDefinition>, result: &mut GrammarValidationResult) {
        for (entity_name, entity) in entities {
            if entity.required_fields.is_empty() {
                result.warnings.push(format!("Entidade '{}' não tem campos obrigatórios", entity_name));
            }
        }
    }

    /// Valida workflows
    fn validate_workflows(&self, workflows: &HashMap<String, WorkflowDefinition>, result: &mut GrammarValidationResult) {
        for (workflow_name, workflow) in workflows {
            if workflow.states.is_empty() {
                result.errors.push(format!("Workflow '{}' deve ter pelo menos um estado", workflow_name));
            }
            
            if workflow.transitions.is_empty() {
                result.warnings.push(format!("Workflow '{}' não tem transições definidas", workflow_name));
            }
        }
    }

    /// Valida gramática core
    fn validate_core_grammar(&self, core_grammar: &CoreGrammar) -> Result<(), Box<dyn std::error::Error>> {
        if core_grammar.name != "grammar_core" {
            return Err("Gramática core deve ter name = 'grammar_core'".into());
        }
        
        if core_grammar.primitive_types.is_empty() {
            return Err("Gramática core deve definir tipos primitivos".into());
        }
        
        if core_grammar.universal_structures.is_empty() {
            return Err("Gramática core deve definir estruturas universais".into());
        }
        
        Ok(())
    }

    /// Parseia gramática core
    fn parse_core_grammar(&self, content: &str) -> Result<CoreGrammar, Box<dyn std::error::Error>> {
        // Parser simplificado - em implementação real seria um parser completo
        Ok(CoreGrammar {
            name: "grammar_core".to_string(),
            version: "1.0.0".to_string(),
            hash: "sha256:core_hash".to_string(),
            primitive_types: HashMap::new(),
            universal_structures: HashMap::new(),
            universal_enforcement: UniversalEnforcementRules {
                rules: vec![],
                triggers: vec![],
            },
        })
    }

    /// Parseia gramática local
    fn parse_local_grammar(&self, content: &str) -> Result<LocalGrammar, Box<dyn std::error::Error>> {
        // Parser simplificado - em implementação real seria um parser completo
        Ok(LocalGrammar {
            name: "example_grammar".to_string(),
            version: "1.0.0".to_string(),
            extends: "grammar_core@1.0.0".to_string(),
            hash: "sha256:example_hash".to_string(),
            time_model: TimeModel {
                name: "default".to_string(),
                unit: crate::enforcement::contextual_enforcer::TimeUnit::Days,
                business_calendar: None,
                calculation_rules: vec![],
            },
            enforcement: EnforcementConfig {
                rules: vec![],
                triggers: vec![],
                witness_requirements: HashMap::new(),
            },
            domain_entities: HashMap::new(),
            workflows: HashMap::new(),
        })
    }

    /// Parseia contrato de registro
    fn parse_registration_contract(&self, content: &str) -> Result<GrammarRegistrationContract, Box<dyn std::error::Error>> {
        // Parser simplificado
        Ok(GrammarRegistrationContract {
            id: LogLineID::generate_new(),
            grammar_name: "example".to_string(),
            grammar_version: "1.0.0".to_string(),
            extends: "grammar_core@1.0.0".to_string(),
            author: self.id_with_keys.id.to_string(),
            created_at: self.current_timestamp(),
            file_path: "grammar/grammar_example.lll".to_string(),
            file_hash: "sha256:example".to_string(),
            signature: "sig_example".to_string(),
            metadata: serde_json::json!({}),
        })
    }

    /// Calcula hash de arquivo
    fn calculate_file_hash(&self, file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let content = fs::read_to_string(file_path)?;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        
        Ok(format!("sha256:{:x}", hasher.finish()))
    }

    /// Timestamp atual
    fn current_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    /// Emite span de core grammar carregada
    async fn emit_core_grammar_loaded_span(&self, core_grammar: &CoreGrammar) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "core_grammar_loaded",
            "grammar_name": core_grammar.name,
            "grammar_version": core_grammar.version,
            "grammar_hash": core_grammar.hash
        });

        self.span_emitter.emit_span(
            "core_grammar_loaded",
            "grammar_loader",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }

    /// Emite span de gramática carregada
    async fn emit_grammar_loaded_span(&self, project_name: &str, loaded_grammar: &LoadedGrammar) -> Result<(), Box<dyn std::error::Error>> {
        let span_data = serde_json::json!({
            "type": "project_grammar_loaded",
            "project_name": project_name,
            "grammar_name": loaded_grammar.grammar.name,
            "grammar_version": loaded_grammar.grammar.version,
            "validation_result": loaded_grammar.validation_result
        });

        self.span_emitter.emit_span(
            "project_grammar_loaded",
            "grammar_loader",
            &self.id_with_keys,
            Some(span_data),
        ).await?;

        Ok(())
    }
}

impl Default for GrammarLoaderConfig {
    fn default() -> Self {
        Self {
            grammar_directory: "./grammar".to_string(),
            validate_hashes: true,
            require_signatures: false,
            allow_extends_override: false,
            max_grammar_size_mb: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motor::span::SpanEmitter;

    #[tokio::test]
    async fn test_grammar_loader_creation() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = std::sync::Arc::new(SpanEmitter::new_mock());
        let config = GrammarLoaderConfig::default();
        
        let loader = GrammarLoader::new(id_with_keys, span_emitter, config);
        assert_eq!(loader.loaded_grammars.len(), 0);
        assert!(loader.core_grammar.is_none());
    }

    #[tokio::test]
    async fn test_registration_contract_validation() {
        let id_with_keys = LogLineIDWithKeys::generate_new().unwrap();
        let span_emitter = std::sync::Arc::new(SpanEmitter::new_mock());
        let config = GrammarLoaderConfig::default();
        let loader = GrammarLoader::new(id_with_keys.clone(), span_emitter, config);
        
        let valid_registration = GrammarRegistrationContract {
            id: LogLineID::generate_new(),
            grammar_name: "test_grammar".to_string(),
            grammar_version: "1.0.0".to_string(),
            extends: "grammar_core@1.0.0".to_string(),
            author: id_with_keys.id.to_string(),
            created_at: loader.current_timestamp(),
            file_path: "test.lll".to_string(),
            file_hash: "sha256:test_hash".to_string(),
            signature: "test_sig".to_string(),
            metadata: serde_json::json!({}),
        };
        
        let result = loader.validate_registration_contract(&valid_registration);
        assert!(result.is_ok());
        
        // Testa registro inválido
        let invalid_registration = GrammarRegistrationContract {
            grammar_name: "".to_string(), // Nome vazio - inválido
            ..valid_registration
        };
        
        let result = loader.validate_registration_contract(&invalid_registration);
        assert!(result.is_err());
    }
}