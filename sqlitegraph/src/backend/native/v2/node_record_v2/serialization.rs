//! Binary serialization implementation for V2 node records

use super::NodeRecordV2;

impl NodeRecordV2 {
    /// Serialize the node record to binary format
    pub fn serialize(&self) -> Vec<u8> {
        let data_bytes = serde_json::to_vec(&self.data).unwrap_or_default();
        let mut buffer =
            Vec::with_capacity(21 + self.kind.len() + self.name.len() + data_bytes.len() + 32);
        buffer.push(2); // version

        // Write flags (4 bytes) - ensure exactly 4 bytes
        let flags_bytes = self.flags.0.to_be_bytes();
        buffer.extend_from_slice(&flags_bytes);

        // Write node ID at correct position (bytes 5-12, immediately after flags)
        let id_bytes = self.id.to_be_bytes();
        buffer.extend_from_slice(&id_bytes);

        let kind_bytes = self.kind.as_bytes();
        let name_bytes = self.name.as_bytes();
        buffer.extend_from_slice(&(kind_bytes.len() as u16).to_be_bytes());
        buffer.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        buffer.extend_from_slice(&(data_bytes.len() as u32).to_be_bytes());

        buffer.extend_from_slice(kind_bytes);
        buffer.extend_from_slice(name_bytes);
        buffer.extend_from_slice(&data_bytes);

        buffer.extend_from_slice(&self.outgoing_cluster_offset.to_be_bytes());
        buffer.extend_from_slice(&self.outgoing_cluster_size.to_be_bytes());
        buffer.extend_from_slice(&self.outgoing_edge_count.to_be_bytes());
        buffer.extend_from_slice(&self.incoming_cluster_offset.to_be_bytes());
        buffer.extend_from_slice(&self.incoming_cluster_size.to_be_bytes());
        buffer.extend_from_slice(&self.incoming_edge_count.to_be_bytes());
        buffer
    }
}