//! Iterator trait implementation for adjacency iteration

use super::AdjacencyIterator;
use crate::backend::native::types::*;

impl<'a> Iterator for AdjacencyIterator<'a> {
    type Item = NativeNodeId;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // EVIDENCE-BASED FIX: Check completion state first to prevent infinite loops
        // When V2 cluster initialization fails, total_count becomes 0
        // This should terminate the iteration immediately
        if self.is_complete() {
            return None;
        }

        // HOT PATH: Fast neighbor lookup with proper error handling
        match self.get_current_neighbor() {
            Ok(Some(neighbor)) => {
                self.current_index += 1;
                Some(neighbor)
            }
            Ok(None) => {
                // Normal termination - no more neighbors available
                None
            }
            Err(_) => {
                // EVIDENCE-BASED FIX: Don't continue iteration on V2 initialization errors
                // When V2 cluster initialization fails, we should terminate, not continue
                // This prevents infinite loops when total_count > 0 but cluster initialization fails
                #[cfg(debug_assertions)]
                {
                    println!(
                        "DEBUG: Iterator terminating due to V2 cluster initialization error for node {}. total_count={}, current_index={}",
                        self.node_id, self.total_count, self.current_index
                    );
                }
                None
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.total_count - self.current_index) as usize;
        (remaining, Some(remaining))
    }
}
