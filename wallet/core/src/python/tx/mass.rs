use crate::imports::*;
use crate::tx::{mass, MAXIMUM_STANDARD_TRANSACTION_MASS};
use kaspa_consensus_client::Transaction;
use kaspa_consensus_core::config::params::Params;

#[pyfunction]
pub fn maximum_standard_transaction_mass() -> u64 {
    MAXIMUM_STANDARD_TRANSACTION_MASS
}

#[pyfunction]
#[pyo3(name = "calculate_transaction_mass")]
#[pyo3(signature = (network_id, tx, minimum_signatures=None))]
pub fn calculate_unsigned_transaction_mass(network_id: &str, tx: &Transaction, minimum_signatures: Option<u16>) -> PyResult<u64> {
    let network_id = NetworkId::from_str(network_id)?;
    let consensus_params = Params::from(network_id);
    let network_params = NetworkParams::from(network_id);
    let mc = mass::MassCalculator::new(&consensus_params, &network_params);
    Ok(mc.calc_overall_mass_for_unsigned_client_transaction(tx, minimum_signatures.unwrap_or(1))?)
}

#[pyfunction]
#[pyo3(name = "update_transaction_mass")]
#[pyo3(signature = (network_id, tx, minimum_signatures=None))]
pub fn update_unsigned_transaction_mass(network_id: &str, tx: &Transaction, minimum_signatures: Option<u16>) -> PyResult<bool> {
    let network_id = NetworkId::from_str(network_id)?;
    let consensus_params = Params::from(network_id);
    let network_params = NetworkParams::from(network_id);
    let mc = mass::MassCalculator::new(&consensus_params, network_params);
    let mass = mc.calc_overall_mass_for_unsigned_client_transaction(tx, minimum_signatures.unwrap_or(1))?;
    if mass > MAXIMUM_STANDARD_TRANSACTION_MASS {
        Ok(false)
    } else {
        tx.set_mass(mass);
        Ok(true)
    }
}

#[pyfunction]
#[pyo3(name = "calculate_transaction_fee")]
#[pyo3(signature = (network_id, tx, minimum_signatures=None))]
pub fn calculate_unsigned_transaction_fee(
    network_id: &str,
    tx: &Transaction,
    minimum_signatures: Option<u16>,
) -> PyResult<Option<u64>> {
    let network_id = NetworkId::from_str(network_id)?;
    let consensus_params = Params::from(network_id);
    let network_params = NetworkParams::from(network_id);
    let mc = mass::MassCalculator::new(&consensus_params, network_params);
    let mass = mc.calc_overall_mass_for_unsigned_client_transaction(tx, minimum_signatures.unwrap_or(1))?;
    if mass > MAXIMUM_STANDARD_TRANSACTION_MASS {
        Ok(None)
    } else {
        Ok(Some(mc.calc_fee_for_mass(mass)))
    }
}
