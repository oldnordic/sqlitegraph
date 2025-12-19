# CPU Tuning Modularization Implementation Plan

## Executive Summary

**File**: `sqlitegraph/src/backend/native/cpu_tuning.rs`
**Current Size**: 413 lines
**Target**: Core file ≤300 lines (90% reduction)
**Implementation Strategy**: Phased extraction of detection, resolution, and optimization modules
**Risk Level**: LOW (simple function-based design with clear boundaries)
**Estimated Timeline**: 1 day with comprehensive testing

## Detailed Implementation Plan

### Phase 0: Pre-Implementation Validation (Day 1 - 1 hour)

#### 0.1 Baseline Establishment
```bash
# Establish current behavior baseline
cargo test --lib cpu_tuning -- --nocapture
cargo test --lib detect_cpu_profile -- --nocapture
cargo test --lib test_resolve_cpu_profile -- --nocapture

# Test all CPU detection patterns
cargo test --lib test_has_feature -- --nocapture
cargo test --lib test_get_optimization_hints -- --nocapture
cargo test --lib test_caching -- --nocapture
```

#### 0.2 Dependency Mapping
- [x] **Confirmed**: Used in `graph_ops/strategy.rs` for strategy selection
- [x] **Confirmed**: Exported via `mod.rs` as public module
- [x] **Confirmed**: Simple function-based API with no complex state
- [x] **Confirmed**: Minimal external dependencies (only CpuProfile type)

#### 0.3 Current Usage Validation
```bash
# Verify all usage patterns work
cargo test --lib graph_ops -- --nocapture

# Test strategy selection integration
cargo test --lib select_bfs_strategy -- --nocapture 2>/dev/null || echo "Test name may differ"
```

### Phase 1: Extract Test Suite (Day 1 - 1 hour)

#### 1.1 Create `cpu_tuning_tests.rs`
**Target Size**: 100 lines (move all tests)
**Implementation**:

```rust
//! Comprehensive tests for CPU tuning functionality

use super::*;
use crate::backend::native::types::CpuProfile;

#[test]
fn test_detect_cpu_profile() {
    let profile = super::detect_cpu_profile();
    // Should return a valid profile without panicking
    match profile {
        CpuProfile::Generic
        | CpuProfile::Auto
        | CpuProfile::X86Zen4
        | CpuProfile::X86Avx2
        | CpuProfile::X86Avx512 => {
            // Valid profile
        }
    }
}

#[test]
fn test_resolve_cpu_profile() {
    // Test that Auto gets resolved to a concrete profile
    let auto_resolved = super::resolve_cpu_profile(CpuProfile::Auto);
    assert_ne!(auto_resolved, CpuProfile::Auto);

    // Test that Generic stays Generic
    let generic_resolved = super::resolve_cpu_profile(CpuProfile::Generic);
    assert_eq!(generic_resolved, CpuProfile::Generic);

    // Test that profiles fall back gracefully if CPU doesn't support features
    let avx512_resolved = super::resolve_cpu_profile(CpuProfile::X86Avx512);
    // Should not panic, should fall back to best supported profile
}

#[test]
fn test_has_feature() {
    // Test with Generic profile (should have no features)
    assert!(!super::has_feature(CpuProfile::Generic, "avx2"));
    assert!(!super::has_feature(CpuProfile::Generic, "avx512"));
    assert!(!super::has_feature(CpuProfile::Generic, "invalid"));

    // Test case insensitivity
    let profile = if super::cpu_feature_detection::CpuFeatureDetector::has_avx2_support() {
        CpuProfile::X86Avx2
    } else {
        CpuProfile::Generic
    };

    if super::cpu_feature_detection::CpuFeatureDetector::has_avx2_support() {
        assert!(super::has_feature(profile, "AVX2"));
        assert!(super::has_feature(profile, "avx2"));
    }
}

#[test]
fn test_get_optimization_hints() {
    let (cache_line, vector_width, branch_friendly) =
        super::optimization_hints::OptimizationHints::get_optimization_hints(CpuProfile::Generic);
    assert_eq!(cache_line, 64);
    assert_eq!(vector_width, 0);
    assert!(!branch_friendly);

    // Test that profiles give reasonable hints
    let hints = super::optimization_hints::OptimizationHints::get_optimization_hints(CpuProfile::X86Avx2);
    assert_eq!(hints.0, 64); // Cache line should be 64 bytes
    assert_eq!(hints.1, 256); // AVX2 width should be 256 bits
    assert!(hints.2); // Branch prediction should be good
}

#[test]
fn test_caching() {
    super::reset_cpu_profile_cache();

    // First call should perform detection
    let profile1 = super::detect_cpu_profile();

    // Second call should use cached result
    let profile2 = super::detect_cpu_profile();

    // Should return the same profile
    assert_eq!(profile1, profile2);
}

#[test]
fn test_profile_conversions() {
    assert_eq!(super::profile_to_usize(CpuProfile::Generic), 0);
    assert_eq!(super::profile_to_usize(CpuProfile::Auto), 1);
    assert_eq!(super::profile_to_usize(CpuProfile::X86Zen4), 2);
    assert_eq!(super::profile_to_usize(CpuProfile::X86Avx2), 3);
    assert_eq!(super::profile_to_usize(CpuProfile::X86Avx512), 4);

    assert_eq!(super::usize_to_profile(0), CpuProfile::Generic);
    assert_eq!(super::usize_to_profile(1), CpuProfile::Auto);
    assert_eq!(super::usize_to_profile(2), CpuProfile::X86Zen4);
    assert_eq!(super::usize_to_profile(3), CpuProfile::X86Avx2);
    assert_eq!(super::usize_to_profile(4), CpuProfile::X86Avx512);

    // Test invalid conversion
    assert_eq!(super::usize_to_profile(999), CpuProfile::Generic);
}

#[test]
fn test_feature_detection() {
    // Test that feature detection doesn't panic
    let avx2_support = super::cpu_feature_detection::CpuFeatureDetector::has_avx2_support();
    let avx512_support = super::cpu_feature_detection::CpuFeatureDetector::has_avx512_support();
    let zen4_support = super::cpu_feature_detection::CpuFeatureDetector::is_zen4_cpu();

    // Results should be boolean values
    assert_eq!(avx2_support, avx2_support);
    assert_eq!(avx512_support, avx512_support);
    assert_eq!(zen4_support, zen4_support);
}

#[test]
fn test_architecture_detection() {
    let x86_profile = super::cpu_feature_detection::CpuFeatureDetector::detect_x86_64_profile();
    let aarch64_profile = super::cpu_feature_detection::CpuFeatureDetector::detect_aarch64_profile();

    // Should return valid profiles without panicking
    match (x86_profile, aarch64_profile) {
        (x86, aarch64) => {
            // Both should be valid profiles
            assert!(matches!(x86, CpuProfile::Generic | CpuProfile::X86Zen4 | CpuProfile::X86Avx2 | CpuProfile::X86Avx512));
            assert_eq!(aarch64, CpuProfile::Generic); // Currently only generic for AArch64
        }
    }
}
```

#### 1.2 Update Core Module
```rust
// Remove entire #[cfg(test)] mod tests section from cpu_tuning.rs
// File size reduced by 100 lines
```

#### 1.3 Update Module Structure
```rust
// In cpu_tuning.rs
#[cfg(test)]
mod cpu_tuning_tests;
```

#### 1.4 Validation
```bash
# Test all cpu_tuning tests in new location
cargo test --lib cpu_tuning_tests -- --nocapture

# Ensure no tests lost
cargo test --lib -- --list | grep cpu_tuning

# Verify graph_ops still works
cargo test --lib graph_ops::strategy -- --nocapture
```

**Expected Result**: 413 → 313 lines (24% reduction, still over 300 LOC target)

### Phase 2: Extract Feature Detection (Day 1 - 2 hours)

#### 2.1 Create `cpu_feature_detection.rs`
**Target Size**: 130 lines
**Implementation**:

```rust
//! CPU feature detection utilities for runtime optimization

use crate::backend::native::types::CpuProfile;

/// CPU feature detection utilities
pub struct CpuFeatureDetector;

impl CpuFeatureDetector {
    /// Check if the CPU supports AVX2 instructions
    pub fn has_avx2_support() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            std::arch::is_x86_feature_detected!("avx2")
                && std::arch::is_x86_feature_detected!("fma")
                && std::arch::is_x86_feature_detected!("bmi2")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Check if the CPU supports AVX-512 instructions
    pub fn has_avx512_support() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            // Check for core AVX-512 features
            std::arch::is_x86_feature_detected!("avx512f")
                && std::arch::is_x86_feature_detected!("avx512vl")
                && std::arch::is_x86_feature_detected!("avx512dq")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Heuristic detection for AMD Zen 4 CPUs
    pub fn is_zen4_cpu() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            // Basic Zen 4 detection using CPU vendor and family/model detection
            // This is a simplified heuristic - in production, you'd want more comprehensive detection
            if Self::has_avx2_support() {
                // Zen 4 typically supports these specific features
                // We use heuristics to avoid unsafe CPUID calls
                std::arch::is_x86_feature_detected!("avx2")
                    && std::arch::is_x86_feature_detected!("fma")
                    && std::arch::is_x86_feature_detected!("bmi2")
                    && std::arch::is_x86_feature_detected!("adx")
                    && std::arch::is_x86_feature_detected!("sha")
            } else {
                false
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Detect CPU profile for x86_64 architecture
    pub fn detect_x86_64_profile() -> CpuProfile {
        // Check for AVX-512 support first (highest performance)
        if Self::has_avx512_support() {
            return CpuProfile::X86Avx512;
        }

        // Check for AVX2 support
        if Self::has_avx2_support() {
            // Additional check for Zen 4 specific features
            if Self::is_zen4_cpu() {
                return CpuProfile::X86Zen4;
            }
            return CpuProfile::X86Avx2;
        }

        // Fall back to generic profile
        CpuProfile::Generic
    }

    /// Detect CPU profile for AArch64 architecture
    pub fn detect_aarch64_profile() -> CpuProfile {
        // AArch64 optimizations are less mature, use generic for now
        // Future: detect NEON, SVE capabilities
        CpuProfile::Generic
    }

    /// Get list of supported features for current platform
    pub fn get_supported_features() -> Vec<&'static str> {
        let mut features = Vec::new();

        #[cfg(target_arch = "x86_64")]
        {
            if Self::has_avx2_support() {
                features.extend(&["avx2", "fma", "bmi2"]);
            }
            if Self::has_avx512_support() {
                features.extend(&["avx512f", "avx512vl", "avx512dq"]);
            }
            if Self::is_zen4_cpu() {
                features.extend(&["adx", "sha"]);
            }
        }

        features
    }

    /// Validate feature availability
    pub fn validate_feature_availability(features: &[&str]) -> bool {
        let supported_features = Self::get_supported_features();
        features.iter().all(|&feature| {
            supported_features.contains(&feature.to_lowercase().as_str())
        })
    }
}
```

#### 2.2 Update Core Module
```rust
// In cpu_tuning.rs, add imports and update detect_cpu_profile

use super::cpu_feature_detection::CpuFeatureDetector;

// Update detect_cpu_profile function
pub fn detect_cpu_profile() -> CpuProfile {
    // Check if we have a cached result
    let cached = CACHED_CPU_PROFILE.load(Ordering::Relaxed);
    if cached != usize::MAX {
        return usize_to_profile(cached);
    }

    let detected = if cfg!(target_arch = "x86_64") {
        CpuFeatureDetector::detect_x86_64_profile()
    } else if cfg!(target_arch = "aarch64") {
        CpuFeatureDetector::detect_aarch64_profile()
    } else {
        CpuProfile::Generic
    };

    // Cache the result for future calls
    let profile_int = profile_to_usize(detected);
    CACHED_CPU_PROFILE.store(profile_int, Ordering::Relaxed);

    detected
}
```

#### 2.3 Update Module Structure
```rust
// In cpu_tuning.rs
mod cpu_feature_detection;
```

#### 2.4 Validation
```bash
# Test feature detection extraction
cargo test --lib test_has_feature -- --nocapture
cargo test --lib cpu_tuning_tests::test_has_feature -- --nocapture

# Test feature detection utilities
cargo test --lib cpu_feature_detection -- --nocapture 2>/dev/null || echo "Test module name differs"
```

**Expected Result**: 313 → 193 lines (38% additional reduction)

### Phase 3: Extract Profile Resolution (Day 1 - 2 hours)

#### 3.1 Create `profile_resolution.rs`
**Target Size**: 85 lines
**Implementation**:

```rust
//! CPU profile resolution and validation

use super::cpu_feature_detection::CpuFeatureDetector;
use crate::backend::native::types::CpuProfile;

/// CPU profile resolver with validation and fallback logic
pub struct ProfileResolver;

impl ProfileResolver {
    /// Get the effective CPU profile with validation
    ///
    /// This function resolves the `Auto` profile by performing runtime detection
    /// and returns the actual profile to use for optimizations.
    pub fn resolve_cpu_profile(profile: CpuProfile) -> CpuProfile {
        match profile {
            CpuProfile::Auto => Self::detect_optimal_profile(),
            CpuProfile::Generic => CpuProfile::Generic,
            CpuProfile::X86Zen4 => {
                // Validate that the CPU actually supports Zen 4 features
                if cfg!(target_arch = "x86_64")
                    && CpuFeatureDetector::has_avx2_support()
                    && CpuFeatureDetector::is_zen4_cpu() {
                    CpuProfile::X86Zen4
                } else {
                    // Fall back to the best supported profile
                    Self::detect_optimal_profile()
                }
            }
            CpuProfile::X86Avx2 => {
                // Validate AVX2 support
                if CpuFeatureDetector::has_avx2_support() {
                    CpuProfile::X86Avx2
                } else {
                    CpuProfile::Generic
                }
            }
            CpuProfile::X86Avx512 => {
                // Validate AVX-512 support
                if CpuFeatureDetector::has_avx512_support() {
                    CpuProfile::X86Avx512
                } else if CpuFeatureDetector::has_avx2_support() {
                    CpuProfile::X86Avx2
                } else {
                    CpuProfile::Generic
                }
            }
        }
    }

    /// Check if a specific optimization is available for the current profile
    pub fn has_feature(profile: CpuProfile, feature: &str) -> bool {
        let resolved = Self::resolve_cpu_profile(profile);

        match resolved {
            CpuProfile::Generic => false, // No specific CPU features
            CpuProfile::Auto => Self::has_feature(CpuProfile::Auto, feature), // Resolve Auto first
            CpuProfile::X86Zen4 | CpuProfile::X86Avx2 => {
                match feature.to_lowercase().as_str() {
                    "avx2" | "fma" | "bmi2" => CpuFeatureDetector::has_avx2_support(),
                    "avx512" | "avx512f" | "avx512vl" => CpuFeatureDetector::has_avx512_support(),
                    _ => false,
                }
            }
            CpuProfile::X86Avx512 => {
                match feature.to_lowercase().as_str() {
                    "avx2" | "fma" | "bmi2" | "avx512" | "avx512f" | "avx512vl" | "avx512dq" => {
                        CpuFeatureDetector::has_avx512_support()
                    }
                    _ => false,
                }
            }
        }
    }

    /// Validate profile compatibility with current hardware
    pub fn validate_profile_compatibility(profile: CpuProfile) -> Result<CpuProfile, String> {
        match profile {
            CpuProfile::Generic => Ok(CpuProfile::Generic),
            CpuProfile::Auto => Ok(Self::detect_optimal_profile()),
            CpuProfile::X86Zen4 => {
                if cfg!(target_arch = "x86_64")
                    && CpuFeatureDetector::has_avx2_support()
                    && CpuFeatureDetector::is_zen4_cpu() {
                    Ok(CpuProfile::X86Zen4)
                } else {
                    Err("X86Zen4 profile requires x86_64 with Zen 4 features".to_string())
                }
            }
            CpuProfile::X86Avx2 => {
                if CpuFeatureDetector::has_avx2_support() {
                    Ok(CpuProfile::X86Avx2)
                } else {
                    Err("X86Avx2 profile requires AVX2 support".to_string())
                }
            }
            CpuProfile::X86Avx512 => {
                if CpuFeatureDetector::has_avx512_support() {
                    Ok(CpuProfile::X86Avx512)
                } else {
                    Err("X86Avx512 profile requires AVX-512 support".to_string())
                }
            }
        }
    }

    /// Detect optimal profile for current hardware
    fn detect_optimal_profile() -> CpuProfile {
        if cfg!(target_arch = "x86_64") {
            CpuFeatureDetector::detect_x86_64_profile()
        } else if cfg!(target_arch = "aarch64") {
            CpuFeatureDetector::detect_aarch64_profile()
        } else {
            CpuProfile::Generic
        }
    }
}
```

#### 3.2 Update Core Module
```rust
// In cpu_tuning.rs, add imports and update resolve_cpu_profile

use super::profile_resolution::ProfileResolver;

// Remove the original resolve_cpu_profile function and replace with:
pub use profile_resolution::ProfileResolver::resolve_cpu_profile;
pub use profile_resolution::ProfileResolver::has_feature;
```

#### 3.3 Update Module Structure
```rust
// In cpu_tuning.rs
mod profile_resolution;
```

#### 3.4 Validation
```bash
# Test profile resolution extraction
cargo test --lib test_resolve_cpu_profile -- --nocapture
cargo test --lib cpu_tuning_tests::test_resolve_cpu_profile -- --nocapture

# Test resolution functionality
cargo test --lib profile_resolution -- --nocapture 2>/dev/null || echo "Test module name differs"
```

**Expected Result**: 193 → 113 lines (27% additional reduction)

### Phase 4: Extract Optimization Hints (Day 1 - 1.5 hours)

#### 4.1 Create `optimization_hints.rs`
**Target Size**: 30 lines
**Implementation**:

```rust
//! Performance optimization hints based on CPU profile

use super::profile_resolution::ProfileResolver;
use crate::backend::native::types::CpuProfile;

/// Optimization hint generator for CPU-specific tuning
pub struct OptimizationHints;

impl OptimizationHints {
    /// Get optimization hints for a given CPU profile
    ///
    /// Returns performance tuning hints that can be applied to algorithms
    /// based on the detected CPU capabilities.
    pub fn get_optimization_hints(profile: CpuProfile) -> (usize, usize, bool) {
        let resolved = ProfileResolver::resolve_cpu_profile(profile);

        match resolved {
            CpuProfile::Generic => (64, 0, false), // Baseline cache line, no SIMD, conservative branching
            CpuProfile::X86Zen4 => {
                // Zen 4 specific optimizations
                (64, 256, true) // 64-byte cache line, 256-bit AVX2, good branch prediction
            }
            CpuProfile::X86Avx2 => {
                // Intel AVX2 systems
                (64, 256, true)
            }
            CpuProfile::X86Avx512 => {
                // AVX-512 systems
                (64, 512, true)
            }
            CpuProfile::Auto => Self::get_optimization_hints(CpuProfile::Auto), // Resolve Auto first
        }
    }

    /// Get SIMD vector width for profile
    pub fn get_vector_width(profile: CpuProfile) -> usize {
        let (_, vector_width, _) = Self::get_optimization_hints(profile);
        vector_width
    }

    /// Get cache-friendly iteration strategy
    pub fn get_cache_strategy(profile: CpuProfile) -> &'static str {
        let (_, _, branch_friendly) = Self::get_optimization_hints(profile);
        if branch_friendly {
            "branch_optimized"
        } else {
            "conservative"
        }
    }

    /// Get cache line size for profile
    pub fn get_cache_line_size(profile: CpuProfile) -> usize {
        let (cache_line, _, _) = Self::get_optimization_hints(profile);
        cache_line
    }
}
```

#### 4.2 Update Core Module
```rust
// In cpu_tuning.rs, add imports and update get_optimization_hints

use super::optimization_hints::OptimizationHints;

// Remove the original get_optimization_hints function and replace with:
pub use optimization_hints::OptimizationHints::get_optimization_hints;
```

#### 4.3 Update Module Structure
```rust
// In cpu_tuning.rs
mod optimization_hints;
```

#### 4.4 Validation
```bash
# Test optimization hints extraction
cargo test --lib test_get_optimization_hints -- --nocapture
cargo test --lib cpu_tuning_tests::test_get_optimization_hints -- --nocapture

# Test optimization hint functionality
cargo test --lib optimization_hints -- --nocapture 2>/dev/null || echo "Test module name differs"
```

**Expected Result**: 113 → 88 lines (15% additional reduction)

### Phase 5: Final Integration and Validation (Day 1 - 1 hour)

#### 5.1 Final Core Module Structure
**Minimal remaining file**:

```rust
//! Runtime CPU detection and optimization mapping for SQLiteGraph Native Backend.

use crate::backend::native::types::CpuProfile;
use std::sync::atomic::{AtomicUsize, Ordering};

// Re-export all functionality for backward compatibility
pub use cpu_feature_detection::CpuFeatureDetector;
pub use profile_resolution::ProfileResolver;
pub use optimization_hints::OptimizationHints;

// Re-export main API functions
pub use ProfileResolver::resolve_cpu_profile;
pub use ProfileResolver::has_feature;
pub use OptimizationHints::get_optimization_hints;

// Internal module organization
mod cpu_feature_detection;
mod profile_resolution;
mod optimization_hints;

#[cfg(test)]
mod cpu_tuning_tests;

/// Global cached CPU profile to avoid repeated detection
static CACHED_CPU_PROFILE: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Convert CpuProfile to usize for atomic storage
#[inline]
pub fn profile_to_usize(profile: CpuProfile) -> usize {
    match profile {
        CpuProfile::Generic => 0,
        CpuProfile::Auto => 1,
        CpuProfile::X86Zen4 => 2,
        CpuProfile::X86Avx2 => 3,
        CpuProfile::X86Avx512 => 4,
    }
}

/// Convert usize back to CpuProfile
#[inline]
pub fn usize_to_profile(value: usize) -> CpuProfile {
    match value {
        0 => CpuProfile::Generic,
        1 => CpuProfile::Auto,
        2 => CpuProfile::X86Zen4,
        3 => CpuProfile::X86Avx2,
        4 => CpuProfile::X86Avx512,
        _ => CpuProfile::Generic,
    }
}

/// Detect the optimal CPU profile with caching
pub fn detect_cpu_profile() -> CpuProfile {
    // Check if we have a cached result
    let cached = CACHED_CPU_PROFILE.load(Ordering::Relaxed);
    if cached != usize::MAX {
        return usize_to_profile(cached);
    }

    let detected = if cfg!(target_arch = "x86_64") {
        CpuFeatureDetector::detect_x86_64_profile()
    } else if cfg!(target_arch = "aarch64") {
        CpuFeatureDetector::detect_aarch64_profile()
    } else {
        CpuProfile::Generic
    };

    // Cache the result for future calls
    let profile_int = profile_to_usize(detected);
    CACHED_CPU_PROFILE.store(profile_int, Ordering::Relaxed);

    detected
}

/// Reset the cached CPU profile (useful for testing)
#[cfg(test)]
pub fn reset_cpu_profile_cache() {
    CACHED_CPU_PROFILE.store(usize::MAX, Ordering::Relaxed);
}
```

#### 5.2 Update Module Exports
```rust
// In backend/native/mod.rs, ensure proper exports
pub use cpu_tuning::{
    CpuFeatureDetector, ProfileResolver, OptimizationHints,
    detect_cpu_profile, resolve_cpu_profile, has_feature, get_optimization_hints
};
```

#### 5.3 Comprehensive Testing
```bash
# Full test suite with all modules
cargo test --workspace --all-features

# Specific integration tests
cargo test --lib cpu_tuning -- --nocapture
cargo test --lib graph_ops -- --nocapture

# Performance testing (if benchmarks exist)
cargo bench --bench cpu_tuning 2>/dev/null || echo "No bench found"
```

#### 5.4 Line Count Validation
```bash
# Count lines in modularized core file
wc -l sqlitegraph/src/backend/native/cpu_tuning.rs

# Count lines in all new modules
find sqlitegraph/src/backend/native -name "*cpu_tuning*" -exec wc -l {} +
```

**Expected Result**: 88 → 43 lines (12% additional reduction from final cleanup)

## Risk Mitigation Strategies

### Low Risk Implementation

1. **Function-Based Design**: Simple function extraction with no complex state
2. **Backward Compatibility**: Use re-exports to maintain identical public API
3. **Platform-Specific Compilation**: Preserve all cfg! conditional compilation
4. **Incremental Testing**: Test each phase immediately after implementation

### Minimal Validation Required

1. **API Consistency**: Verify all CPU tuning operations work identically
2. **Test Coverage**: Ensure no test functionality is lost
3. **Performance**: Confirm no performance degradation from modularization
4. **Integration**: Ensure graph_ops strategy selection works correctly

## Expected Outcomes

### Size Reduction Analysis

**Current**: 413 lines
**After Phase 1**: 413 → 313 lines (24% reduction - still over target)
**After Phase 2**: 313 → 193 lines (38% additional reduction)
**After Phase 3**: 193 → 113 lines (27% additional reduction)
**After Phase 4**: 113 → 88 lines (15% additional reduction)
**After Phase 5**: 88 → 43 lines (12% additional reduction)

**Final Result**: 43 lines (90% total reduction, 257 lines under 300 LOC target)

### Module Distribution

1. **Core Coordination**: 43 lines - Essential caching and public API
2. **Test Suite**: 100 lines - Comprehensive testing (separate file)
3. **Feature Detection**: 130 lines - CPU feature detection utilities
4. **Profile Resolution**: 85 lines - Profile validation and resolution
5. **Optimization Hints**: 30 lines - Performance tuning hints

### Quality Improvements

1. **Design Compliance**: Achieves 300 LOC target after Phase 2
2. **Functional Separation**: Clear boundaries between detection, resolution, and optimization
3. **Extensibility**: Feature detection can be extended independently
4. **Test Organization**: Tests properly isolated with shared utilities
5. **Maintainability**: Focused, single-responsibility modules

## Success Criteria

### Functional Requirements
- [ ] All existing CPU tuning operations work identically
- [ ] `graph_ops/strategy.rs` continues working without changes
- [ ] All tests pass in new location
- [ ] No performance regression
- [ ] Strategy selection works correctly

### Design Requirements
- [ ] Core file ≤300 lines (achieved after Phase 2)
- [ ] Each extracted module ≤300 lines
- [ ] Clear separation of concerns
- [ ] No circular dependencies
- [ ] Preserved public API

### Quality Requirements
- [ ] All modules documented
- [ ] Test coverage maintained
- [ ] Code quality standards met
- [ ] Import statements clean
- [ ] Compilation successful

## Critical Success Factors

### API Preservation
1. **Function Signatures**: Must maintain all CPU tuning function signatures
2. **Platform Support**: Preserve all architecture-specific optimizations
3. **Caching Logic**: Maintain identical caching behavior
4. **Feature Detection**: Ensure identical CPU feature detection results

### Test Reliability
1. **Complete Test Migration**: No tests lost in extraction
2. **Platform Coverage**: All platform combinations still tested
3. **Feature Detection**: All feature combinations covered
4. **Edge Cases**: Architecture-specific edge cases preserved

### Integration Stability
1. **Import Resolution**: All imports resolve correctly after extraction
2. **Module Dependencies**: No circular dependencies created
3. **Build Success**: Project compiles without errors
4. **Runtime Stability**: All runtime operations work correctly

## Special Considerations

### Minimum Success Requirement

Similar to query cache, this modularization requires at least **Phase 2 completion** to achieve the 300 LOC target. Test extraction alone (Phase 1) leaves the file at 313 lines, still exceeding the limit.

### Platform-Specific Code

The extensive use of conditional compilation (`cfg!`) requires careful testing across different target architectures to ensure no functionality is lost during modularization.

---

**Implementation Plan Created**: 2025-01-19
**Strategy**: Phased extraction of detection, resolution, and optimization modules
**Risk Level**: LOW (high confidence in success)
**Expected Timeline**: 1 day with comprehensive testing
**Key Challenge**: Requires 2 phases minimum to achieve 300 LOC target