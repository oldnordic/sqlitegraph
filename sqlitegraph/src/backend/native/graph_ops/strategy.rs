//! Strategy selection and graph size categorization for native graph backend.

use crate::backend::native::cpu_tuning::resolve_cpu_profile;
use crate::backend::native::types::CpuProfile;

/// Estimate graph size category for optimization selection
#[inline(always)]
pub fn estimate_graph_size_category(node_count: usize) -> &'static str {
    match node_count {
        0..=999 => "small",      // < 1K nodes
        1000..=9999 => "medium", // 1K-10K nodes
        _ => "large",            // >= 10K nodes
    }
}

/// Select optimal BFS strategy based on CPU profile and graph size
#[inline(always)]
pub fn select_bfs_strategy(cpu_profile: CpuProfile, node_count: usize) -> &'static str {
    let size_category = estimate_graph_size_category(node_count);
    let resolved_profile = resolve_cpu_profile(cpu_profile);

    match (resolved_profile, size_category) {
        (CpuProfile::X86Avx512, "small") => "simd512_optimized",
        (CpuProfile::X86Avx512, "medium") => "simd512_pointer_table",
        (CpuProfile::X86Zen4, "small") => "avx2_optimized",
        (CpuProfile::X86Zen4, "medium") => "avx2_pointer_table",
        (CpuProfile::X86Avx2, "small") => "avx2_optimized",
        (CpuProfile::X86Avx2, "medium") => "avx2_pointer_table",
        _ => "generic_scalar",
    }
}
