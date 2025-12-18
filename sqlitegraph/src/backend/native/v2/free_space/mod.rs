//! Free space management rewritten as small modules (<300 LOC each).

mod block;
mod manager;
mod stats;

pub use block::FreeBlock;
pub use manager::{AllocationStrategy, FreeSpaceManager, MIN_BLOCK_SIZE};
pub use stats::{AllocationStats, CompactionReport, FreeSpaceAnalysis};

#[cfg(all(test, feature = "v2_experimental"))]
mod tests {
    use super::*;

    fn manager() -> FreeSpaceManager {
        FreeSpaceManager::new(AllocationStrategy::FirstFit)
    }

    #[test]
    fn test_basic_allocation() {
        let mut mgr = manager();
        mgr.add_free_block(1000, 256);
        let offset = mgr.allocate(100).unwrap();
        assert_eq!(offset, 1000);
        assert_eq!(mgr.total_free_space(), 156);
    }

    #[test]
    fn test_block_splitting() {
        let mut mgr = manager();
        mgr.add_free_block(1000, 500);
        assert_eq!(mgr.allocate(100).unwrap(), 1000);
        assert_eq!(mgr.free_blocks().len(), 1);
        assert_eq!(mgr.free_blocks()[0].offset, 1100);
        assert_eq!(mgr.free_blocks()[0].size, 400);
    }

    #[test]
    fn test_block_merging() {
        let mut mgr = manager();
        mgr.add_free_block(1000, 100);
        mgr.add_free_block(1100, 150);
        assert_eq!(mgr.free_blocks().len(), 1);
        assert_eq!(mgr.free_blocks()[0].size, 250);
    }

    #[test]
    fn test_allocation_strategies() {
        let mut mgr = FreeSpaceManager::new(AllocationStrategy::BestFit);
        mgr.add_free_block(1000, 100);
        mgr.add_free_block(2000, 200);
        mgr.add_free_block(3000, 150);
        assert_eq!(mgr.allocate(120).unwrap(), 3000);
    }

    
    #[test]
    fn test_validation() {
        let mut mgr = manager();
        assert!(mgr.validate().is_ok());
        mgr.add_free_block(1000, 200);
        mgr.add_free_block(1100, 100);
        assert!(mgr.validate().is_err());
    }
}
