// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use liquid_prelude::{
    string::{String, ToString},
    vec::Vec,
};

pub const HASH_LENGTH: usize = 32;

#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Hash([u8; HASH_LENGTH]);

impl Default for Hash {
    fn default() -> Self {
        Self([0u8; HASH_LENGTH])
    }
}

impl Hash {
    pub fn as_ptr(&self) -> *const [u8; HASH_LENGTH] {
        &self.0 as *const _
    }
}

impl From<[u8; HASH_LENGTH]> for Hash {
    fn from(hash: [u8; HASH_LENGTH]) -> Self {
        Self(hash)
    }
}

impl From<Vec<u8>> for Hash {
    fn from(bytes: Vec<u8>) -> Self {
        assert!(bytes.len() == HASH_LENGTH);

        let mut hash = [0u8; HASH_LENGTH];
        hash[..HASH_LENGTH].clone_from_slice(&bytes[..HASH_LENGTH]);
        Self(hash)
    }
}

impl From<&str> for Hash {
    fn from(mut hash: &str) -> Self {
        if !hash.is_ascii() {
            panic!("invalid hash representation");
        }

        if hash.starts_with("0x") || hash.starts_with("0X") {
            if hash.len() != HASH_LENGTH * 2 + 2 {
                panic!("invalid hash representation");
            }
            hash = &hash[2..];
        } else if hash.len() != HASH_LENGTH * 2 {
            panic!("invalid hash representation");
        }

        let mut ret = [0u8; HASH_LENGTH];
        let bytes = hash.as_bytes();
        for i in 0..HASH_LENGTH {
            let high = (bytes[i * 2] as char).to_digit(16).unwrap();
            let low = (bytes[i * 2 + 1] as char).to_digit(16).unwrap();
            let digit = (high << 4) + low;
            ret[i] = digit as u8;
        }
        Self(ret)
    }
}

impl ToString for Hash {
    fn to_string(&self) -> String {
        let mut ret = String::with_capacity(HASH_LENGTH * 2 + 2);
        ret.push_str("0x");

        for digit in self.0.iter() {
            let low = digit & 0x0fu8;
            let high = digit >> 4;
            ret.push(core::char::from_digit(high.into(), 16).unwrap());
            ret.push(core::char::from_digit(low.into(), 16).unwrap());
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let hash: Hash =
            "27772adc63db07aae765b71eb2b533064fa781bd57457e1b138592d8198d0959".into();
        assert_eq!(
            hash.to_string(),
            "0x27772adc63db07aae765b71eb2b533064fa781bd57457e1b138592d8198d0959"
        );
        assert_eq!(
            hash,
            Hash::from([
                0x27, 0x77, 0x2a, 0xdc, 0x63, 0xdb, 0x07, 0xaa, 0xe7, 0x65, 0xb7, 0x1e,
                0xb2, 0xb5, 0x33, 0x06, 0x4f, 0xa7, 0x81, 0xbd, 0x57, 0x45, 0x7e, 0x1b,
                0x13, 0x85, 0x92, 0xd8, 0x19, 0x8d, 0x09, 0x59
            ])
        )
    }

    #[test]
    #[should_panic]
    fn invalid_hash() {
        let _: Hash =
            "0x772adc63db07aae765b71eb2b533064fa781bd57457e1b138592d8198d0959".into();
    }
}
