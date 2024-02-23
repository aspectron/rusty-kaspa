//"pskt" end with 0xff
pub const PSKT_MAGIC_BYTES: [u8; 5] = [0x70, 0x73, 0x6B, 0x74, 0xFF];

pub const PSKT_SEPARATOR: u8 = 0x00;

// version number
pub const PSKT_HIGHEST_VERSION: u8 = 1;

// Global types
pub const PSKT_GLOBAL_UNSIGNED_TX: u8 = 0x00;
pub const PSKT_GLOBAL_XPUB: u8 = 0x01;
pub const PSKT_GLOBAL_TX_VERSION: u8 = 0x02;
pub const PSKT_GLOBAL_FALLBACK_LOCKTIME: u8 = 0x03;
pub const PSKT_GLOBAL_INPUT_COUNT: u8 = 0x04;
pub const PSKT_GLOBAL_OUTPUT_COUNT: u8 = 0x05;
pub const PSKT_GLOBAL_TX_MODIFIABLE: u8 = 0x06;
pub const PSKT_GLOBAL_VERSION: u8 = 0xFB;
pub const PSKT_GLOBAL_PROPRIETARY: u8 = 0xFC;

// Input types
pub const PSKT_IN_NON_WITNESS_UTXO: u8 = 0x00;
pub const PSKT_IN_WITNESS_UTXO: u8 = 0x01;
pub const PSKT_IN_PARTIAL_SIG: u8 = 0x02;
pub const PSKT_IN_SIGHASH: u8 = 0x03;
pub const PSKT_IN_REDEEM_SCRIPT: u8 = 0x04;
pub const PSKT_IN_WITNESS_SCRIPT: u8 = 0x05;
pub const PSKT_IN_BIP32_DERIVATION: u8 = 0x06;
pub const PSKT_IN_SCRIPT_SIG: u8 = 0x07;
pub const PSKT_IN_SCRIPT_WITNESS: u8 = 0x08;
pub const PSKT_IN_RIPEMD160: u8 = 0x0A;
pub const PSKT_IN_SHA256: u8 = 0x0B;
pub const PSKT_IN_HASH160: u8 = 0x0C;
pub const PSKT_IN_HASH256: u8 = 0x0D;
pub const PSKT_IN_PREVIOUS_TXID: u8 = 0x0e;
pub const PSKT_IN_OUTPUT_INDEX: u8 = 0x0f;
pub const PSKT_IN_SEQUENCE: u8 = 0x10;
pub const PSKT_IN_REQUIRED_TIME_LOCKTIME: u8 = 0x11;
pub const PSKT_IN_REQUIRED_HEIGHT_LOCKTIME: u8 = 0x12;
pub const PSKT_IN_TAP_KEY_SIG: u8 = 0x13;
pub const PSKT_IN_TAP_SCRIPT_SIG: u8 = 0x14;
pub const PSKT_IN_TAP_LEAF_SCRIPT: u8 = 0x15;
pub const PSKT_IN_TAP_BIP32_DERIVATION: u8 = 0x16;
pub const PSKT_IN_TAP_INTERNAL_KEY: u8 = 0x17;
pub const PSKT_IN_TAP_MERKLE_ROOT: u8 = 0x18;
pub const PSKT_IN_PROPRIETARY: u8 = 0xFC;

// Output types
pub const PSKT_OUT_REDEEM_SCRIPT: u8 = 0x00;
pub const PSKT_OUT_WITNESS_SCRIPT: u8 = 0x01;
pub const PSKT_OUT_BIP32_DERIVATION: u8 = 0x02;
pub const PSKT_OUT_AMOUNT: u8 = 0x03;
pub const PSKT_OUT_SCRIPT: u8 = 0x04;
pub const PSKT_OUT_TAP_INTERNAL_KEY: u8 = 0x05;
pub const PSKT_OUT_TAP_TREE: u8 = 0x06;
pub const PSKT_OUT_TAP_BIP32_DERIVATION: u8 = 0x07;
pub const PSKT_OUT_PROPRIETARY: u8 = 0xFC;
