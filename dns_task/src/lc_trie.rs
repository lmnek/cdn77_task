// NOTE: type aliases for better readability
// -> code would benefit from introducing generics
type Value = (u16, usize); // (pop ID, prefix-length)
type Key = u128;

#[derive(Debug, Clone)]
struct Node {
    skip: usize,          // path compression
    branch: usize,        // level compression
    adr: usize,           // index of left-most child in prefix table
    value: Option<Value>, // None for internal nodes
}

#[derive(Debug)]
pub struct LCTrie {
    nodes: Vec<Node>, // prefix table
}

impl LCTrie {
    // For key + its prefix length, find the most specific match
    // and return corresponding value + prefix length
    pub fn lookup(&self, prefix_key: Key, prefix_len: usize) -> Option<Value> {
        if self.nodes.is_empty() {
            return None;
        }

        // Start from the root node
        let mut node = &self.nodes[0];
        let mut pos = node.skip;
        let mut adr = node.adr;

        // Save the best matching value and its prefix len
        let mut best_match: Option<Value> = node.value;

        // Search through the tree
        while node.branch > 0 && pos < prefix_len {
            let level_idx = extract_bits_u128(prefix_key, pos, node.branch) as usize;

            match self.nodes.get(adr + level_idx) {
                Some(next_node) => {
                    node = next_node;
                    pos += node.branch + node.skip;
                    adr = node.adr;

                    // Update best match if this node has a value
                    if let Some(_) = node.value {
                        best_match = node.value;
                    }
                }
                None => break, // Prevent out-of-bounds access
            }
        }

        best_match
    }

    // other functions would be here... -> new, insert, delete
}

// NOTE: add underflow/overflow check

fn extract_bits_u128(number: u128, pos: usize, branch: usize) -> u128 {
    let mask = (1u128 << branch) - 1;
    (number >> (128 - pos - branch)) & mask
}

#[allow(dead_code)]
fn extract_bits_u8(number: u8, pos: usize, branch: usize) -> u8 {
    let mask = (1u8 << branch) - 1;
    (number >> (8 - pos - branch)) & mask
}

#[cfg(test)]
mod tests {
    use crate::lc_trie::extract_bits_u8;

    #[test]
    fn test_extract_bits() {
        assert_eq!(extract_bits_u8(0b01110111, 1, 5), 0b00011101);
        assert_eq!(extract_bits_u8(0b01110111, 3, 3), 0b00000101);
        assert_eq!(extract_bits_u8(0b00010000, 3, 1), 0b00000001);
        assert_eq!(extract_bits_u8(0b00000000, 3, 1), 0b00000000);
    }
}
