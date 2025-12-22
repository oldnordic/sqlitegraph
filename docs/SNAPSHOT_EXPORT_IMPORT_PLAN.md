# SQLiteGraph V2 Snapshot Export/Import Enhancement Plan

## Overview

This document outlines a comprehensive plan for implementing advanced SQLiteGraph V2 features:
- Snapshot Export/Import with instant database snapshots
- Minimal Planner Abstraction for export/import decision making
- CLI Validation Tools for operational use

## Current State Analysis

### ✅ Implemented Features
- **Authority Rules**: Complete WAL ↔ Graph authority resolution in `v2/wal/recovery/states.rs`
- **Export Modes**: CheckpointAligned, LsnBounded, Full exports
- **Import Modes**: Fresh and Merge imports with comprehensive validation
- **Error Handling**: Production-grade error propagation and conversion

### ❌ Missing Features
- **Snapshot Export/Import**: Instant database state snapshots without WAL complexity
- **Minimal Planner Abstraction**: Decision layer for export/import strategy
- **CLI Validation Tools**: Standalone command-line validation utilities

---

## Feature 1: Snapshot Export/Import Implementation

### 1.1 Snapshot Export Design

#### Core Concept
Snapshot export provides instant database state snapshots without WAL replay complexity, similar to database checkpoints but optimized for export/import workflows.

#### Architecture
```rust
// New export mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    CheckpointAligned,
    LsnBounded,
    Full,
    Snapshot,  // NEW: Instant database snapshot
}

// Snapshot configuration
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Snapshot identifier (human-readable)
    pub snapshot_id: String,

    /// Include database statistics
    pub include_statistics: bool,

    /// Compression level for snapshot
    pub compression_level: Option<u32>,

    /// Minimum snapshot age (require stable state for N seconds)
    pub min_stable_duration: Duration,
}
```

#### Implementation Plan

**Phase 1A: Snapshot Export Implementation**

**Location**: `src/backend/native/v2/export/snapshot.rs`

```rust
/// Snapshot exporter for instant database state exports
pub struct SnapshotExporter {
    config: SnapshotConfig,
    graph_file: GraphFile,
    wal_config: V2WALConfig,
}

impl SnapshotExporter {
    /// Create new snapshot exporter
    pub fn new(
        graph_path: &Path,
        snapshot_config: SnapshotConfig,
    ) -> NativeResult<Self> {
        // Implementation details:
        // 1. Open graph file and validate current state
        // 2. Ensure stable state (no active transactions)
        // 3. Capture database statistics if requested
        // 4. Create snapshot manifest with metadata
    }

    /// Create snapshot (instant capture)
    pub fn create_snapshot(&self) -> NativeResult<SnapshotResult> {
        // Implementation details:
        // 1. Validate stable state conditions
        // 2. Copy graph file atomically
        // 3. Generate snapshot manifest
        // 4. Calculate snapshot checksums
        // 5. Create snapshot metadata file
    }

    /// Validate snapshot before creation
    pub fn validate_snapshot_conditions(&self) -> NativeResult<SnapshotValidationReport> {
        // Implementation details:
        // 1. Check for active transactions
        // 2. Verify WAL state consistency
        // 3. Ensure sufficient stable duration
        // 4. Validate disk space availability
    }
}

/// Snapshot export result
#[derive(Debug, Clone)]
pub struct SnapshotResult {
    /// Snapshot identifier
    pub snapshot_id: String,

    /// Snapshot creation timestamp
    pub timestamp: u64,

    /// Snapshot file paths
    pub snapshot_files: Vec<PathBuf>,

    /// Snapshot metadata
    pub metadata: SnapshotMetadata,

    /// Export duration
    pub export_duration: Duration,
}
```

**Phase 1B: Snapshot Import Implementation**

**Location**: `src/backend/native/v2/import/snapshot.rs`

```rust
/// Snapshot importer for instant database restoration
pub struct SnapshotImporter {
    config: SnapshotImportConfig,
    snapshot_metadata: SnapshotMetadata,
    target_path: PathBuf,
}

impl SnapshotImporter {
    /// Create snapshot importer
    pub fn new(
        snapshot_dir: &Path,
        target_path: &Path,
        import_config: SnapshotImportConfig,
    ) -> NativeResult<Self> {
        // Implementation details:
        // 1. Read snapshot metadata
        // 2. Validate snapshot integrity
        // 3. Check target compatibility
        // 4. Set up import configuration
    }

    /// Import snapshot (instant restore)
    pub fn import_snapshot(&self) -> NativeResult<SnapshotImportResult> {
        // Implementation details:
        // 1. Validate target directory state
        // 2. Copy snapshot files atomically
        // 3. Update snapshot metadata
        // 4. Verify import integrity
    }

    /// Validate snapshot before import
    pub fn validate_snapshot(&self) -> NativeResult<SnapshotValidationReport> {
        // Implementation details:
        // 1. Verify snapshot file integrity
        // 2. Check format compatibility
        // 3. Validate target directory conditions
        // 4. Check disk space requirements
    }
}
```

#### Snapshot Metadata Format

**Location**: `src/backend/native/v2/export/snapshot_metadata.rs`

```rust
/// Snapshot metadata for snapshot identification and validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot format version
    pub version: u32,

    /// Snapshot identifier
    pub snapshot_id: String,

    /// Creation timestamp
    pub created_at: u64,

    /// Database statistics at snapshot time
    pub database_stats: DatabaseStatistics,

    /// File checksums for integrity verification
    pub file_checksums: HashMap<String, String>,

    /// SQLiteGraph version compatibility
    pub sqlitegraph_version: String,

    /// V2 format version
    pub v2_format_version: u32,

    /// Snapshot size information
    pub snapshot_size: u64,

    /// Compression algorithm used
    pub compression: Option<String>,
}

/// Database statistics captured in snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatistics {
    /// Total node count
    pub node_count: u64,

    /// Total edge count
    pub edge_count: u64,

    /// Database size in bytes
    pub database_size: u64,

    /// WAL information
    pub wal_info: Option<WalStatistics>,

    /// Performance metrics
    pub performance_metrics: Option<PerformanceMetrics>,
}
```

---

## Feature 2: Minimal Planner Abstraction

### 2.1 Planner Design

#### Core Concept
A minimal planner abstraction that decides the optimal export/import strategy based on system state, requirements, and constraints.

#### Architecture
```rust
/// Planner for export/import strategy decisions
pub trait ExportImportPlanner {
    /// Analyze current system state and requirements
    fn analyze(&self, context: &PlanningContext) -> PlanningAnalysis;

    /// Recommend optimal export strategy
    fn recommend_export_strategy(&self, analysis: &PlanningAnalysis) -> ExportRecommendation;

    /// Recommend optimal import strategy
    fn recommend_import_strategy(&self, analysis: &PlanningAnalysis) -> ImportRecommendation;

    /// Validate planned operation
    fn validate_plan(&self, plan: &ExecutionPlan) -> PlanValidationResult;
}

/// Planning context for decision making
#[derive(Debug, Clone)]
pub struct PlanningContext {
    /// Source/target paths
    pub source_path: Option<PathBuf>,
    pub target_path: Option<PathBuf>,

    /// User requirements
    pub requirements: OperationRequirements,

    /// System constraints
    pub constraints: SystemConstraints,

    /// Current system state
    pub system_state: SystemState,
}

/// Export strategy recommendation
#[derive(Debug, Clone)]
pub struct ExportRecommendation {
    /// Recommended export mode
    pub export_mode: ExportMode,

    /// Recommended configuration
    pub config: V2ExportConfig,

    /// Estimated execution time
    pub estimated_duration: Duration,

    /// Estimated storage requirements
    pub estimated_size: u64,

    /// Confidence level (0.0-1.0)
    pub confidence: f64,

    /// Reasoning for recommendation
    pub reasoning: Vec<String>,
}
```

#### Implementation Plan

**Phase 2A: Core Planner Implementation**

**Location**: `src/backend/native/v2/planning/core_planner.rs`

```rust
/// Core export/import planner implementation
pub struct CorePlanner {
    config: PlannerConfig,
}

impl ExportImportPlanner for CorePlanner {
    fn analyze(&self, context: &PlanningContext) -> PlanningAnalysis {
        // Implementation details:
        // 1. Analyze system state (WAL size, transaction activity)
        // 2. Evaluate requirements (consistency, performance)
        // 3. Consider constraints (disk space, time windows)
        // 4. Calculate feasibility metrics
    }

    fn recommend_export_strategy(&self, analysis: &PlanningAnalysis) -> ExportRecommendation {
        // Implementation details:
        // 1. Rule-based decision logic
        // 2. Cost-benefit analysis for different modes
        // 3. Performance impact estimation
        // 4. Risk assessment
    }

    fn recommend_import_strategy(&self, analysis: &PlanningAnalysis) -> ImportRecommendation {
        // Implementation details:
        // 1. Target compatibility assessment
        // 2. Import complexity evaluation
        // 3. Performance impact estimation
        // 4. Risk mitigation recommendations
    }

    fn validate_plan(&self, plan: &ExecutionPlan) -> PlanValidationResult {
        // Implementation details:
        // 1. Feasibility validation
        // 2. Resource requirement verification
        // 3. Risk assessment
        // 4. Dependency checking
    }
}

/// Rule-based decision engine for export/import strategies
pub struct RuleEngine {
    rules: Vec<PlanningRule>,
}

impl RuleEngine {
    /// Create new rule engine with default rules
    pub fn new() -> Self {
        // Implementation details:
        // 1. Load default planning rules
        // 2. Configure rule priorities
        // 3. Set up rule dependencies
    }

    /// Evaluate all rules against context
    pub fn evaluate(&self, context: &PlanningContext) -> Vec<RuleEvaluation> {
        // Implementation details:
        // 1. Apply each rule to context
        // 2. Calculate rule scores
        // 3. Aggregate rule results
        // 4. Generate recommendations
    }
}
```

**Phase 2B: Planning Rules Implementation**

**Location**: `src/backend/native/v2/planning/rules.rs`

```rust
/// Planning rule for export/import decision making
pub trait PlanningRule {
    /// Rule identifier
    fn id(&self) -> &str;

    /// Rule priority (higher = more important)
    fn priority(&self) -> u32;

    /// Evaluate rule against planning context
    fn evaluate(&self, context: &PlanningContext) -> RuleEvaluation;

    /// Check if rule applies to given context
    fn applies(&self, context: &PlanningContext) -> bool;
}

/// Rule evaluation result
#[derive(Debug, Clone)]
pub struct RuleEvaluation {
    /// Rule identifier
    pub rule_id: String,

    /// Rule priority
    pub priority: u32,

    /// Recommendation score (0.0-1.0)
    pub score: f64,

    /// Recommended action
    pub recommendation: Recommendation,

    /// Confidence level
    pub confidence: f64,

    /// Reasoning
    pub reasoning: String,
}

/// Rule: Use Snapshot for Instant Export
pub struct SnapshotExportRule;

impl PlanningRule for SnapshotExportRule {
    fn id(&self) -> &str { "snapshot_export_rule" }

    fn priority(&self) -> u32 { 100 } // High priority

    fn evaluate(&self, context: &PlanningContext) -> RuleEvaluation {
        // Implementation details:
        // 1. Check if instant export needed
        // 2. Verify stable state conditions
        // 3. Evaluate snapshot suitability
        // 4. Calculate recommendation score
    }

    fn applies(&self, context: &PlanningContext) -> bool {
        // Implementation details:
        // 1. Check user requirements for instant export
        // 2. Verify system supports snapshot mode
        // 3. Check time constraints
    }
}

/// Rule: Prefer WAL Replay for Maximum Consistency
pub struct ConsistencyRule;

impl PlanningRule for ConsistencyRule {
    fn id(&self) -> &str { "consistency_rule" }

    fn priority(&self) -> u32 { 90 }

    fn evaluate(&self, context: &PlanningContext) -> RuleEvaluation {
        // Implementation details:
        // 1. Evaluate consistency requirements
        // 2. Check WAL state and availability
        // 3. Assess replay complexity
        // 4. Generate consistency recommendation
    }

    fn applies(&self, context: &PlanningContext) -> bool {
        // Implementation details:
        // 1. Check consistency requirements
        // 2. Verify WAL file existence
        // 3. Assess replay feasibility
    }
}
```

---

## Feature 3: CLI Validation Tools

### 3.1 CLI Design

#### Core Concept
Command-line tools for validation, planning, and diagnostics of SQLiteGraph export/import operations.

#### Architecture
```rust
/// CLI command definitions
#[derive(Debug, Clone)]
pub enum ValidationCommand {
    /// Validate export directory
    ValidateExport {
        export_dir: PathBuf,
        detailed: bool,
    },

    /// Validate import target
    ValidateImport {
        export_dir: PathBuf,
        target_dir: PathBuf,
        dry_run: bool,
    },

    /// Plan export strategy
    PlanExport {
        source_dir: PathBuf,
        requirements: OperationRequirements,
    },

    /// Plan import strategy
    PlanImport {
        export_dir: PathBuf,
        target_dir: PathBuf,
        requirements: OperationRequirements,
    },

    /// System diagnostics
    Diagnostics {
        source_dir: Option<PathBuf>,
        target_dir: Option<PathBuf>,
    },
}
```

#### Implementation Plan

**Phase 3A: CLI Framework Implementation**

**Location**: `sqlitegraph/src/bin/sqlitegraph_validation.rs`

```rust
/// SQLiteGraph Validation CLI Tool
#[derive(Debug, Clone)]
struct ValidationCli {
    command: ValidationCommand,
    verbose: bool,
    output_format: OutputFormat,
}

impl ValidationCli {
    /// Execute validation command
    pub fn execute(&self) -> NativeResult<ValidationResult> {
        match &self.command {
            ValidationCommand::ValidateExport { export_dir, detailed } => {
                self.validate_export_command(export_dir, *detailed)
            }
            ValidationCommand::ValidateImport { export_dir, target_dir, dry_run } => {
                self.validate_import_command(export_dir, target_dir, *dry_run)
            }
            ValidationCommand::PlanExport { source_dir, requirements } => {
                self.plan_export_command(source_dir, requirements)
            }
            ValidationCommand::PlanImport { export_dir, target_dir, requirements } => {
                self.plan_import_command(export_dir, target_dir, requirements)
            }
            ValidationCommand::Diagnostics { source_dir, target_dir } => {
                self.diagnostics_command(source_dir, target_dir)
            }
        }
    }

    /// Validate export directory command
    fn validate_export_command(&self, export_dir: &Path, detailed: bool) -> NativeResult<ValidationResult> {
        // Implementation details:
        // 1. Check export directory existence
        // 2. Validate manifest file
        // 3. Verify export file integrity
        // 4. Generate detailed report if requested
        // 5. Return validation result
    }

    /// Validate import command
    fn validate_import_command(&self, export_dir: &Path, target_dir: &Path, dry_run: bool) -> NativeResult<ValidationResult> {
        // Implementation details:
        // 1. Load and validate export metadata
        // 2. Check target directory conditions
        // 3. Validate format compatibility
        // 4. Perform dry-run import if requested
        // 5. Generate validation report
    }

    /// Plan export command
    fn plan_export_command(&self, source_dir: &Path, requirements: &OperationRequirements) -> NativeResult<ValidationResult> {
        // Implementation details:
        // 1. Analyze source database state
        // 2. Evaluate user requirements
        // 3. Generate export recommendations
        // 4. Create execution plan
        // 5. Return planning result
    }

    /// Plan import command
    fn plan_import_command(&self, export_dir: &Path, target_dir: &Path, requirements: &OperationRequirements) -> NativeResult<ValidationResult> {
        // Implementation details:
        // 1. Analyze export and target states
        // 2. Evaluate import requirements
        // 3. Generate import recommendations
        // 4. Create execution plan
        // 5. Return planning result
    }

    /// Diagnostics command
    fn diagnostics_command(&self, source_dir: Option<&Path>, target_dir: Option<&Path>) -> NativeResult<ValidationResult> {
        // Implementation details:
        // 1. System state analysis
        // 2. Performance metrics collection
        // 3. Resource utilization analysis
        // 4. Health check reporting
        // 5. Generate diagnostic report
    }
}

/// Validation result structure
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Command executed
    pub command: String,

    /// Success status
    pub success: bool,

    /// Validation details
    pub details: ValidationDetails,

    /// Recommendations
    pub recommendations: Vec<String>,

    /// Execution duration
    pub duration: Duration,
}

/// Validation details
#[derive(Debug, Clone)]
pub struct ValidationDetails {
    /// Issues found
    pub issues: Vec<ValidationIssue>,

    /// Warnings generated
    pub warnings: Vec<ValidationWarning>,

    /// Information messages
    pub info: Vec<String>,

    /// Statistics
    pub statistics: ValidationStatistics,
}
```

**Phase 3B: CLI Integration**

**Location**: `sqlitegraph/src/bin/sqlitegraph.rs` (add to existing CLI)

```rust
// Add validation subcommand to existing CLI
#[derive(Debug, Clone)]
pub enum SqliteGraphCommand {
    // Existing commands...
    Validate(Box<ValidateArgs>),  // NEW
}

#[derive(Debug, Clone)]
pub struct ValidateArgs {
    pub subcommand: ValidateSubcommand,
}

#[derive(Debug, Clone)]
pub enum ValidateSubcommand {
    Export {
        export_dir: PathBuf,
        detailed: bool,
    },
    Import {
        export_dir: PathBuf,
        target_dir: PathBuf,
        dry_run: bool,
    },
    Plan {
        operation: PlanOperation,
        source: Option<PathBuf>,
        target: Option<PathBuf>,
        requirements_file: Option<PathBuf>,
    },
    Diagnostics {
        source: Option<PathBuf>,
        target: Option<PathBuf>,
    },
}

// Add validation command handler
impl SqliteGraphCommand {
    pub fn execute(self) -> NativeResult<()> {
        match self {
            // Existing command handling...
            SqliteGraphCommand::Validate(args) => {
                let cli = ValidationCli::new(args)?;
                cli.execute()?;
            }
        }
    }
}
```

---

## Implementation Timeline

### **Phase 1: Snapshot Export/Import (2 weeks)**
- Week 1: Snapshot export implementation with metadata and validation
- Week 2: Snapshot import implementation with integrity verification
- **Deliverable**: Complete snapshot export/import system

### **Phase 2: Planner Abstraction (2 weeks)**
- Week 1: Core planner implementation with rule engine
- Week 2: Planning rules and decision logic
- **Deliverable**: Minimal planner abstraction system

### **Phase 3: CLI Validation Tools (1 week)**
- Week 1: CLI framework and command implementations
- **Deliverable**: Complete CLI validation toolset

### **Phase 4: Integration & Testing (1 week)**
- Week 1: Integration testing and documentation
- **Deliverable**: Fully integrated system with comprehensive documentation

**Total Timeline: 6 weeks**

---

## Testing Strategy

### **Unit Tests**
- Snapshot export/import functionality
- Planner rule evaluation
- CLI command execution

### **Integration Tests**
- End-to-end snapshot workflows
- Planner decision making
- CLI tool integration

### **Performance Tests**
- Snapshot creation/restore performance
- Planning algorithm efficiency
- CLI tool responsiveness

### **System Tests**
- Database consistency validation
- Error handling and recovery
- Resource utilization under load

---

## Risk Assessment

### **Technical Risks**
- **Snapshot Consistency**: Ensuring atomic snapshot creation
- **Performance Impact**: Snapshot creation overhead on active databases
- **Storage Requirements**: Additional disk space for snapshots

### **Mitigation Strategies**
- **Consistency**: Implement proper locking and state validation
- **Performance**: Optimize snapshot algorithms and use copy-on-write where possible
- **Storage**: Implement snapshot rotation and cleanup policies

---

## Success Criteria

### **Functional Requirements**
- ✅ Snapshot export creates consistent database state
- ✅ Snapshot import restores database accurately
- ✅ Planner provides optimal strategy recommendations
- ✅ CLI tools provide comprehensive validation

### **Performance Requirements**
- ✅ Snapshot creation completes within acceptable timeframes
- ✅ Planner decisions are made quickly and accurately
- ✅ CLI tools provide timely feedback

### **Quality Requirements**
- ✅ All features maintain existing SQLiteGraph quality standards
- ✅ Comprehensive test coverage (90%+)
- ✅ Complete documentation and examples

---

**Document Status**: Active Implementation Plan
**Created**: 2025-12-21
**Next Review**: Before Phase 1 implementation begin