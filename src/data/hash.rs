use serde::{Serialize, Serializer};

#[derive(Debug, Hash, PartialEq, Clone, Copy)]
pub enum GeneralHashType {
    SHA512,
    SHA256,
    SHA1,
    XXH64,
    XXH32,
    NULL,
}

impl GeneralHashType {
    pub fn hasher(&self) -> Box<dyn GeneralHasher> {
        match self {
            GeneralHashType::SHA512 => Box::new(sha2::Sha512Hasher::new()),
            GeneralHashType::SHA256 => Box::new(sha2::Sha256Hasher::new()),
            GeneralHashType::SHA1 => Box::new(sha1::Sha1Hasher::new()),
            GeneralHashType::XXH64 => Box::new(xxh::Xxh64Hasher::new()),
            GeneralHashType::XXH32 => Box::new(xxh::Xxh32Hasher::new()),
            GeneralHashType::NULL => Box::new(null::NullHasher::new()),
        }
    }
    
}

#[derive(Debug, Hash, PartialEq, Clone)]
pub enum GeneralHash {
    SHA512([u8; 64]),
    SHA256([u8; 32]),
    SHA1([u8; 20]),
    XXH64([u8; 8]),
    XXH32([u8; 4]),
    NULL,
}

impl Serialize for GeneralHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let capacity = match self {
            GeneralHash::SHA512(_) => 128,
            GeneralHash::SHA256(_) => 64,
            GeneralHash::SHA1(_) => 40,
            GeneralHash::XXH64(_) => 16,
            GeneralHash::XXH32(_) => 8,
            GeneralHash::NULL => 0,
        };

        let mut hex = String::with_capacity(capacity);

        match self {
            GeneralHash::SHA512(data) => for byte in data {
                hex.push_str(&format!("{:02x}", byte));
            },
            GeneralHash::SHA256(data) => for byte in data {
                hex.push_str(&format!("{:02x}", byte));
            },
            GeneralHash::SHA1(data) => for byte in data {
                hex.push_str(&format!("{:02x}", byte));
            },
            GeneralHash::XXH64(data) => for byte in data {
                hex.push_str(&format!("{:02x}", byte));
            },
            GeneralHash::XXH32(data) => for byte in data {
                hex.push_str(&format!("{:02x}", byte));
            },
            GeneralHash::NULL => {
                hex.push_str("0");
            }
        }

        serializer.serialize_str(&hex)
    }
}

impl GeneralHash {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            GeneralHash::SHA512(data) => data,
            GeneralHash::SHA256(data) => data,
            GeneralHash::SHA1(data) => data,
            GeneralHash::XXH64(data) => data,
            GeneralHash::XXH32(data) => data,
            GeneralHash::NULL => &[0; 0],
        }
    }

    pub fn new_sha512() -> Self { Self::from_type(GeneralHashType::SHA512) }
    pub fn new_sha256() -> Self { Self::from_type(GeneralHashType::SHA256) }
    pub fn new_sha1() -> Self { Self::from_type(GeneralHashType::SHA1) }
    pub fn new_xxh64() -> Self { Self::from_type(GeneralHashType::XXH64) }
    pub fn new_xxh32() -> Self { Self::from_type(GeneralHashType::XXH32) }

    pub fn hash_type(&self) -> GeneralHashType {
        match self {
            GeneralHash::SHA512(_) => GeneralHashType::SHA512,
            GeneralHash::SHA256(_) => GeneralHashType::SHA256,
            GeneralHash::SHA1(_) => GeneralHashType::SHA1,
            GeneralHash::XXH64(_) => GeneralHashType::XXH64,
            GeneralHash::XXH32(_) => GeneralHashType::XXH32,
            GeneralHash::NULL => GeneralHashType::NULL,
        }
    }
    
    pub fn from_type(hash_type: GeneralHashType) -> Self {
        match hash_type {
            GeneralHashType::SHA512 => GeneralHash::SHA512([0; 64]),
            GeneralHashType::SHA256 => GeneralHash::SHA256([0; 32]),
            GeneralHashType::SHA1 => GeneralHash::SHA1([0; 20]),
            GeneralHashType::XXH64 => GeneralHash::XXH64([0; 8]),
            GeneralHashType::XXH32 => GeneralHash::XXH32([0; 4]),
            GeneralHashType::NULL => GeneralHash::NULL,
        }
    }

    pub fn hasher(&self) -> Box<dyn GeneralHasher> {
        self.hash_type().hasher()
    }
}

pub trait GeneralHasher {
    fn new() -> Self where Self: Sized;
    fn update(&mut self, data: &[u8]);
    fn finalize(self: Box<Self>) -> GeneralHash;
}

mod sha1;
mod sha2;
mod xxh;
mod null;


pub static NULL_HASH_SHA256: GeneralHash = GeneralHash::SHA256([0; 32]);