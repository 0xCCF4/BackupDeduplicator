use sha1::Digest;
use crate::data::{GeneralHash, GeneralHasher};

pub struct Sha1Hasher {
    hasher: sha1::Sha1
}

impl GeneralHasher for Sha1Hasher {
    fn new() -> Self {
        Sha1Hasher {
            hasher: sha1::Sha1::new()
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> GeneralHash {
        GeneralHash::SHA1(self.hasher.finalize().into())
    }
}
