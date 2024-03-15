use serde::{Serialize, Serializer};

#[derive(Debug, Hash, PartialEq, Clone, Copy)]
pub enum GeneralHashType {
    SHA256,
    // SHA1,
}

#[derive(Debug, Hash, PartialEq, Clone)]
pub enum GeneralHash {
    SHA256([u8; 32]),
    //SHA1([u8; 20]),
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
            /* GeneralHash::SHA1(data) => {
                // to hex string
                let mut hex = String::with_capacity(40);
                for byte in data {
                    hex.push_str(&format!("{:02x}", byte));
                }
                serializer.serialize_str(&hex)
            } */
        }
    }
}

impl GeneralHash {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            GeneralHash::SHA256(data) => data,
            // GeneralHash::SHA1(data) => data,
        }
    }

    pub fn new_sha256() -> Self {
        GeneralHash::SHA256([0; 32])
    }
    
    // pub fn new_sha1() -> Self { GeneralHash::SHA1([0; 20]) }

    pub fn hash_type(&self) -> GeneralHashType {
        match self {
            GeneralHash::SHA256(_) => GeneralHashType::SHA256,
            //GeneralHash::SHA1(_) => GeneralHashType::SHA1,
        }
    }
    
    pub fn from_type(hash_type: GeneralHashType) -> Self {
        match hash_type {
            GeneralHashType::SHA256 => GeneralHash::new_sha256(),
            //GeneralHashType::SHA1 => GeneralHash::new_sha1(),
        }
    }
}

pub static NULL_HASH_SHA256: GeneralHash = GeneralHash::SHA256([0; 32]);
// pub static NULL_HASH_SHA1: GeneralHash = GeneralHash::SHA1([0; 20]);
