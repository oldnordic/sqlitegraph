//! Memory mapping operations and management for GraphFile
//!
//! This module provides memory-mapped file operations including initialization,
//! resizing, bounds checking, and safe read/write operations with recursion protection.

use crate::backend::native::{
    types::NativeResult,
    types::NativeBackendError,
};

/// Memory mapping management utilities for GraphFile
pub struct MMapManager;

impl MMapManager {
    /// Get memory mapping statistics for debugging
    pub fn get_mmap_statistics(mmap_size: u64, is_initialized: bool) -> MMapStatistics {
        MMapStatistics {
            is_initialized,
            mmap_size,
            is_mmap_active: is_initialized,
        }
    }

    /// Check if memory mapping is available and configured
    #[cfg(feature = "v2_experimental")]
    pub fn is_mmap_available() -> bool {
        true
    }

    /// Check if memory mapping is disabled
    #[cfg(not(feature = "v2_experimental"))]
    pub fn is_mmap_available() -> bool {
        false
    }

    /// Validate memory map bounds for reading
    #[cfg(feature = "v2_experimental")]
    pub fn validate_read_bounds(
        mmap_size: usize,
        offset: u64,
        buffer_len: usize,
    ) -> NativeResult<()> {
        if offset as usize + buffer_len > mmap_size {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Read beyond mmap region: offset={}, len={}, mmap_size={}",
                    offset,
                    buffer_len,
                    mmap_size
                ),
            });
        }
        Ok(())
    }

    /// Validate memory map bounds for writing
    #[cfg(feature = "v2_experimental")]
    pub fn validate_write_bounds(
        mmap_size: usize,
        offset: u64,
        data_len: usize,
    ) -> NativeResult<()> {
        if offset as usize + data_len > mmap_size {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Write beyond mmap region: offset={}, len={}, mmap_size={}",
                    offset,
                    data_len,
                    mmap_size
                ),
            });
        }
        Ok(())
    }

    /// Check for recursion depth in mmap operations
    pub fn check_recursion_depth(current_depth: u32, max_depth: u32) -> NativeResult<()> {
        if current_depth > max_depth {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!("mmap recursion depth exceeded: {}", current_depth),
            });
        }
        Ok(())
    }
}

/// Memory mapping statistics for debugging and monitoring
#[derive(Debug, Clone)]
pub struct MMapStatistics {
    pub is_initialized: bool,
    pub mmap_size: u64,
    pub is_mmap_active: bool,
}

impl MMapStatistics {
    /// Check if mmap is properly initialized
    pub fn is_valid(&self) -> bool {
        self.is_initialized && self.is_mmap_active && self.mmap_size > 0
    }

    /// Get memory mapping size in bytes
    pub fn get_size_bytes(&self) -> u64 {
        self.mmap_size
    }

    /// Get memory mapping size in kilobytes
    pub fn get_size_kb(&self) -> f64 {
        self.mmap_size as f64 / 1024.0
    }

    /// Get memory mapping size in megabytes
    pub fn get_size_mb(&self) -> f64 {
        self.mmap_size as f64 / (1024.0 * 1024.0)
    }
}

/// Memory mapping configuration options
#[derive(Debug, Clone)]
pub struct MMapConfig {
    pub enable_mmap: bool,
    pub growth_threshold_kb: u64,
    pub max_recursion_depth: u32,
}

impl Default for MMapConfig {
    fn default() -> Self {
        Self {
            enable_mmap: cfg!(feature = "v2_experimental"),
            growth_threshold_kb: 1024, // 1MB
            max_recursion_depth: 10,
        }
    }
}

impl MMapConfig {
    /// Create new mmap configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable memory mapping
    pub fn disabled() -> Self {
        Self {
            enable_mmap: false,
            ..Default::default()
        }
    }

    /// Set custom growth threshold
    pub fn with_growth_threshold(mut self, threshold_kb: u64) -> Self {
        self.growth_threshold_kb = threshold_kb;
        self
    }

    /// Set custom recursion depth limit
    pub fn with_max_recursion_depth(mut self, depth: u32) -> Self {
        self.max_recursion_depth = depth;
        self
    }

    /// Check if mmap should be enabled
    pub fn should_enable_mmap(&self) -> bool {
        self.enable_mmap && MMapManager::is_mmap_available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmap_config() {
        let config = MMapConfig::new();

        // Check based on feature flag
        let should_enable = cfg!(feature = "v2_experimental");
        assert_eq!(config.enable_mmap, should_enable,
            "MMapConfig.enable_mmap should match v2_experimental feature flag state (enabled={})", should_enable);

        assert_eq!(config.growth_threshold_kb, 1024);
        assert_eq!(config.max_recursion_depth, 10);
    }

    #[test]
    fn test_mmap_config_disabled() {
        let config = MMapConfig::disabled();
        assert!(!config.enable_mmap);
    }

    #[test]
    fn test_mmap_config_builder() {
        let config = MMapConfig::new()
            .with_growth_threshold(2048)
            .with_max_recursion_depth(20);

        assert_eq!(config.growth_threshold_kb, 2048);
        assert_eq!(config.max_recursion_depth, 20);
    }

    #[test]
    fn test_mmap_statistics() {
        let stats = MMapStatistics {
            is_initialized: true,
            mmap_size: 1024 * 1024, // 1MB
            is_mmap_active: true,
        };

        assert!(stats.is_valid());
        assert_eq!(stats.get_size_bytes(), 1024 * 1024);
        assert_eq!(stats.get_size_kb(), 1024.0);
        assert_eq!(stats.get_size_mb(), 1.0);
    }

    #[test]
    fn test_mmap_statistics_uninitialized() {
        let stats = MMapStatistics {
            is_initialized: false,
            mmap_size: 0,
            is_mmap_active: false,
        };

        assert!(!stats.is_valid());
        assert_eq!(stats.get_size_bytes(), 0);
        assert_eq!(stats.get_size_kb(), 0.0);
        assert_eq!(stats.get_size_mb(), 0.0);
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_validate_read_bounds_success() {
        let result = MMapManager::validate_read_bounds(1024, 100, 100);
        assert!(result.is_ok());
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_validate_read_bounds_failure() {
        let result = MMapManager::validate_read_bounds(1024, 900, 200); // 900 + 200 > 1024
        assert!(result.is_err());
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_validate_write_bounds_success() {
        let result = MMapManager::validate_write_bounds(1024, 100, 100);
        assert!(result.is_ok());
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_validate_write_bounds_failure() {
        let result = MMapManager::validate_write_bounds(1024, 900, 200); // 900 + 200 > 1024
        assert!(result.is_err());
    }

    #[test]
    fn test_recursion_depth_check_success() {
        let result = MMapManager::check_recursion_depth(5, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_recursion_depth_check_failure() {
        let result = MMapManager::check_recursion_depth(15, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_mmap_manager_availability() {
        let is_available = MMapManager::is_mmap_available();
        // This test will pass regardless of feature flags
        // since the function handles the feature gate internally
        assert!(is_available == cfg!(feature = "v2_experimental"));
    }
}