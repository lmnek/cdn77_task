mod lc_trie;

use lc_trie::LCTrie;
use std::net::Ipv6Addr;

#[derive(Debug, Clone)]
pub struct Ipv6Net {
    pub ip: Ipv6Addr,
    pub prefix_len: usize,
}

struct Data {
    lc_trie: LCTrie,
}

impl Data {
    pub fn load_data() -> Data {
        todo!();
    }

    //func (d *Data) Route(ecs *net.IPNet) (pop uint16, scope int)
    pub fn route(&self, ecs: &Ipv6Net) -> Option<(u16, usize)> {
        self.lc_trie.lookup(ecs.ip.to_bits(), ecs.prefix_len)
    }
}

fn main() {
    let data = Data::load_data();

    let ecs = Ipv6Net {
        ip: "2001:49f0:d0b8:8a00::1"
            .parse()
            .expect("Invalid IPv6 address"),
        prefix_len: 56,
    };

    match data.route(&ecs) {
        Some((pop_id, scope_prefix)) => {
            println!("Pop ID: {pop_id}, scope-prefix: {scope_prefix}");
        }
        None => println!("No matching route found"),
    }
}
