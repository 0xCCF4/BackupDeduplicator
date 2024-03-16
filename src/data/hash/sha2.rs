use sha2::Digest;
use crate::data::{GeneralHash, GeneralHasher};

pub struct Sha512Hasher {
    hasher: sha2::Sha512
}
pub struct Sha256Hasher {
    hasher: sha2::Sha256
}

impl GeneralHasher for Sha512Hasher {
    fn new() -> Self {
        Sha512Hasher {
            hasher: sha2::Sha512::new()
        }
    }

    fn update(&mut self, data: &[u8]) {
        Digest::update(&mut self.hasher, data);
    }

    fn finalize(self: Box<Self>) -> GeneralHash {
        GeneralHash::SHA512(self.hasher.finalize().into())
    }
}
impl GeneralHasher for Sha256Hasher {
    fn new() -> Self {
        Sha256Hasher {
            hasher: sha2::Sha256::new()
        }
    }

    fn update(&mut self, data: &[u8]) {
        Digest::update(&mut self.hasher, data);
    }

    fn finalize(self: Box<Self>) -> GeneralHash {
        GeneralHash::SHA256(self.hasher.finalize().into())
    }
}
