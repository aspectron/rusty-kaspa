use crate::result::Result;

#[derive(Debug, Clone)]
pub enum Fees {
    None,
    Include(u64),
    Exclude(u64),
}

impl From<i64> for Fees {
    fn from(fee: i64) -> Self {
        if fee < 0 {
            Fees::Include(fee.unsigned_abs())
        } else {
            Fees::Exclude(fee as u64)
        }
    }
}

impl From<u64> for Fees {
    fn from(fee: u64) -> Self {
        Fees::Exclude(fee)
    }
}

impl TryFrom<&str> for Fees {
    type Error = crate::error::Error;
    fn try_from(fee: &str) -> Result<Self> {
        if fee.is_empty() {
            Ok(Fees::None)
        } else {
            let fee = crate::utils::try_kaspa_str_to_sompi_i64(fee)?.unwrap_or(0);
            Ok(Fees::from(fee))
        }
    }
}

impl TryFrom<String> for Fees {
    type Error = crate::error::Error;
    fn try_from(fee: String) -> Result<Self> {
        Self::try_from(fee.as_str())
    }
}
