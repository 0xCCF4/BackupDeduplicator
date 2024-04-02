use xxhash_rust::{xxh32, xxh64};
use crate::hash::{GeneralHash, GeneralHasher};

pub struct Xxh64Hasher {
    hasher: xxh64::Xxh64
}
pub struct Xxh32Hasher {
    hasher: xxh32::Xxh32
}


impl GeneralHasher for Xxh64Hasher {
    fn new() -> Self {
        Xxh64Hasher {
            hasher: xxh64::Xxh64::default()
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> GeneralHash {
        GeneralHash::XXH64(self.hasher.digest().to_be_bytes())
    }
}
impl GeneralHasher for Xxh32Hasher {
    fn new() -> Self {
        Xxh32Hasher {
            hasher: xxh32::Xxh32::new(0)
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> GeneralHash {
        GeneralHash::XXH32(self.hasher.digest().to_be_bytes())
    }
}
