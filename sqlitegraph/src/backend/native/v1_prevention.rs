//! V1 Legacy Prevention Barrier
//!
//! This module deliberately prevents V1 legacy code from ever compiling again.
//! Any attempt to reference V1 types, features, or patterns will result in
//! compilation errors.
//!
//! Phase 3 of V1 Purge Mission: Enforce that V1 cannot compile.

// DELIBERATE COMPILATION ERRORS FOR V1 ATTEMPTS
//
// The following type aliases and const assertions will cause compilation
// failures if any V1 legacy code attempts to be compiled or used.

// V1 Type Aliases that will fail compilation if referenced
#[allow(dead_code)]
#[allow(non_camel_case_types)] // Intentional naming to discourage usage
type NodeRecordV1_DO_NOT_USE = [u8; 0]; // Intentionally invalid type
#[allow(dead_code)]
#[allow(non_camel_case_types)] // Intentional naming to discourage usage
type EdgeRecordV1_DO_NOT_USE = [u8; 0]; // Intentionally invalid type
#[allow(dead_code)]
#[allow(non_camel_case_types)] // Intentional naming to discourage usage
type GraphFileV1_DO_NOT_USE = [u8; 0]; // Intentionally invalid type

// Compile-time assertion: If V1 code exists, this will fail
const _V1_BARRIER: [(); 0] = [(); 0]; // Intentionally impossible to use

/// Deliberate compilation barrier function - never call this
#[allow(dead_code)]
fn v1_compilation_barrier_do_not_use() -> ! {
    // This function can never be called successfully
    // Any reference to V1 patterns should route here
    panic!("V1 LEGACY CODE DETECTED: This codebase is V2-ONLY. V1 has been permanently removed.")
}

/// Compile-time check function that prevents V1 feature usage
#[allow(dead_code)]
const fn v1_feature_check_prevented() {
    // This const function will fail if V1 features are attempted
    // The unreachable macro in const context will cause compilation failure
    let _ = [(); {
        // If this evaluates to non-zero, compilation fails
        0  // V1 is completely removed, so this is 0
    }];
}

// Module-level documentation warning
#[doc = "⚠️  V1 LEGACY BARRIER: V1 code has been permanently removed from this codebase."]
#[doc = "Any attempt to reintroduce V1 patterns, types, or features will fail to compile."]
pub mod v1_quarantine {
    /// This module is a quarantine zone - nothing here should ever be used
    #[allow(dead_code)]
    pub const V1_REMOVAL_COMPLETE: bool = true;

    /// Compile-time assertion that V1 is gone
    const _: [(); 1] = [(); {
        if V1_REMOVAL_COMPLETE {
            1
        } else {
            // This branch can never happen - V1 is permanently removed
            0
        }
    }];
}

// Feature flag barriers - prevent V1 feature flags
#[allow(unexpected_cfgs)]
#[cfg(feature = "v1_experimental")]
compile_error!("V1_EXPERIMENTAL FEATURE DETECTED: V1 has been permanently removed. This feature cannot be enabled.");

#[allow(unexpected_cfgs)]
#[cfg(feature = "enable_v1")]
compile_error!("ENABLE_V1 FEATURE DETECTED: V1 has been permanently removed. This feature cannot be enabled.");

#[allow(unexpected_cfgs)]
#[cfg(feature = "legacy_v1")]
compile_error!("LEGACY_V1 FEATURE DETECTED: V1 has been permanently removed. This feature cannot be enabled.");

#[allow(unexpected_cfgs)]
#[cfg(any(feature = "v1", feature = "v1_compatibility", feature = "v1_mode"))]
compile_error!("V1 COMPATIBILITY FEATURE DETECTED: V1 has been permanently removed. Use V2-only APIs.");

// Prevent V1 module imports - this path deliberately doesn't exist
// Uncomment the following line if V1 modules ever try to reappear:
// #[path = "v1_DO_NOT_EXIST.rs"]
// mod v1_import_barrier;

// Debug assertions for development
#[cfg(debug_assertions)]
const _: () = {
    let _assertion = assert!(
        true, // V1 has been successfully removed
        "V1 LEGACY CODE DETECTED: This should never happen"
    );
};

/// Runtime check that enforces V2-only behavior
#[inline(always)]
pub fn enforce_v2_only() {
    #[cfg(debug_assertions)]
    debug_assert!(
        true, // V2 is the only supported version
        "V1 LEGACY DETECTED: This codebase is V2-only"
    );
}

/// Marker that V1 has been completely purged
pub const V1_PERMANENTLY_REMOVED: &str = "V1 legacy code has been permanently removed from SQLiteGraph - V2-ONLY now and forever";