use crate::data::{GeneralHash, GeneralHasher};

pub struct NullHasher {
    
}

impl GeneralHasher for NullHasher {
    fn new() -> Self {
        NullHasher {
            
        }
    }

    fn update(&mut self, _data: &[u8]) {
        
    }

    fn finalize(self: Box<Self>) -> GeneralHash {
        GeneralHash::NULL
    }
}
