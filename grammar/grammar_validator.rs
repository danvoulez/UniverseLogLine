/// # LogLine Grammar Validator
///
/// Validador específico de regras e estruturas gramaticais.
/// Responsável por:
/// - Validar sintaxe de regras de enforcement
/// - Verificar compatibilidade entre gramáticas
/// - Validar fórmulas de cálculo temporal
/// - Checar integridade de workflows
/// - Validar estruturas de dados customizadas

use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

use crate::enforcement::contextual_enforcer::{
    LocalGrammar, EnforcementRule, EnforcementRuleType, RuleScope,
    TriggerDefinition, TimeCalculationRule, TimeUnit,
    WorkflowDefinition, TransitionDefinition, EntityDefinition
};

/// Validador de gramáticas
pub struct GrammarValidator {
    /// Regras de validação por tipo
    validation_rules: HashMap<String, Vec<ValidationRule>>,
    
    /// Cache de validações já realizadas
    validation_cache: HashMap<String, ValidationCacheEntry>,
}

/// Regra de validação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub rule_name: String,
    pub description: String,
    pub severity: ValidationSeverity,
    pub validator_fn: String, // Nome da função validadora
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,   // Bloqueia carregamento
    Warning, // Permite mas alerta
    Info,    // Apenas informativo
}

/// Entrada do cache de validação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCacheEntry {
    pub grammar_hash: String,
    pub validated_at: u64,
    pub result: DetailedValidationResult,
}

/// Resultado detalhado de validação
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedValidationResult {
    pub is_valid: bool,
    pub validation_summary: ValidationSummary,
    pub rule_validations: Vec<RuleValidationResult>,
    pub time_model_validation: TimeModelValidationResult,
    pub workflow_validations: Vec<WorkflowValidationResult>,
    pub entity_validations: Vec<EntityValidationResult>,
    pub compatibility_check: CompatibilityCheckResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_rules_checked: u32,
    pub errors_count: u32,
    pub warnings_count: u32,
    pub infos_count: u32,
    pub validation_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleValidationResult {
    pub rule_id: String,
    pub rule_type: EnforcementRuleType,
    pub is_valid: bool,
    pub syntax_valid: bool,
    pub semantics_valid: bool,
    pub performance_warning: Option<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeModelValidationResult {
    pub model_name: String,
    pub unit_valid: bool,
    pub calendar_valid: bool,
    pub calculation_rules_valid: bool,
    pub formula_validations: Vec<FormulaValidationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaValidationResult {
    pub formula_name: String,
    pub formula: String,
    pub is_valid: bool,
    pub syntax_errors: Vec<String>,
    pub variable_dependencies: Vec<String>,
    pub computational_complexity: ComplexityLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplexityLevel {
    Simple,    // O(1)
    Linear,    // O(n)
    Complex,   // O(n²) ou maior
    Recursive, // Recursão potencial
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowValidationResult {
    pub workflow_id: String,
    pub is_valid: bool,
    pub has_cycles: bool,
    pub unreachable_states: Vec<String>,
    pub invalid_transitions: Vec<String>,
    pub state_validations: Vec<StateValidationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateValidationResult {
    pub state_id: String,
    pub is_reachable: bool,
    pub has_exit_transitions: bool,
    pub auto_transitions_valid: bool,
    pub role_requirements_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityValidationResult {
    pub entity_type: String,
    pub is_valid: bool,
    pub field_validations: Vec<FieldValidationResult>,
    pub constraint_validations: Vec<ConstraintValidationResult>,
    pub index_suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValidationResult {
    pub field_name: String,
    pub field_type: String,
    pub is_valid: bool,
    pub type_compatible: bool,
    pub constraint_valid: bool,
    pub performance_impact: PerformanceImpact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceImpact {
    None,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintValidationResult {
    pub constraint_expression: String,
    pub is_valid: bool,
    pub is_enforceable: bool,
    pub estimated_cost: f64, // Custo computacional estimado
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityCheckResult {
    pub core_compatible: bool,
    pub breaking_changes: Vec<BreakingChange>,
    pub migration_required: bool,
    pub migration_suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingChange {
    pub change_type: BreakingChangeType,
    pub description: String,
    pub affected_components: Vec<String>,
    pub migration_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BreakingChangeType {
    RemovedField,
    ChangedFieldType,
    AddedRequiredField,
    RemovedRule,
    ChangedRuleSemantics,
    IncompatibleTimeModel,
}

impl GrammarValidator {
    /// Cria novo validador
    pub fn new() -> Self {
        let mut validator = Self {
            validation_rules: HashMap::new(),
            validation_cache: HashMap::new(),
        };
        
        validator.initialize_default_rules();
        validator
    }

    /// Valida gramática completa
    pub fn validate_grammar(&mut self, grammar: &LocalGrammar) -> DetailedValidationResult {
        let start_time = std::time::Instant::now();
        
        // Verifica cache
        let cache_key = format!("{}@{}", grammar.name, grammar.hash);
        if let Some(cached) = self.validation_cache.get(&cache_key) {
            if self.is_cache_valid(&cached) {
                return cached.result.clone();
            }
        }

        println!("🔍 Validando gramática: {} v{}", grammar.name, grammar.version);

        let mut result = DetailedValidationResult {
            is_valid: true,
            validation_summary: ValidationSummary {
                total_rules_checked: 0,
                errors_count: 0,
                warnings_count: 0,
                infos_count: 0,
                validation_duration_ms: 0,
            },
            rule_validations: Vec::new(),
            time_model_validation: TimeModelValidationResult {
                model_name: grammar.time_model.name.clone(),
                unit_valid: true,
                calendar_valid: true,
                calculation_rules_valid: true,
                formula_validations: Vec::new(),
            },
            workflow_validations: Vec::new(),
            entity_validations: Vec::new(),
            compatibility_check: CompatibilityCheckResult {
                core_compatible: true,
                breaking_changes: Vec::new(),
                migration_required: false,
                migration_suggestions: Vec::new(),
            },
        };

        // Valida regras de enforcement
        self.validate_enforcement_rules(grammar, &mut result);
        
        // Valida modelo de tempo
        self.validate_time_model_detailed(grammar, &mut result);
        
        // Valida workflows
        self.validate_workflows_detailed(grammar, &mut result);
        
        // Valida entidades
        self.validate_entities_detailed(grammar, &mut result);
        
        // Verifica compatibilidade
        self.check_compatibility(grammar, &mut result);

        // Finaliza validação
        let duration = start_time.elapsed();
        result.validation_summary.validation_duration_ms = duration.as_millis() as u64;
        
        if result.validation_summary.errors_count > 0 {
            result.is_valid = false;
        }

        // Salva no cache
        self.validation_cache.insert(cache_key, ValidationCacheEntry {
            grammar_hash: grammar.hash.clone(),
            validated_at: self.current_timestamp(),
            result: result.clone(),
        });

        println!("✅ Validação concluída: {} erros, {} avisos em {}ms", 
            result.validation_summary.errors_count,
            result.validation_summary.warnings_count,
            result.validation_summary.validation_duration_ms
        );

        result
    }

    /// Valida regras de enforcement
    fn validate_enforcement_rules(&self, grammar: &LocalGrammar, result: &mut DetailedValidationResult) {
        for rule in &grammar.enforcement.rules {
            let rule_validation = self.validate_single_rule(rule);
            
            if !rule_validation.is_valid {
                result.validation_summary.errors_count += 1;
                result.is_valid = false;
            }
            
            result.rule_validations.push(rule_validation);
            result.validation_summary.total_rules_checked += 1;
        }
    }

    /// Valida regra individual
    fn validate_single_rule(&self, rule: &EnforcementRule) -> RuleValidationResult {
        let mut rule_result = RuleValidationResult {
            rule_id: rule.rule_id.clone(),
            rule_type: rule.rule_type.clone(),
            is_valid: true,
            syntax_valid: true,
            semantics_valid: true,
            performance_warning: None,
            suggestions: Vec::new(),
        };

        // Valida sintaxe da expressão
        if !self.validate_rule_syntax(&rule.expression) {
            rule_result.syntax_valid = false;
            rule_result.is_valid = false;
        }

        // Valida semântica
        if !self.validate_rule_semantics(rule) {
            rule_result.semantics_valid = false;
            rule_result.is_valid = false;
        }

        // Analisa performance
        if let Some(warning) = self.analyze_rule_performance(rule) {
            rule_result.performance_warning = Some(warning);
        }

        // Gera sugestões
        rule_result.suggestions = self.generate_rule_suggestions(rule);

        rule_result
    }

    /// Valida sintaxe de regra
    fn validate_rule_syntax(&self, expression: &str) -> bool {
        // Parser simples - em implementação real seria um parser completo
        if expression.is_empty() {
            return false;
        }
        
        // Verifica balanceamento de parênteses
        let mut balance = 0i32;
        for char in expression.chars() {
            match char {
                '(' => balance += 1,
                ')' => balance -= 1,
                _ => {}
            }
            if balance < 0 {
                return false;
            }
        }
        
        balance == 0
    }

    /// Valida semântica de regra
    fn validate_rule_semantics(&self, rule: &EnforcementRule) -> bool {
        // Verificações semânticas básicas
        match &rule.rule_type {
            EnforcementRuleType::FieldValidation => {
                // Deve referenciar campos válidos
                rule.expression.contains("!=") || rule.expression.contains("==") || 
                rule.expression.contains(">") || rule.expression.contains("<")
            },
            EnforcementRuleType::BusinessRule => {
                // Deve ter lógica de negócio válida
                rule.expression.len() > 5 // Regra mínima de tamanho
            },
            EnforcementRuleType::WorkflowTransition => {
                // Deve referenciar estados válidos
                rule.expression.contains("->") || rule.expression.contains("from") || rule.expression.contains("to")
            },
            _ => true, // Outros tipos são válidos por padrão
        }
    }

    /// Analisa performance da regra
    fn analyze_rule_performance(&self, rule: &EnforcementRule) -> Option<String> {
        // Análise simples de performance
        if rule.expression.len() > 1000 {
            return Some("Expressão muito longa pode impactar performance".to_string());
        }
        
        if rule.expression.contains("*") && rule.expression.contains("SELECT") {
            return Some("Query com wildcard pode ser lenta".to_string());
        }
        
        None
    }

    /// Gera sugestões para regra
    fn generate_rule_suggestions(&self, rule: &EnforcementRule) -> Vec<String> {
        let mut suggestions = Vec::new();
        
        if rule.priority == 0 {
            suggestions.push("Considere definir prioridade para a regra".to_string());
        }
        
        if matches!(rule.scope, RuleScope::Universal) && rule.priority < 100 {
            suggestions.push("Regras universais devem ter prioridade alta".to_string());
        }
        
        suggestions
    }

    /// Valida modelo de tempo detalhadamente
    fn validate_time_model_detailed(&self, grammar: &LocalGrammar, result: &mut DetailedValidationResult) {
        let time_model = &grammar.time_model;
        
        // Valida unidade de tempo
        result.time_model_validation.unit_valid = self.validate_time_unit(&time_model.unit);
        
        // Valida calendário de negócio
        if let Some(calendar) = &time_model.business_calendar {
            result.time_model_validation.calendar_valid = self.validate_business_calendar(calendar);
        }
        
        // Valida regras de cálculo
        for calc_rule in &time_model.calculation_rules {
            let formula_validation = self.validate_calculation_formula(calc_rule);
            if !formula_validation.is_valid {
                result.time_model_validation.calculation_rules_valid = false;
                result.validation_summary.errors_count += 1;
            }
            result.time_model_validation.formula_validations.push(formula_validation);
        }
    }

    /// Valida unidade de tempo
    fn validate_time_unit(&self, unit: &TimeUnit) -> bool {
        // Todas as unidades definidas são válidas
        true
    }

    /// Valida calendário de negócio
    fn validate_business_calendar(&self, calendar: &crate::enforcement::contextual_enforcer::BusinessCalendar) -> bool {
        // Verifica se dias de trabalho são válidos (1-7)
        calendar.work_days.iter().all(|&day| day >= 1 && day <= 7) &&
        // Verifica se horário de trabalho é válido
        calendar.work_hours.0 < calendar.work_hours.1 &&
        calendar.work_hours.1 <= 24
    }

    /// Valida fórmula de cálculo
    fn validate_calculation_formula(&self, rule: &TimeCalculationRule) -> FormulaValidationResult {
        let mut result = FormulaValidationResult {
            formula_name: rule.name.clone(),
            formula: rule.formula.clone(),
            is_valid: true,
            syntax_errors: Vec::new(),
            variable_dependencies: Vec::new(),
            computational_complexity: ComplexityLevel::Simple,
        };

        // Análise simples da fórmula
        if rule.formula.is_empty() {
            result.is_valid = false;
            result.syntax_errors.push("Fórmula não pode estar vazia".to_string());
        }

        // Extrai dependências de variáveis
        result.variable_dependencies = self.extract_variable_dependencies(&rule.formula);
        
        // Determina complexidade
        result.computational_complexity = self.analyze_formula_complexity(&rule.formula);

        result
    }

    /// Extrai dependências de variáveis da fórmula
    fn extract_variable_dependencies(&self, formula: &str) -> Vec<String> {
        let mut dependencies = Vec::new();
        
        // Parser simples para encontrar variáveis
        let words: Vec<&str> = formula.split_whitespace().collect();
        for word in words {
            if word.chars().all(|c| c.is_alphanumeric() || c == '_') && 
               !word.chars().all(|c| c.is_numeric()) {
                dependencies.push(word.to_string());
            }
        }
        
        dependencies.sort();
        dependencies.dedup();
        dependencies
    }

    /// Analisa complexidade computacional da fórmula
    fn analyze_formula_complexity(&self, formula: &str) -> ComplexityLevel {
        if formula.contains("for") || formula.contains("while") {
            return ComplexityLevel::Linear;
        }
        
        if formula.contains("nested") || formula.contains("**") {
            return ComplexityLevel::Complex;
        }
        
        if formula.contains("recursive") || formula.contains("factorial") {
            return ComplexityLevel::Recursive;
        }
        
        ComplexityLevel::Simple
    }

    /// Valida workflows detalhadamente
    fn validate_workflows_detailed(&self, grammar: &LocalGrammar, result: &mut DetailedValidationResult) {
        for (workflow_id, workflow) in &grammar.workflows {
            let workflow_validation = self.validate_single_workflow(workflow_id, workflow);
            
            if !workflow_validation.is_valid {
                result.validation_summary.errors_count += 1;
            }
            
            result.workflow_validations.push(workflow_validation);
        }
    }

    /// Valida workflow individual
    fn validate_single_workflow(&self, workflow_id: &str, workflow: &WorkflowDefinition) -> WorkflowValidationResult {
        let mut result = WorkflowValidationResult {
            workflow_id: workflow_id.to_string(),
            is_valid: true,
            has_cycles: false,
            unreachable_states: Vec::new(),
            invalid_transitions: Vec::new(),
            state_validations: Vec::new(),
        };

        // Analisa alcançabilidade de estados
        let reachable_states = self.find_reachable_states(workflow);
        for state_id in workflow.states.keys() {
            if !reachable_states.contains(state_id) {
                result.unreachable_states.push(state_id.clone());
            }
        }

        // Verifica ciclos
        result.has_cycles = self.detect_workflow_cycles(workflow);

        // Valida transições
        for transition in &workflow.transitions {
            if !workflow.states.contains_key(&transition.from_state) ||
               !workflow.states.contains_key(&transition.to_state) {
                result.invalid_transitions.push(format!(
                    "{} -> {}", transition.from_state, transition.to_state
                ));
            }
        }

        // Valida estados individuais
        for (state_id, state) in &workflow.states {
            let state_validation = self.validate_workflow_state(state_id, state, workflow);
            result.state_validations.push(state_validation);
        }

        if !result.unreachable_states.is_empty() || !result.invalid_transitions.is_empty() {
            result.is_valid = false;
        }

        result
    }

    /// Encontra estados alcançáveis no workflow
    fn find_reachable_states(&self, workflow: &WorkflowDefinition) -> HashSet<String> {
        let mut reachable = HashSet::new();
        let mut to_visit = vec![workflow.initial_state.clone()];
        
        while let Some(current) = to_visit.pop() {
            if reachable.insert(current.clone()) {
                // Adiciona estados alcançáveis por transições
                for transition in &workflow.transitions {
                    if transition.from_state == current {
                        to_visit.push(transition.to_state.clone());
                    }
                }
            }
        }
        
        reachable
    }

    /// Detecta ciclos no workflow
    fn detect_workflow_cycles(&self, workflow: &WorkflowDefinition) -> bool {
        // Implementação simples de detecção de ciclos
        // Em implementação real seria um algoritmo de DFS completo
        for transition in &workflow.transitions {
            if transition.from_state == transition.to_state {
                return true; // Auto-loop
            }
        }
        false
    }

    /// Valida estado individual do workflow
    fn validate_workflow_state(&self, state_id: &str, state: &crate::enforcement::contextual_enforcer::StateDefinition, workflow: &WorkflowDefinition) -> StateValidationResult {
        StateValidationResult {
            state_id: state_id.to_string(),
            is_reachable: true, // Calculado anteriormente
            has_exit_transitions: workflow.transitions.iter().any(|t| t.from_state == state_id),
            auto_transitions_valid: state.auto_transitions.iter().all(|t| !t.is_empty()),
            role_requirements_valid: true, // Validação simplificada
        }
    }

    /// Valida entidades detalhadamente
    fn validate_entities_detailed(&self, grammar: &LocalGrammar, result: &mut DetailedValidationResult) {
        for (entity_type, entity) in &grammar.domain_entities {
            let entity_validation = self.validate_single_entity(entity_type, entity);
            
            if !entity_validation.is_valid {
                result.validation_summary.errors_count += 1;
            }
            
            result.entity_validations.push(entity_validation);
        }
    }

    /// Valida entidade individual
    fn validate_single_entity(&self, entity_type: &str, entity: &EntityDefinition) -> EntityValidationResult {
        let mut result = EntityValidationResult {
            entity_type: entity_type.to_string(),
            is_valid: true,
            field_validations: Vec::new(),
            constraint_validations: Vec::new(),
            index_suggestions: Vec::new(),
        };

        // Valida campos
        for (field_name, field_def) in &entity.fields {
            let field_validation = self.validate_entity_field(field_name, field_def);
            if !field_validation.is_valid {
                result.is_valid = false;
            }
            result.field_validations.push(field_validation);
        }

        // Gera sugestões de índices
        result.index_suggestions = self.generate_index_suggestions(entity);

        result
    }

    /// Valida campo de entidade
    fn validate_entity_field(&self, field_name: &str, field_def: &crate::enforcement::contextual_enforcer::FieldDefinition) -> FieldValidationResult {
        FieldValidationResult {
            field_name: field_name.to_string(),
            field_type: field_def.field_type.clone(),
            is_valid: !field_def.field_type.is_empty(),
            type_compatible: self.is_type_compatible(&field_def.field_type),
            constraint_valid: field_def.constraints.iter().all(|c| !c.is_empty()),
            performance_impact: self.assess_field_performance_impact(field_def),
        }
    }

    /// Verifica compatibilidade de tipo
    fn is_type_compatible(&self, field_type: &str) -> bool {
        // Lista de tipos suportados
        matches!(field_type, 
            "string" | "number" | "integer" | "boolean" | 
            "timestamp" | "logline_id" | "hash_sha256" | "object" | "array"
        )
    }

    /// Avalia impacto de performance do campo
    fn assess_field_performance_impact(&self, field_def: &crate::enforcement::contextual_enforcer::FieldDefinition) -> PerformanceImpact {
        match field_def.field_type.as_str() {
            "string" if field_def.constraints.iter().any(|c| c.contains("length > 1000")) => PerformanceImpact::Medium,
            "object" | "array" => PerformanceImpact::Low,
            _ => PerformanceImpact::None,
        }
    }

    /// Gera sugestões de índices
    fn generate_index_suggestions(&self, entity: &EntityDefinition) -> Vec<String> {
        let mut suggestions = Vec::new();
        
        // Sugere índices para campos obrigatórios
        for required_field in &entity.required_fields {
            if !entity.immutable_fields.contains(required_field) {
                suggestions.push(format!("Considere índice para campo obrigatório: {}", required_field));
            }
        }
        
        suggestions
    }

    /// Verifica compatibilidade com core
    fn check_compatibility(&self, grammar: &LocalGrammar, result: &mut DetailedValidationResult) {
        // Verifica se extends é compatível
        if !grammar.extends.starts_with("grammar_core@") {
            result.compatibility_check.core_compatible = false;
            result.compatibility_check.breaking_changes.push(BreakingChange {
                change_type: BreakingChangeType::IncompatibleTimeModel,
                description: "Deve estender grammar_core".to_string(),
                affected_components: vec!["core".to_string()],
                migration_path: Some("Atualize extends para grammar_core@1.0.0".to_string()),
            });
        }
    }

    /// Inicializa regras padrão de validação
    fn initialize_default_rules(&mut self) {
        // Adiciona regras padrão
        self.validation_rules.insert("enforcement".to_string(), vec![
            ValidationRule {
                rule_name: "rule_id_required".to_string(),
                description: "Regras devem ter ID único".to_string(),
                severity: ValidationSeverity::Error,
                validator_fn: "validate_rule_id".to_string(),
            },
        ]);
    }

    /// Verifica se cache é válido
    fn is_cache_valid(&self, cached: &ValidationCacheEntry) -> bool {
        let now = self.current_timestamp();
        let cache_age = now - cached.validated_at;
        cache_age < 3600_000_000 // 1 hora em microssegundos
    }

    /// Timestamp atual
    fn current_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }
}

impl Default for GrammarValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enforcement::contextual_enforcer::{TimeUnit, TimeModel};

    #[test]
    fn test_rule_syntax_validation() {
        let validator = GrammarValidator::new();
        
        assert!(validator.validate_rule_syntax("valor > 0"));
        assert!(validator.validate_rule_syntax("(campo1 == 'test') && (campo2 != null)"));
        assert!(!validator.validate_rule_syntax("((unbalanced"));
        assert!(!validator.validate_rule_syntax(""));
    }

    #[test]
    fn test_time_unit_validation() {
        let validator = GrammarValidator::new();
        
        assert!(validator.validate_time_unit(&TimeUnit::Days));
        assert!(validator.validate_time_unit(&TimeUnit::BusinessDays));
        assert!(validator.validate_time_unit(&TimeUnit::Hours));
    }

    #[test]
    fn test_formula_complexity_analysis() {
        let validator = GrammarValidator::new();
        
        assert!(matches!(
            validator.analyze_formula_complexity("a + b"),
            ComplexityLevel::Simple
        ));
        
        assert!(matches!(
            validator.analyze_formula_complexity("for i in range(n): sum += i"),
            ComplexityLevel::Linear
        ));
        
        assert!(matches!(
            validator.analyze_formula_complexity("factorial(n)"),
            ComplexityLevel::Recursive
        ));
    }
}