use std::fmt;
use std::fmt::Display;
use std::str::FromStr;
use serde::{Deserialize, Serialize, Serializer};
use serde::de::Error;
use crate::utils;

#[derive(Debug, Hash, PartialEq, Clone, Copy, Serialize, Deserialize)]
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

impl FromStr for GeneralHashType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SHA512" => Ok(GeneralHashType::SHA512),
            "SHA256" => Ok(GeneralHashType::SHA256),
            "SHA1" => Ok(GeneralHashType::SHA1),
            "XXH64" => Ok(GeneralHashType::XXH64),
            "XXH32" => Ok(GeneralHashType::XXH32),
            "NULL" => Ok(GeneralHashType::NULL),
            _ => Err("SHA512, SHA256, SHA1, XXH64, XXH32, NULL"),
        }
    }
}

impl fmt::Display for GeneralHashType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GeneralHashType::SHA512 => write!(f, "SHA512"),
            GeneralHashType::SHA256 => write!(f, "SHA256"),
            GeneralHashType::SHA1 => write!(f, "SHA1"),
            GeneralHashType::XXH64 => write!(f, "XXH64"),
            GeneralHashType::XXH32 => write!(f, "XXH32"),
            GeneralHashType::NULL => write!(f, "NULL"),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, PartialOrd)]
pub enum GeneralHash {
    SHA512([u8; 64]),
    SHA256([u8; 32]),
    SHA1([u8; 20]),
    XXH64([u8; 8]),
    XXH32([u8; 4]),
    NULL,
}

impl Display for GeneralHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let capacity = match self {
            GeneralHash::SHA512(_) => 128,
            GeneralHash::SHA256(_) => 64,
            GeneralHash::SHA1(_) => 40,
            GeneralHash::XXH64(_) => 16,
            GeneralHash::XXH32(_) => 8,
            GeneralHash::NULL => 0,
        };

        let mut hex = String::with_capacity(capacity + 1 + 6);

        hex.push_str((self.hash_type().to_string() + ":").as_str());

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
                hex.push_str("00");
            }
        }

        write!(f, "{}", hex)
    }
}

impl Serialize for GeneralHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for GeneralHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
        let hex = String::deserialize(deserializer)?;
        let mut iter = hex.split(':');
        let hash_type = GeneralHashType::from_str(iter.next().ok_or_else(|| D::Error::custom("No hash type"))?).map_err(|err| D::Error::custom(format!("Failed to parse hash type: {}", err)))?;
        let data = iter.next().ok_or_else(|| D::Error::custom("No hash data"))?;
        let data = utils::decode_hex(data).map_err(|err| D::Error::custom(format!("Failed to decode hash data: {}", err)))?;
        let mut hash = GeneralHash::from_type(hash_type);
        match &mut hash {
            GeneralHash::SHA512(target_data) => {
                if data.len() != 64 {
                    return Err(D::Error::custom("Invalid data length"));
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::SHA256(target_data) => {
                if data.len() != 32 {
                    return Err(D::Error::custom("Invalid data length"));
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::SHA1(target_data) => {
                if data.len() != 20 {
                    return Err(D::Error::custom("Invalid data length"));
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::XXH64(target_data) => {
                if data.len() != 8 {
                    return Err(D::Error::custom("Invalid data length"));
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::XXH32(target_data) => {
                if data.len() != 4 {
                    return Err(D::Error::custom("Invalid data length"));
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::NULL => {}
        }
        Ok(hash)
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
