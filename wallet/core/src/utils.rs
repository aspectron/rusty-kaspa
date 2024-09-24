//!
//! Kaspa value formatting and parsing utilities.
//!

use crate::result::Result;
use kaspa_addresses::Address;
use kaspa_consensus_core::constants::*;
use kaspa_consensus_core::network::NetworkType;
use separator::{separated_float, separated_int, separated_uint_with_output, Separatable};
use workflow_log::style;

pub fn try_kaspa_str_to_sompi<S: Into<String>>(s: S, decimals: Option<u32>) -> Result<Option<u64>> {
    let s: String = s.into();
    let amount = s.trim();
    if amount.is_empty() {
        return Ok(None);
    }

    Ok(Some(str_to_sompi(amount, decimals)?))
}

pub fn try_kaspa_str_to_sompi_i64<S: Into<String>>(s: S) -> Result<Option<i64>> {
    let s: String = s.into();
    let amount = s.trim();
    if amount.is_empty() {
        return Ok(None);
    }

    let amount = amount.parse::<f64>()? * SOMPI_PER_KASPA as f64;
    Ok(Some(amount as i64))
}

#[inline]
pub fn sompi_to_kaspa(sompi: u64) -> f64 {
    sompi as f64 / SOMPI_PER_KASPA as f64
}

#[inline]
pub fn sompi_to_kaspa_with_decimals(sompi: u64, decimals: Option<u32>) -> f64 {
    let decimals = decimals.unwrap_or(8);
    let sompi_per_unit = 10u64.pow(decimals);

    sompi as f64 / sompi_per_unit as f64
}

#[inline]
pub fn kaspa_to_sompi(kaspa: f64) -> u64 {
    (kaspa * SOMPI_PER_KASPA as f64) as u64
}

#[inline]
pub fn sompi_to_kaspa_string(sompi: u64) -> String {
    sompi_to_kaspa(sompi).separated_string()
}

#[inline]
pub fn sompi_to_kaspa_string_with_trailing_zeroes(sompi: u64) -> String {
    separated_float!(format!("{:.8}", sompi_to_kaspa(sompi)))
}

pub fn kaspa_suffix(network_type: &NetworkType) -> &'static str {
    match network_type {
        NetworkType::Mainnet => "KAS",
        NetworkType::Testnet => "TKAS",
        NetworkType::Simnet => "SKAS",
        NetworkType::Devnet => "DKAS",
    }
}

#[inline]
pub fn sompi_to_kaspa_string_with_suffix(sompi: u64, network_type: &NetworkType) -> String {
    let kas = sompi_to_kaspa_string(sompi);
    let suffix = kaspa_suffix(network_type);
    format!("{kas} {suffix}")
}

#[inline]
pub fn sompi_to_kaspa_string_with_trailing_zeroes_and_suffix(sompi: u64, network_type: &NetworkType) -> String {
    let kas = sompi_to_kaspa_string_with_trailing_zeroes(sompi);
    let suffix = kaspa_suffix(network_type);
    format!("{kas} {suffix}")
}

pub fn format_address_colors(address: &Address, range: Option<usize>) -> String {
    let address = address.to_string();

    let parts = address.split(':').collect::<Vec<&str>>();
    let prefix = style(parts[0]).dim();
    let payload = parts[1];
    let range = range.unwrap_or(6);
    let start = range;
    let finish = payload.len() - range;

    let left = &payload[0..start];
    let center = style(&payload[start..finish]).dim();
    let right = &payload[finish..];

    format!("{prefix}:{left}:{center}:{right}")
}

fn str_to_sompi(amount: &str, decimals: Option<u32>) -> Result<u64> {
    let decimals = decimals.unwrap_or(8);
    let sompi_per_unit = 10u64.pow(decimals);

    // Check if the amount contains a decimal point, if doesn't return value directly.
    let Some(dot_idx) = amount.find('.') else {
        return Ok(amount.parse::<u64>()? * sompi_per_unit);
    };

    // Parse the integer part of the amount
    let integer = amount[..dot_idx].parse::<u64>()? * sompi_per_unit;

    let decimal = &amount[dot_idx + 1..];
    let decimal_len = decimal.len();
    let decimal = if decimal_len == 0 {
        // If there are no digits after the decimal point, the fractional value is 0.
        0
    } else if decimal_len <= decimals as usize {
        // If its within allowed decimals range, parse it as u64 and pad with zeros to the right to reach the correct precision.
        decimal.parse::<u64>()? * 10u64.pow(decimals - decimal_len as u32)
    } else {
        // Truncate values longer than allowed decimal places.
        // TODO - discuss how to handle values longer than supplied decimal places.
        // (reject, truncate, ceil(), etc.)
        decimal[..decimals as usize].parse::<u64>()?
    };
    Ok(integer + decimal)
}
