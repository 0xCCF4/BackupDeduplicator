use std::fmt;
use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;
use serde::{Deserialize, Serialize, Serializer};
use serde::de::Error;
use crate::data::{File, FilePath};
use crate::utils;


/// `GeneralHashType` is an enum that represents the different types of hash functions that can be used.
///
/// The following hash functions are supported: SHA512, SHA256, SHA1, XXH64, XXH32, and NULL.
///
/// The `hasher` method returns a new instance of a `GeneralHasher` trait object that corresponds to the hash type.
/// The `hasher` can then be used to compute a hash of that kind.
///
/// # Traits
/// * `FromStr` - to allow parsing a string into a `GeneralHashType`.
/// * `Display` - to allow formatting a `GeneralHashType` into a string.
///
/// # Examples
/// ```
/// use std::str::FromStr;
/// use backup_deduplicator::data::GeneralHashType;
///
/// let hash_type = GeneralHashType::from_str("SHA256").unwrap();
/// let mut hasher = hash_type.hasher();
/// hasher.update(b"Hello, world!".as_slice());
///
/// assert_eq!(hash_type.to_string(), "SHA256");
///
/// let hash = hasher.finalize();
/// assert_eq!(hash.to_string(), "SHA256:315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3");
/// assert_eq!(hash_type, GeneralHashType::SHA256);
///
/// ```
///
/// # See also
/// * `GeneralHash` - representation of a hash value.
/// * `GeneralHasher` - trait for computing hash values.
///
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
    /// Returns a new instance of a `GeneralHasher` trait object that corresponds to the hash type.
    /// The `hasher` can then be used to compute a hash of that kind.
    ///
    /// # Returns
    /// A new instance of a `GeneralHasher` trait object.
    ///
    /// # Examples
    /// See the example in the `GeneralHashType` documentation.
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

impl Display for GeneralHashType {
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

/// `GeneralHash` is an enum that represents a hash value.
///
/// The hash value is stored as a byte array of a fixed size.
/// The size of the byte array depends on the hash function used.
///
/// The following hash functions are supported: SHA512, SHA256, SHA1, XXH64, XXH32, and NULL.
///
/// The `hash_type` method returns the type of the hash function used.
/// The `hasher` method returns a new instance of a `GeneralHasher` trait object that corresponds to the hash type.
/// The `hasher` can then be used to compute a hash of that kind.
///
/// # Traits
/// * `Display` - to allow formatting a `GeneralHash` into a string.
/// * `FromStr` - to allow parsing a string into a `GeneralHash`.
/// * `Serialize` - to allow serializing a `GeneralHash` into a string.
/// * `Deserialize` - to allow deserializing a `GeneralHash` from a string.
///
/// # Examples
/// ```
/// use std::str::FromStr;
/// use backup_deduplicator::data::{GeneralHash, GeneralHashType};
///
/// let hash = GeneralHash::from_str("SHA256:315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3").unwrap();
///
/// let mut hasher = GeneralHashType::SHA256.hasher();
/// hasher.update(b"Hello, world!".as_slice());
/// let new_hash = hasher.finalize();
///
/// assert_eq!(hash, new_hash);
/// assert_eq!(hash.to_string(), new_hash.to_string());
///
/// ```
///
/// # See also
/// * `GeneralHashType` - representation of the different types of hash functions.
/// * `GeneralHasher` - trait for computing hash values.
///
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

impl FromStr for GeneralHash {
    type Err = &'static str;

    fn from_str(hex: &str) -> Result<Self, Self::Err> {
        let mut iter = hex.split(':');
        let hash_type = GeneralHashType::from_str(iter.next().ok_or_else(|| "No hash type")?).map_err(|_| "Failed to parse hash type")?;
        let data = iter.next().ok_or_else(|| "No hash data")?;
        let data = utils::decode_hex(data).map_err(|_| "Failed to decode hash data")?;
        let mut hash = GeneralHash::from_type(hash_type);
        match &mut hash {
            GeneralHash::SHA512(target_data) => {
                if data.len() != 64 {
                    return Err("Invalid data length");
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::SHA256(target_data) => {
                if data.len() != 32 {
                    return Err("Invalid data length");
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::SHA1(target_data) => {
                if data.len() != 20 {
                    return Err("Invalid data length");
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::XXH64(target_data) => {
                if data.len() != 8 {
                    return Err("Invalid data length");
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::XXH32(target_data) => {
                if data.len() != 4 {
                    return Err("Invalid data length");
                }
                target_data.copy_from_slice(&data);
            },
            GeneralHash::NULL => {}
        }
        Ok(hash)
    }
}

impl<'de> Deserialize<'de> for GeneralHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
        let hex = String::deserialize(deserializer)?;
        GeneralHash::from_str(hex.as_str()).map_err(D::Error::custom)
    }
}

impl GeneralHash {
    /// Returns the hash value as a byte array.
    ///
    /// # Returns
    /// A reference to the byte array that represents the hash value.
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

    /// Returns a new instance of a SHA512 hash value.
    pub fn new_sha512() -> Self { Self::from_type(GeneralHashType::SHA512) }
    /// Returns a new instance of a SHA256 hash value.
    pub fn new_sha256() -> Self { Self::from_type(GeneralHashType::SHA256) }
    /// Returns a new instance of a SHA1 hash value.
    pub fn new_sha1() -> Self { Self::from_type(GeneralHashType::SHA1) }
    /// Returns a new instance of a XXH64 hash value.
    pub fn new_xxh64() -> Self { Self::from_type(GeneralHashType::XXH64) }
    /// Returns a new instance of a XXH32 hash value.
    pub fn new_xxh32() -> Self { Self::from_type(GeneralHashType::XXH32) }

    /// Returns the type of the hash function used.
    ///
    /// # Returns
    /// The type of the hash function used.
    ///
    /// # Examples
    /// ```
    /// use backup_deduplicator::data::{GeneralHash, GeneralHashType};
    ///
    /// let hash = GeneralHash::new_sha256();
    ///
    /// let m = match hash.hash_type() {
    ///     GeneralHashType::SHA256 => true,
    ///     _ => false,
    /// };
    ///
    /// assert!(m);
    ///
    /// ```
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

    /// Returns a new instance of a `GeneralHash` with the specified hash type.
    ///
    /// # Arguments
    /// * `hash_type` - The type of the hash function to use.
    ///
    /// # Returns
    /// A new instance of a `GeneralHash` with the specified hash type.
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

    /// Returns a new instance of a `GeneralHash` with the specified hash type.
    ///
    /// # Arguments
    /// * `hash_type` - The type of the hash function to use.
    ///
    /// # Returns
    /// A new instance of a `GeneralHash` with the specified hash type.
    ///
    /// # See also
    /// * `GeneralHashType` - representation of the different types of hash functions.
    pub fn hasher(&self) -> Box<dyn GeneralHasher> {
        self.hash_type().hasher()
    }

    /// Computes the hash value of the specified data.
    ///
    /// # Arguments
    /// * `reader` - The data to hash (supplied as `std::io::Read`).
    ///
    /// # Returns
    /// The size of the data that was hashed.
    ///
    /// # Errors
    /// Returns an error if the data could not be read.
    pub fn hash_file<T>(&mut self, mut reader: T) -> anyhow::Result<u64>
        where T: std::io::Read {

        let mut hasher = self.hasher();
        let mut buffer = [0; 4096];
        let mut content_size = 0;

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            content_size += bytes_read as u64;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        *self = hasher.finalize();

        Ok(content_size)
    }

    /// Computes the hash value of file iterator/directory.
    ///
    /// # Arguments
    /// * `children` - The iterator of files to hash.
    ///
    /// # Returns
    /// The count of files that were hashed.
    ///
    /// # Errors
    /// Does not return an error. Might return an error in the future.
    pub fn hash_directory<'a>(&mut self, children: impl Iterator<Item = &'a File>) -> anyhow::Result<u64> {
        let mut hasher = self.hasher();

        let mut content_size = 0;

        for child in children {
            content_size += 1;
            hasher.update(child.get_content_hash().as_bytes());
        }

        *self = hasher.finalize();

        Ok(content_size)
    }

    /// Computes the hash value of the specified path.
    ///
    /// # Arguments
    /// * `path` - The path to hash.
    ///
    /// # Returns
    /// Does not return a value.
    ///
    /// # Errors
    /// Does not return an error. Might return an error in the future.
    pub fn hash_path(&mut self, path: &Path) -> anyhow::Result<()> {
        let mut hasher = self.hasher();

        hasher.update(path.as_os_str().as_encoded_bytes());

        *self = hasher.finalize();

        Ok(())
    }

    /// Computes the hash value of the specified file path.
    ///
    /// # Arguments
    /// * `path` - The file path to hash.
    ///
    /// # Returns
    /// Does not return a value.
    ///
    /// # Errors
    /// Does not return an error. Might return an error in the future.
    pub fn hash_filepath(&mut self, path: &FilePath) -> anyhow::Result<()> {
        let mut hasher = self.hasher();

        for component in &path.path {
            hasher.update(component.path.as_os_str().as_encoded_bytes());
        }

        *self = hasher.finalize();

        Ok(())
    }
}

/// `GeneralHasher` is a trait for computing hash values.
///
/// # Methods
/// * `new` - creates a new instance of a `GeneralHasher`.
/// * `update` - updates the hash value with the specified data.
/// * `finalize` - finalizes the hash value and returns the result.
///
/// # Examples
/// See the example in the `GeneralHash` documentation.
///
/// # See also
/// * `GeneralHash` - representation of a hash value.
/// * `GeneralHashType` - representation of the different types of hash functions.
pub trait GeneralHasher {
    /// Creates a new instance of a `GeneralHasher`.
    ///
    /// # Returns
    /// A new instance of a `GeneralHasher`.
    fn new() -> Self where Self: Sized;

    /// Updates the hash value with the specified data.
    ///
    /// # Arguments
    /// * `data` - The data to hash.
    fn update(&mut self, data: &[u8]);

    /// Finalizes the hash value and returns the result.
    /// Consumes the `GeneralHasher` instance.
    ///
    /// # Returns
    /// The hash value.
    fn finalize(self: Box<Self>) -> GeneralHash;
}

/// `GeneralHasher` implementation for the SHA1 crate
mod sha1;
/// `GeneralHasher` implementation for the SHA2 crate
mod sha2;
/// `GeneralHasher` implementation for the XXH crate
mod xxh;
/// `GeneralHasher` implementation for the NULL hash function
mod null;
