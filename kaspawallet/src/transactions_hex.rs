//! Multi-transaction hex codec: multiple hex-encoded transactions
//! are joined using a literal `_` separator. The character is
//! intentionally outside the hex alphabet so a double-click
//! selection in a terminal still grabs one transaction.

use thiserror::Error;

/// Separator between transactions.
pub const HEX_TRANSACTIONS_SEPARATOR: char = '_';

/// Failure modes of [`decode_transactions_from_hex`].
#[derive(Debug, Error)]
pub enum HexCodecError {
    /// One of the separator-joined chunks failed `hex::decode`.
    #[error("invalid hex for transaction #{index}: {source}")]
    InvalidHex {
        /// Zero-based position of the offending chunk.
        index: usize,
        /// Underlying hex-decoder error.
        #[source]
        source: hex::FromHexError,
    },
}

/// Render a batch of binary transactions as
/// `hex0_hex1_..._hexN`.
pub fn encode_transactions_to_hex(transactions: &[Vec<u8>]) -> String {
    let mut out = String::with_capacity(transactions.iter().map(|t| t.len() * 2).sum::<usize>() + transactions.len());
    for (i, tx) in transactions.iter().enumerate() {
        if i > 0 {
            out.push(HEX_TRANSACTIONS_SEPARATOR);
        }
        out.push_str(&hex::encode(tx));
    }
    out
}

/// Parse the hex-batch string back into binary transactions.
/// Empty input yields a single empty-byte-vector entry (the
/// natural `split('_')` semantics on the empty string).
pub fn decode_transactions_from_hex(transactions_hex: &str) -> Result<Vec<Vec<u8>>, HexCodecError> {
    let mut out = Vec::new();
    for (i, chunk) in transactions_hex.split(HEX_TRANSACTIONS_SEPARATOR).enumerate() {
        let bytes = hex::decode(chunk).map_err(|source| HexCodecError::InvalidHex { index: i, source })?;
        out.push(bytes);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_two_transactions() {
        let txs = vec![vec![0x00, 0x01, 0x02], vec![0xde, 0xad, 0xbe, 0xef]];
        let encoded = encode_transactions_to_hex(&txs);
        assert_eq!(encoded, "000102_deadbeef");
        let decoded = decode_transactions_from_hex(&encoded).expect("decode");
        assert_eq!(decoded, txs);
    }

    #[test]
    fn single_transaction_emits_no_separator() {
        let encoded = encode_transactions_to_hex(&[vec![0xff, 0xee]]);
        assert!(!encoded.contains(HEX_TRANSACTIONS_SEPARATOR));
        assert_eq!(encoded, "ffee");
    }

    #[test]
    fn invalid_hex_chunk_reports_index() {
        let err = decode_transactions_from_hex("abcd_zz").expect_err("must reject non-hex");
        match err {
            HexCodecError::InvalidHex { index, .. } => assert_eq!(index, 1),
        }
    }
}
