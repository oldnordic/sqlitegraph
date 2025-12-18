//! Shared string table split into focused modules to satisfy the ≤300 LOC rule.

mod metrics;
mod serialization;
mod table;

pub use metrics::CompressionMetrics;
pub use table::StringTable;

#[cfg(all(test, feature = "v2_experimental"))]
mod tests {
    use super::*;

    #[test]
    fn test_string_table_basic_operations() {
        let mut table = StringTable::new();
        let offset1 = table.get_or_add_offset("calls").unwrap();
        let offset2 = table.get_or_add_offset("imports").unwrap();
        assert_ne!(offset1, offset2);
        assert_eq!(offset1, table.get_or_add_offset("calls").unwrap());
        assert_eq!(table.get_string(offset1).unwrap(), "calls");
        assert_eq!(table.get_string(offset2).unwrap(), "imports");
    }

    #[test]
    fn test_common_edge_type_prepopulation() {
        let mut table = StringTable::new();
        let calls_offset = table.get_or_add_offset("calls").unwrap();
        let imports_offset = table.get_or_add_offset("imports").unwrap();
        assert!(calls_offset < 10);
        assert!(imports_offset < 10);
        assert_ne!(calls_offset, imports_offset);
    }

    #[test]
    fn test_string_table_serialization() {
        let mut table = StringTable::new();
        table.get_or_add_offset("calls").unwrap();
        table.get_or_add_offset("custom_edge").unwrap();
        table.get_or_add_offset("references").unwrap();

        let serialized = table.serialize();
        let deserialized = StringTable::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.get_string(0).unwrap(), "calls");
        assert_eq!(deserialized.len(), table.len());
    }

    #[test]
    fn test_compression_efficiency() {
        let mut table = StringTable::new();
        for _ in 0..100 {
            table.get_or_add_offset("calls").unwrap();
            table.get_or_add_offset("imports").unwrap();
        }
        for i in 0..50 {
            table
                .get_or_add_offset(&format!("edge_type_{}", i))
                .unwrap();
        }

        let metrics = table.compression_metrics();
        assert!(metrics.compression_ratio > 1.0);
        assert!(metrics.total_unique_strings < 200);
    }

    #[test]
    fn test_string_table_validation() {
        let mut table = StringTable::new();
        table.get_or_add_offset("test").unwrap();
        table.get_or_add_offset("edge").unwrap();
        assert!(table.validate().is_ok());
    }

    #[test]
    fn test_edge_type_offset_limits() {
        let mut table = StringTable::new();
        for i in 0..1000 {
            let offset = table
                .get_or_add_offset(&format!("edge_type_{}", i))
                .unwrap();
            if i < u16::MAX as usize {
                assert!(offset <= u16::MAX);
            }
        }
    }
}
