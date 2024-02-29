use serde::{Serialize, Serializer};

#[derive(Debug, Hash, PartialEq, Clone)]
pub enum GeneralHash {
    SHA256([u8; 32]),
}

impl Serialize for GeneralHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        match self {
            GeneralHash::SHA256(data) => {
                // to hex string
                let mut hex = String::with_capacity(64);
                for byte in data {
                    hex.push_str(&format!("{:02x}", byte));
                }
                serializer.serialize_str(&hex)
            }
        }
    }
}

impl GeneralHash {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            GeneralHash::SHA256(data) => data,
        }
    }

    pub fn new_sha256() -> Self {
        GeneralHash::SHA256([0; 32])
    }
}

pub static NULL_HASH_SHA256: GeneralHash = GeneralHash::SHA256([0; 32]);
