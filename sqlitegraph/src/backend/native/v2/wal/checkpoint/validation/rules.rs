//! Checkpoint Validation Rules
//!
//! This module provides validation rule definitions and the core validation engine
//! for V2 WAL checkpoint operations. It defines the rules for file integrity,
//! format validation, and basic validation criteria.

use crate::backend::native::v2::wal::V2WALConfig;
use crate::backend::native::v2::wal::checkpoint::constants::*;
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointResult};
use std::fs;

/// Validation rule severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationSeverity {
    /// Informational validation result
    Info = 0,
    /// Warning that doesn't prevent checkpoint operation
    Warning = 1,
    /// Error that prevents checkpoint operation
    Error = 2,
    /// Critical error that requires immediate attention
    Critical = 3,
}

/// Validation rule definition
#[derive(Debug, Clone)]
pub struct ValidationRule {
    /// Rule identifier
    pub id: String,
    /// Rule description
    pub description: String,
    /// Rule severity
    pub severity: ValidationSeverity,
    /// Whether the rule is enabled
    pub enabled: bool,
}

/// Validation rule engine for managing and executing validation rules
#[derive(Debug, Clone)]
pub struct ValidationRuleEngine {
    /// Available validation rules
    rules: Vec<ValidationRule>,
}

impl ValidationRuleEngine {
    /// Create a new validation rule engine with default rules
    pub fn new() -> Self {
        let mut engine = Self { rules: Vec::new() };
        engine.register_default_rules();
        engine
    }

    /// Register the default set of validation rules
    fn register_default_rules(&mut self) {
        self.rules.extend_from_slice(&[
            ValidationRule {
                id: "file_exists".to_string(),
                description: "Checkpoint file must exist".to_string(),
                severity: ValidationSeverity::Critical,
                enabled: true,
            },
            ValidationRule {
                id: "file_not_empty".to_string(),
                description: "Checkpoint file must not be empty".to_string(),
                severity: ValidationSeverity::Error,
                enabled: true,
            },
            ValidationRule {
                id: "min_file_size".to_string(),
                description: "Checkpoint file must meet minimum size requirements".to_string(),
                severity: ValidationSeverity::Error,
                enabled: true,
            },
            ValidationRule {
                id: "max_file_size".to_string(),
                description: "Checkpoint file must not exceed maximum size limits".to_string(),
                severity: ValidationSeverity::Error,
                enabled: true,
            },
            ValidationRule {
                id: "magic_number".to_string(),
                description: "Checkpoint file must have correct magic number".to_string(),
                severity: ValidationSeverity::Critical,
                enabled: true,
            },
            ValidationRule {
                id: "version_compatibility".to_string(),
                description: "Checkpoint file version must be supported".to_string(),
                severity: ValidationSeverity::Critical,
                enabled: true,
            },
            ValidationRule {
                id: "v2_metadata".to_string(),
                description: "V2-specific metadata must be valid".to_string(),
                severity: ValidationSeverity::Error,
                enabled: true,
            },
            ValidationRule {
                id: "lsn_range_valid".to_string(),
                description: "LSN range must be valid and contiguous".to_string(),
                severity: ValidationSeverity::Error,
                enabled: true,
            },
            ValidationRule {
                id: "block_size_valid".to_string(),
                description: "V2 block size must match expected value".to_string(),
                severity: ValidationSeverity::Critical,
                enabled: true,
            },
            ValidationRule {
                id: "cluster_alignment".to_string(),
                description: "Cluster alignment must be correct".to_string(),
                severity: ValidationSeverity::Error,
                enabled: true,
            },
        ]);
    }

    /// Add a custom validation rule
    pub fn add_rule(&mut self, rule: ValidationRule) {
        self.rules.push(rule);
    }

    /// Remove a validation rule by ID
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        if let Some(pos) = self.rules.iter().position(|r| r.id == rule_id) {
            self.rules.remove(pos);
            true
        } else {
            false
        }
    }

    /// Enable or disable a validation rule
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == rule_id) {
            rule.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Get all enabled rules of a specific severity level or higher
    pub fn get_enabled_rules(&self, min_severity: ValidationSeverity) -> Vec<&ValidationRule> {
        self.rules
            .iter()
            .filter(|r| r.enabled && r.severity >= min_severity)
            .collect()
    }

    /// Get rule by ID
    pub fn get_rule(&self, rule_id: &str) -> Option<&ValidationRule> {
        self.rules.iter().find(|r| r.id == rule_id)
    }

    /// Get all rules
    pub fn get_all_rules(&self) -> &[ValidationRule] {
        &self.rules
    }
}

/// File validation rules for checkpoint files
pub struct FileValidationRules;

impl FileValidationRules {
    /// Validate that checkpoint file exists
    pub fn validate_file_exists(checkpoint_path: &std::path::Path) -> CheckpointResult<bool> {
        if !checkpoint_path.exists() {
            return Ok(false);
        }
        Ok(true)
    }

    /// Validate file size constraints
    pub fn validate_file_size(checkpoint_path: &std::path::Path) -> CheckpointResult<()> {
        let metadata = fs::metadata(checkpoint_path).map_err(|e| {
            CheckpointError::validation(format!("Failed to read checkpoint metadata: {}", e))
        })?;

        // Check if file is empty
        if metadata.len() == 0 {
            return Err(CheckpointError::validation(
                "Checkpoint file is empty".to_string(),
            ));
        }

        // Check minimum size
        if metadata.len() < MIN_CHECKPOINT_SIZE {
            return Err(CheckpointError::validation(format!(
                "Checkpoint file too small: {} bytes (minimum: {})",
                metadata.len(),
                MIN_CHECKPOINT_SIZE
            )));
        }

        // Check maximum size
        if metadata.len() > MAX_CHECKPOINT_SIZE {
            return Err(CheckpointError::validation(format!(
                "Checkpoint file too large: {} bytes (maximum: {})",
                metadata.len(),
                MAX_CHECKPOINT_SIZE
            )));
        }

        Ok(())
    }

    /// Validate checkpoint file magic number
    pub fn validate_magic_number(file: &mut fs::File) -> CheckpointResult<()> {
        use std::io::Read;

        let mut magic = [0u8; 4];
        file.read_exact(&mut magic).map_err(|e| {
            CheckpointError::validation(format!("Failed to read checkpoint magic: {}", e))
        })?;

        if magic != *CHECKPOINT_MAGIC {
            return Err(CheckpointError::validation(format!(
                "Invalid checkpoint magic: expected {:?}, got {:?}",
                CHECKPOINT_MAGIC, magic
            )));
        }

        Ok(())
    }

    /// Validate checkpoint file version
    pub fn validate_version(file: &mut fs::File) -> CheckpointResult<u32> {
        use std::io::Read;

        let mut version_bytes = [0u8; 4];
        file.read_exact(&mut version_bytes).map_err(|e| {
            CheckpointError::validation(format!("Failed to read checkpoint version: {}", e))
        })?;

        let version = u32::from_le_bytes(version_bytes);
        if version != CHECKPOINT_VERSION {
            return Err(CheckpointError::validation(format!(
                "Unsupported checkpoint version: {} (supported: {})",
                version, CHECKPOINT_VERSION
            )));
        }

        Ok(version)
    }
}

/// Validation rule configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Enable strict validation mode (all rules enforced)
    pub strict_mode: bool,
    /// Minimum severity level for validation errors
    pub min_error_severity: ValidationSeverity,
    /// Allow warnings in non-strict mode
    pub allow_warnings: bool,
    /// Custom validation timeouts
    pub validation_timeout: std::time::Duration,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            strict_mode: false,
            min_error_severity: ValidationSeverity::Error,
            allow_warnings: true,
            validation_timeout: std::time::Duration::from_secs(30),
        }
    }
}

/// Validation context for rule execution
#[derive(Debug)]
pub struct ValidationContext<'a> {
    /// Checkpoint configuration
    pub config: &'a V2WALConfig,
    /// Validation configuration
    pub validation_config: ValidationConfig,
    /// Checkpoint file path
    pub checkpoint_path: &'a std::path::Path,
    /// Validation rule engine
    pub rule_engine: &'a ValidationRuleEngine,
}

impl<'a> ValidationContext<'a> {
    /// Create a new validation context
    pub fn new(
        config: &'a V2WALConfig,
        checkpoint_path: &'a std::path::Path,
        rule_engine: &'a ValidationRuleEngine,
    ) -> Self {
        Self {
            config,
            validation_config: ValidationConfig::default(),
            checkpoint_path,
            rule_engine,
        }
    }

    /// Create validation context with custom validation config
    pub fn with_validation_config(
        config: &'a V2WALConfig,
        checkpoint_path: &'a std::path::Path,
        rule_engine: &'a ValidationRuleEngine,
        validation_config: ValidationConfig,
    ) -> Self {
        Self {
            config,
            validation_config,
            checkpoint_path,
            rule_engine,
        }
    }

    /// Check if a rule should be executed based on configuration
    pub fn should_execute_rule(&self, rule: &ValidationRule) -> bool {
        if !rule.enabled {
            return false;
        }

        if self.validation_config.strict_mode {
            return true; // In strict mode, execute all enabled rules
        }

        // In non-strict mode, respect severity and warning configuration
        if rule.severity >= self.validation_config.min_error_severity {
            return true;
        }

        if self.validation_config.allow_warnings && rule.severity == ValidationSeverity::Warning {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
        use tempfile::tempdir;

    #[test]
    fn test_validation_rule_engine_creation() {
        let engine = ValidationRuleEngine::new();
        let rules = engine.get_all_rules();
        assert!(
            !rules.is_empty(),
            "Engine should have default rules registered"
        );

        // Check for key default rules
        assert!(engine.get_rule("file_exists").is_some());
        assert!(engine.get_rule("magic_number").is_some());
        assert!(engine.get_rule("version_compatibility").is_some());
    }

    #[test]
    fn test_validation_rule_management() {
        let mut engine = ValidationRuleEngine::new();

        // Add custom rule
        let custom_rule = ValidationRule {
            id: "test_rule".to_string(),
            description: "Test rule".to_string(),
            severity: ValidationSeverity::Warning,
            enabled: true,
        };
        engine.add_rule(custom_rule.clone());

        assert!(engine.get_rule("test_rule").is_some());

        // Disable rule
        assert!(engine.set_rule_enabled("test_rule", false));
        assert!(!engine.get_rule("test_rule").unwrap().enabled);

        // Remove rule
        assert!(engine.remove_rule("test_rule"));
        assert!(engine.get_rule("test_rule").is_none());
    }

    #[test]
    fn test_validation_severity_ordering() {
        assert!(ValidationSeverity::Info < ValidationSeverity::Warning);
        assert!(ValidationSeverity::Warning < ValidationSeverity::Error);
        assert!(ValidationSeverity::Error < ValidationSeverity::Critical);
    }

    #[test]
    fn test_file_validation_rules_file_not_exists() {
        let temp_dir = tempdir().unwrap();
        let non_existent_path = temp_dir.path().join("nonexistent.checkpoint");

        let result = FileValidationRules::validate_file_exists(&non_existent_path);
        assert!(result.unwrap() == false);
    }

    #[test]
    fn test_file_validation_rules_empty_file() {
        let temp_dir = tempdir().unwrap();
        let empty_path = temp_dir.path().join("empty.checkpoint");
        fs::write(&empty_path, b"").unwrap();

        let result = FileValidationRules::validate_file_size(&empty_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_file_validation_rules_size_limits() {
        let temp_dir = tempdir().unwrap();

        // Test file too small
        let small_path = temp_dir.path().join("small.checkpoint");
        fs::write(&small_path, vec![0u8; 100]).unwrap(); // 100 bytes < MIN_CHECKPOINT_SIZE

        let result = FileValidationRules::validate_file_size(&small_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too small"));
    }

    #[test]
    fn test_validation_config() {
        let config = ValidationConfig::default();
        assert!(!config.strict_mode);
        assert_eq!(config.min_error_severity, ValidationSeverity::Error);
        assert!(config.allow_warnings);

        let strict_config = ValidationConfig {
            strict_mode: true,
            min_error_severity: ValidationSeverity::Warning,
            allow_warnings: false,
            validation_timeout: std::time::Duration::from_secs(60),
        };
        assert!(strict_config.strict_mode);
        assert_eq!(
            strict_config.min_error_severity,
            ValidationSeverity::Warning
        );
        assert!(!strict_config.allow_warnings);
    }

    #[test]
    fn test_validation_context() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };
        let engine = ValidationRuleEngine::new();
        let checkpoint_path = temp_dir.path().join("test.checkpoint");

        let context = ValidationContext::new(&config, &checkpoint_path, &engine);
        assert!(!context.validation_config.strict_mode);

        let strict_context = ValidationContext::with_validation_config(
            &config,
            &checkpoint_path,
            &engine,
            ValidationConfig {
                strict_mode: true,
                ..Default::default()
            },
        );
        assert!(strict_context.validation_config.strict_mode);
    }

    #[test]
    fn test_validation_context_rule_execution() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };
        let engine = ValidationRuleEngine::new();
        let checkpoint_path = temp_dir.path().join("test.checkpoint");

        let context = ValidationContext::new(&config, &checkpoint_path, &engine);

        // In non-strict mode with default config, error and critical rules should execute
        let critical_rule = ValidationRule {
            id: "critical_test".to_string(),
            description: "Critical test rule".to_string(),
            severity: ValidationSeverity::Critical,
            enabled: true,
        };

        let warning_rule = ValidationRule {
            id: "warning_test".to_string(),
            description: "Warning test rule".to_string(),
            severity: ValidationSeverity::Warning,
            enabled: true,
        };

        assert!(context.should_execute_rule(&critical_rule));
        assert!(context.should_execute_rule(&warning_rule)); // Warnings allowed by default

        // With warnings disabled
        let no_warnings_config = ValidationConfig {
            allow_warnings: false,
            ..Default::default()
        };
        let no_warnings_context = ValidationContext::with_validation_config(
            &config,
            &checkpoint_path,
            &engine,
            no_warnings_config,
        );

        assert!(no_warnings_context.should_execute_rule(&critical_rule));
        assert!(!no_warnings_context.should_execute_rule(&warning_rule));
    }
}
