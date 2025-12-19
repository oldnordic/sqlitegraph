//! Trace and debugging infrastructure for edge cluster operations.
//!
//! This module provides thread-local trace context management and strict mode
//! validation for cluster operations. It isolates debugging concerns from
//! the core cluster logic.

use crate::backend::native::{FileOffset};
use std::cell::{Cell, RefCell};
use std::fmt::Write;

/// Adjacency direction for cluster construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Outgoing,
    Incoming,
}

#[derive(Clone, Copy, Debug)]
pub struct TraceContext {
    pub node_id: i64,
    pub direction: Direction,
    pub cluster_offset: FileOffset,
    pub payload_size: u32,
    pub strict: bool,
}

pub struct TraceGuard {
    strict_guard: StrictModeGuard,
}

pub struct StrictModeGuard {
    previous: bool,
}

thread_local! {
    static TRACE_CONTEXT: RefCell<Option<TraceContext>> = RefCell::new(None);
    static STRICT_MODE: Cell<bool> = Cell::new(false);
}

impl TraceGuard {
    pub fn new(context: TraceContext) -> Self {
        TRACE_CONTEXT.with(|slot| {
            *slot.borrow_mut() = Some(context);
        });
        let strict_guard = StrictModeGuard::new(context.strict);
        TraceGuard { strict_guard }
    }
}

impl Drop for TraceGuard {
    fn drop(&mut self) {
        TRACE_CONTEXT.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

impl StrictModeGuard {
    pub fn new(strict: bool) -> Self {
        let previous = STRICT_MODE.with(|cell| {
            let prev = cell.get();
            cell.set(strict);
            prev
        });
        StrictModeGuard { previous }
    }
}

impl Drop for StrictModeGuard {
    fn drop(&mut self) {
        STRICT_MODE.with(|cell| {
            cell.set(self.previous);
        });
    }
}

/// Check if strict mode is currently enabled for trace validation.
pub fn strict_mode_enabled() -> bool {
    STRICT_MODE.with(|cell| cell.get())
}

/// Execute a function with the current trace context if available.
pub fn with_trace_context<F: FnOnce(&TraceContext)>(f: F) {
    TRACE_CONTEXT.with(|slot| {
        if let Some(ctx) = *slot.borrow() {
            f(&ctx);
        }
    });
}

/// Get the current trace context if available.
pub fn current_trace_context() -> Option<TraceContext> {
    TRACE_CONTEXT.with(|slot| *slot.borrow())
}

/// Format a detailed reason string for strict mode violations.
pub fn format_strict_reason(
    ctx: Option<TraceContext>,
    detail: &str,
    edge_index: usize,
    cursor: usize,
    payload_size: usize,
    remaining: usize,
    preview: &[u8],
) -> String {
    let mut preview_hex = String::new();
    for (i, byte) in preview.iter().enumerate() {
        if i > 0 {
            preview_hex.push(' ');
        }
        let _ = write!(&mut preview_hex, "{:02X}", byte);
    }
    let preview_ascii = String::from_utf8_lossy(preview);

    if let Some(ctx) = ctx {
        format!(
            "{} [node_id={}, direction={:?}, cluster_offset={}, payload_size={}, edge_index={}, cursor={}, remaining={}, preview_hex={}, preview_ascii={:?}]",
            detail,
            ctx.node_id,
            ctx.direction,
            ctx.cluster_offset,
            payload_size,
            edge_index,
            cursor,
            remaining,
            preview_hex,
            preview_ascii
        )
    } else {
        format!(
            "{} [payload_size={}, edge_index={}, cursor={}, remaining={}, preview_hex={}, preview_ascii={:?}]",
            detail, payload_size, edge_index, cursor, remaining, preview_hex, preview_ascii
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_equality() {
        assert_eq!(Direction::Outgoing, Direction::Outgoing);
        assert_eq!(Direction::Incoming, Direction::Incoming);
        assert_ne!(Direction::Outgoing, Direction::Incoming);
    }

    #[test]
    fn test_trace_context_creation() {
        let ctx = TraceContext {
            node_id: 42,
            direction: Direction::Outgoing,
            cluster_offset: 1000,
            payload_size: 500,
            strict: true,
        };
        assert_eq!(ctx.node_id, 42);
        assert_eq!(ctx.direction, Direction::Outgoing);
        assert!(ctx.strict);
    }

    #[test]
    fn test_strict_mode_guard() {
        // Test that strict mode guard preserves previous state
        let initial_state = strict_mode_enabled();
        {
            let _guard = StrictModeGuard::new(true);
            assert!(strict_mode_enabled());
        }
        // Should return to original state after guard drops
        assert_eq!(strict_mode_enabled(), initial_state);
    }

    #[test]
    fn test_trace_guard() {
        let ctx = TraceContext {
            node_id: 123,
            direction: Direction::Incoming,
            cluster_offset: 2000,
            payload_size: 300,
            strict: false,
        };

        {
            let _guard = TraceGuard::new(ctx);
            // Test that trace context is available
            let current_ctx = current_trace_context();
            assert!(current_ctx.is_some());
            assert_eq!(current_ctx.unwrap().node_id, 123);
        }

        // Trace context should be cleared after guard drops
        assert!(current_trace_context().is_none());
    }

    #[test]
    fn test_with_trace_context() {
        let ctx = TraceContext {
            node_id: 999,
            direction: Direction::Outgoing,
            cluster_offset: 5000,
            payload_size: 100,
            strict: true,
        };

        {
            let _guard = TraceGuard::new(ctx);
            let mut called = false;
            with_trace_context(|trace_ctx| {
                called = true;
                assert_eq!(trace_ctx.node_id, 999);
                assert_eq!(trace_ctx.direction, Direction::Outgoing);
            });
            assert!(called);
        }

        // Should not call function when no trace context
        let mut called = false;
        with_trace_context(|_trace_ctx| {
            called = true;
        });
        assert!(!called);
    }

    #[test]
    fn test_format_strict_reason_with_context() {
        let ctx = TraceContext {
            node_id: 42,
            direction: Direction::Outgoing,
            cluster_offset: 1000,
            payload_size: 200,
            strict: true,
        };

        let reason = format_strict_reason(
            Some(ctx),
            "Test error",
            5,
            100,
            200,
            50,
            b"\x01\x02\x03",
        );

        assert!(reason.contains("Test error"));
        assert!(reason.contains("node_id=42"));
        assert!(reason.contains("direction=Outgoing"));
        assert!(reason.contains("cluster_offset=1000"));
        assert!(reason.contains("edge_index=5"));
        assert!(reason.contains("01 02 03"));
    }

    #[test]
    fn test_format_strict_reason_without_context() {
        let reason = format_strict_reason(
            None,
            "Test error",
            3,
            50,
            150,
            75,
            b"\xFF\xEE",
        );

        assert!(reason.contains("Test error"));
        assert!(reason.contains("payload_size=150"));
        assert!(reason.contains("edge_index=3"));
        assert!(reason.contains("FF EE"));
        assert!(!reason.contains("node_id="));
    }
}