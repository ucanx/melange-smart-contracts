use std::str::FromStr;

use crate::{
    contract::MIN_CR_ALLOWED,
    state::{AssetConfig, Position},
};
use cosmwasm_std::{Decimal, Deps, Env, StdError, StdResult};
use terraswap::asset::{Asset, AssetInfo};
use sei_cosmwasm::SeiQueryWrapper;

// Check zero balance & same collateral with position
pub fn assert_collateral(
    deps: Deps<SeiQueryWrapper>,
    position: &Position,
    collateral: &Asset
) -> StdResult<()> {
    if !collateral
        .info
        .equal(&position.collateral.info.to_normal(deps.api)?)
        || collateral.amount.is_zero()
    {
        return Err(StdError::generic_err("Wrong collateral"));
    }

    Ok(())
}

// Check zero balance & same asset with position
pub fn assert_asset(deps: Deps<SeiQueryWrapper>, position: &Position, asset: &Asset) -> StdResult<()> {
    if !asset.info.equal(&position.asset.info.to_normal(deps.api)?) || asset.amount.is_zero() {
        return Err(StdError::generic_err("Wrong asset"));
    }

    Ok(())
}

pub fn assert_migrated_asset(asset_config: &AssetConfig) -> StdResult<()> {
    if asset_config.end_price.is_some() {
        return Err(StdError::generic_err(
            "Operation is not allowed for the deprecated asset",
        ));
    }

    Ok(())
}

pub fn assert_revoked_collateral(
    load_collateral_res: (Decimal, Decimal, bool),
) -> StdResult<(Decimal, Decimal)> {
    if load_collateral_res.2 {
        println!("**********************************WoW-1");
        return Err(StdError::generic_err(
            "The collateral asset provided is no longer valid",
        ));
    }

    Ok((load_collateral_res.0, load_collateral_res.1))
}

pub fn assert_min_collateral_ratio(min_collateral_ratio: Decimal) -> StdResult<()> {
    if min_collateral_ratio < Decimal::from_str(MIN_CR_ALLOWED)? {
        Err(StdError::generic_err(format!(
            "min_collateral_ratio must be bigger or equal than {}",
            MIN_CR_ALLOWED
        )))
    } else {
        Ok(())
    }
}

pub fn assert_protocol_fee(protocol_fee_rate: Decimal) -> StdResult<Decimal> {
    if protocol_fee_rate >= Decimal::one() {
        Err(StdError::generic_err(
            "protocol_fee_rate must be smaller than 1",
        ))
    } else {
        Ok(protocol_fee_rate)
    }
}
