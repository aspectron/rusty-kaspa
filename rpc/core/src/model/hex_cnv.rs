#[cfg(test)]
mod tests {
    use kaspa_consensus_core::BlueWorkType;
    use kaspa_utils::hex::*;
    use smallvec::{smallvec, SmallVec};

    #[test]
    fn test_vec_hex_convert() {
        let v: Vec<u8> = vec![0x0, 0xab, 0x55, 0x30, 0x1f, 0x63];
        let k = "00ab55301f63";
        assert_eq!(k.len(), v.len() * 2);
        assert_eq!(k.to_string(), v.to_hex());
        assert_eq!(Vec::from_hex(k).unwrap(), v);

        assert!(Vec::from_hex("not a number").is_err());
        assert!(Vec::from_hex("ab01").is_ok());

        // even str length is required
        assert!(Vec::from_hex("ab0").is_err());
        // empty str is supported
        assert_eq!(Vec::from_hex("").unwrap().len(), 0);
    }

    #[test]
    fn test_smallvec_hex_convert() {
        type TestVec = SmallVec<[u8; 36]>;

        let v: TestVec = smallvec![0x0, 0xab, 0x55, 0x30, 0x1f, 0x63];
        let k = "00ab55301f63";
        assert_eq!(k.len(), v.len() * 2);
        assert_eq!(k.to_string(), v.to_hex());
        assert_eq!(SmallVec::<[u8; 36]>::from_hex(k).unwrap(), v);

        assert!(TestVec::from_hex("not a number").is_err());
        assert!(TestVec::from_hex("ab01").is_ok());

        // even str length is required
        assert!(TestVec::from_hex("ab0").is_err());
        // empty str is supported
        assert_eq!(TestVec::from_hex("").unwrap().len(), 0);
    }

    #[test]
    fn test_blue_work_type_hex_convert() {
        const HEX_STR: &str = "a1b21";
        const HEX_VAL: u64 = 0xa1b21;
        let b: BlueWorkType = BlueWorkType::from_u64(HEX_VAL);
        assert_eq!(HEX_STR.to_string(), b.to_hex());
        assert!(BlueWorkType::from_hex("not a number").is_err());

        // max str len is 48 for a 192 bits Uint
        // odd lengths are accepted
        // leading '0' are ignored
        // empty str is supported
        const TEST_STR: &str = "000fedcba987654321000000a9876543210fedcba9876543210fedcba9876543210";
        for i in 0..TEST_STR.len() {
            assert!(BlueWorkType::from_hex(&TEST_STR[0..i]).is_ok() == (i <= 48));
            if 0 < i && i < 33 {
                let b = BlueWorkType::from_hex(&TEST_STR[0..i]).unwrap();
                let u = u128::from_str_radix(&TEST_STR[0..i], 16).unwrap();
                assert_eq!(b, BlueWorkType::from_u128(u));
                assert_eq!(b.to_hex(), format!("{u:x}"));
            }
        }
    }
}
