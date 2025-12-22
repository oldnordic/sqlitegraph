use super::block::FreeBlock;
use super::stats::{AllocationStats, CompactionReport, FreeSpaceAnalysis};
use crate::backend::native::{NativeBackendError, NativeResult};

pub const MIN_BLOCK_SIZE: u32 = 32;
const MAX_FRAGMENTATION_RATIO: f64 = 0.3;

#[derive(Debug, Clone, Copy)]
pub enum AllocationStrategy {
    FirstFit,
    BestFit,
    WorstFit,
}

#[derive(Debug, Clone)]
pub struct FreeSpaceManager {
    free_blocks: Vec<FreeBlock>,
    strategy: AllocationStrategy,
    stats: AllocationStats,
}

impl FreeSpaceManager {
    pub fn new(strategy: AllocationStrategy) -> Self {
        Self {
            free_blocks: Vec::new(),
            strategy,
            stats: AllocationStats::default(),
        }
    }

    pub fn add_free_block(&mut self, offset: u64, size: u32) {
        if size < MIN_BLOCK_SIZE {
            return;
        }
        self.free_blocks.push(FreeBlock::new(offset, size));
        self.stats.total_deallocations += 1;
        self.stats.total_deallocated_bytes += size as u64;
        self.try_merge_adjacent_blocks();
        self.update_fragmentation_ratio();
    }

    pub fn allocate(&mut self, requested_size: u32) -> NativeResult<u64> {
        if requested_size == 0 {
            return Ok(0);
        }

        let index = self.find_suitable_block(requested_size)?;
        let mut block = self.free_blocks.remove(index);
        let allocated_offset = block.offset;

        if let Some(remaining) = block.split_if_needed(requested_size) {
            self.free_blocks.push(remaining);
            self.stats.block_splits += 1;
        }

        self.stats.total_allocations += 1;
        self.stats.total_allocated_bytes += requested_size as u64;
        self.update_fragmentation_ratio();
        Ok(allocated_offset)
    }

    fn find_suitable_block(&self, requested_size: u32) -> NativeResult<usize> {
        let candidates: Vec<usize> = self
            .free_blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| block.can_accommodate(requested_size))
            .map(|(index, _)| index)
            .collect();

        if candidates.is_empty() {
            return Err(NativeBackendError::OutOfSpace);
        }

        let selected = match self.strategy {
            AllocationStrategy::FirstFit => candidates[0],
            AllocationStrategy::BestFit => candidates
                .iter()
                .min_by_key(|&&i| self.free_blocks[i].size)
                .copied()
                .ok_or_else(|| NativeBackendError::CorruptFreeSpace {
                    reason: "BestFit strategy failed to find minimum block in non-empty candidates".to_string(),
                })?,
            AllocationStrategy::WorstFit => candidates
                .iter()
                .max_by_key(|&&i| self.free_blocks[i].size)
                .copied()
                .ok_or_else(|| NativeBackendError::CorruptFreeSpace {
                    reason: "WorstFit strategy failed to find maximum block in non-empty candidates".to_string(),
                })?,
        };
        Ok(selected)
    }

    fn try_merge_adjacent_blocks(&mut self) {
        if self.free_blocks.len() < 2 {
            return;
        }

        self.free_blocks.sort_by_key(|block| block.offset);
        let mut merged = Vec::new();
        let mut current = self.free_blocks[0].clone();

        for next in self.free_blocks[1..].iter() {
            if current.can_merge_with(next) {
                current.merge_with(next);
                self.stats.block_merges += 1;
            } else {
                merged.push(current);
                current = next.clone();
            }
        }

        merged.push(current);
        self.free_blocks = merged;
    }

    fn update_fragmentation_ratio(&mut self) {
        if self.free_blocks.is_empty() {
            self.stats.fragmentation_ratio = 0.0;
            return;
        }

        let total_free: u64 = self.free_blocks.iter().map(|b| b.size as u64).sum();
        let largest: u64 = self
            .free_blocks
            .iter()
            .map(|b| b.size as u64)
            .max()
            .unwrap_or(0);

        self.stats.fragmentation_ratio = if total_free > 0 {
            1.0 - (largest as f64 / total_free as f64)
        } else {
            0.0
        };
    }

    pub fn needs_compaction(&self) -> bool {
        self.stats.fragmentation_ratio > MAX_FRAGMENTATION_RATIO && self.free_blocks.len() > 10
    }

    pub fn compact(&mut self) -> CompactionReport {
        let start_fragments = self.free_blocks.len();
        let start_ratio = self.stats.fragmentation_ratio;
        self.try_merge_adjacent_blocks();

        let before = self.free_blocks.len();
        self.free_blocks
            .retain(|block| block.size >= MIN_BLOCK_SIZE);
        let removed_small = before - self.free_blocks.len();
        self.update_fragmentation_ratio();

        CompactionReport {
            initial_fragments: start_fragments,
            final_fragments: self.free_blocks.len(),
            initial_fragmentation_ratio: start_ratio,
            final_fragmentation_ratio: self.stats.fragmentation_ratio,
            small_fragments_removed: removed_small,
            blocks_merged: self.stats.block_merges,
        }
    }

    pub fn stats(&self) -> &AllocationStats {
        &self.stats
    }

    pub fn free_blocks(&self) -> &[FreeBlock] {
        &self.free_blocks
    }

    pub fn total_free_space(&self) -> u64 {
        self.free_blocks.iter().map(|block| block.size as u64).sum()
    }

    pub fn largest_free_block(&self) -> Option<u32> {
        self.free_blocks.iter().map(|block| block.size).max()
    }

    pub fn validate(&self) -> NativeResult<()> {
        let mut sorted = self.free_blocks.clone();
        sorted.sort_by_key(|block| block.offset);

        for i in 0..sorted.len().saturating_sub(1) {
            let current = &sorted[i];
            let next = &sorted[i + 1];
            if current.offset + current.size as u64 > next.offset {
                return Err(NativeBackendError::CorruptFreeSpace {
                    reason: format!(
                        "Overlapping free blocks: {}-{} and {}-{}",
                        current.offset,
                        current.offset + current.size as u64,
                        next.offset,
                        next.offset + next.size as u64
                    ),
                });
            }
        }

        if !(0.0..=1.0).contains(&self.stats.fragmentation_ratio) {
            return Err(NativeBackendError::CorruptFreeSpace {
                reason: format!(
                    "Invalid fragmentation ratio: {}",
                    self.stats.fragmentation_ratio
                ),
            });
        }

        Ok(())
    }

    pub fn reset(&mut self) {
        self.free_blocks.clear();
        self.stats = AllocationStats::default();
    }

    pub fn export_analysis(&self) -> FreeSpaceAnalysis {
        let histogram = self.create_size_histogram();
        FreeSpaceAnalysis {
            total_blocks: self.free_blocks.len(),
            total_free_bytes: self.total_free_space(),
            largest_block: self.largest_free_block().unwrap_or(0),
            average_block_size: if !self.free_blocks.is_empty() {
                self.total_free_space() / self.free_blocks.len() as u64
            } else {
                0
            },
            fragmentation_ratio: self.stats.fragmentation_ratio,
            blocks_by_size: self
                .free_blocks
                .iter()
                .map(|block| (block.size, block.offset))
                .collect(),
            size_histogram: histogram,
        }
    }

    fn create_size_histogram(&self) -> Vec<(u32, usize)> {
        const SIZE_RANGES: [(u32, u32); 6] = [
            (32, 64),
            (64, 256),
            (256, 1024),
            (1024, 4096),
            (4096, 16384),
            (16384, u32::MAX),
        ];

        let mut histogram = vec![(0, 0); SIZE_RANGES.len()];
        for block in &self.free_blocks {
            for (i, &(min, max)) in SIZE_RANGES.iter().enumerate() {
                if block.size >= min && block.size <= max {
                    histogram[i].1 += 1;
                    break;
                }
            }
        }
        histogram
    }
}
