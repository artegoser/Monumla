//! Shared in-process identities without walking the value twice.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

struct FingerprintHasher {
    low: DefaultHasher,
    high: DefaultHasher,
}

impl Hasher for FingerprintHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.low.finish()
    }

    // DefaultHasher is byte-wise: the default integer/string writes preserve
    // its encoding. Compatibility tests guard both supported toolchains.
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.low.write(bytes);
        self.high.write(bytes);
    }
}

/// Deterministic in-process identity, independent of pointer addresses.
/// Not a portable file hash or a cryptographic digest.
pub fn fingerprint(value: &impl Hash) -> u128 {
    let mut hasher = FingerprintHasher {
        low: DefaultHasher::new(),
        high: DefaultHasher::new(),
    };
    0x9e37_79b9_7f4a_7c15_u64.hash(&mut hasher.high);
    value.hash(&mut hasher);
    u128::from(hasher.low.finish()) | (u128::from(hasher.high.finish()) << 64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StateMap, StateValue};

    fn two_pass(value: &impl Hash) -> u128 {
        let mut low = DefaultHasher::new();
        value.hash(&mut low);
        let mut high = DefaultHasher::new();
        0x9e37_79b9_7f4a_7c15_u64.hash(&mut high);
        value.hash(&mut high);
        u128::from(low.finish()) | (u128::from(high.finish()) << 64)
    }

    struct PrimitiveWrites;
    impl Hash for PrimitiveWrites {
        fn hash<H: Hasher>(&self, h: &mut H) {
            h.write(b"prefix\0");
            h.write_u8(u8::MAX);
            h.write_u16(u16::MAX);
            h.write_u32(u32::MAX);
            h.write_u64(u64::MAX);
            h.write_u128(u128::MAX);
            h.write_usize(usize::MAX);
            h.write_i8(i8::MIN);
            h.write_i16(i16::MIN);
            h.write_i32(i32::MIN);
            h.write_i64(i64::MIN);
            h.write_i128(i128::MIN);
            h.write_isize(isize::MIN);
            "prefix-free string 🦀".hash(h);
            h.write(&[]);
        }
    }

    #[test]
    fn single_walk_matches_two_pass_primitive_and_prefix_encodings() {
        assert_eq!(fingerprint(&PrimitiveWrites), two_pass(&PrimitiveWrites));
        for length in 0..512 {
            let bytes: Vec<_> = (0..length).map(|i| i as u8).collect();
            assert_eq!(fingerprint(&bytes), two_pass(&bytes));
            let text = "x\0🦀".repeat(length);
            assert_eq!(fingerprint(&text), two_pass(&text));
        }
        for strings in [("a", "bc"), ("ab", "c"), ("", "abc")] {
            assert_eq!(fingerprint(&strings), two_pass(&strings));
        }
    }

    #[test]
    fn single_walk_preserves_nested_state_fingerprints() {
        let mut nested = StateMap::new();
        for i in 0..32_u64 {
            nested.insert(format!("key-{i}"), i);
        }
        let values = vec![
            StateValue::Bool(true),
            StateValue::I64(i64::MIN),
            StateValue::U64(u64::MAX),
            StateValue::F64(-0.0),
            StateValue::F64(f64::INFINITY),
            StateValue::F64(f64::NAN),
            StateValue::String("nested\0value".into()),
            StateValue::Map(nested),
        ];
        let mut state = StateMap::new();
        for (index, value) in values.iter().enumerate() {
            assert_eq!(fingerprint(value), two_pass(value));
            state.insert(index.to_string(), value.clone());
            state.insert("list", values.clone());
            assert_eq!(fingerprint(&state), two_pass(&state));
        }
        assert_eq!(
            fingerprint(&StateValue::F64(0.0)),
            fingerprint(&StateValue::F64(-0.0))
        );
    }
}
