use cosmwasm_storage::Bucket;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Decimal, Order, StdError, StdResult, Storage};

use crate::state::{AssetConfig, PREFIX_ASSET_CONFIG};
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyAssetConfig {
    pub token: CanonicalAddr,
    pub min_collateral_ratio: Decimal,
    pub end_price: Option<Decimal>,
}

pub fn migrate_asset_configs(storage: &mut dyn Storage) -> StdResult<()> {
    let mut legacy_asset_configs_bucket: Bucket<LegacyAssetConfig> =
        Bucket::new(storage, PREFIX_ASSET_CONFIG);

    let mut asset_configs: Vec<(CanonicalAddr, LegacyAssetConfig)> = vec![];
    for item in legacy_asset_configs_bucket.range(None, None, Order::Ascending) {
        let (k, p) = item?;
        asset_configs.push((CanonicalAddr::from(k), p));
    }

    for (asset, _) in asset_configs.clone().into_iter() {
        legacy_asset_configs_bucket.remove(asset.as_slice());
    }

    let mut new_asset_configs_bucket: Bucket<AssetConfig> =
        Bucket::new(storage, PREFIX_ASSET_CONFIG);

    for (asset, asset_config) in asset_configs.into_iter() {
        let new_asset_config = &AssetConfig {
            token: asset_config.token,
            min_collateral_ratio: asset_config.min_collateral_ratio,
            end_price: asset_config.end_price,
        };
        new_asset_configs_bucket.save(asset.as_slice(), new_asset_config)?;
    }

    Ok(())
}

#[cfg(test)]
mod migrate_tests {
    use crate::state::read_asset_config;

    use super::*;
    use cosmwasm_std::{testing::mock_dependencies, Api};

    pub fn asset_configs_old_store(storage: &mut dyn Storage) -> Bucket<LegacyAssetConfig> {
        Bucket::new(storage, PREFIX_ASSET_CONFIG)
    }

    #[test]
    fn test_asset_configs_migration() {
        let mut deps = mock_dependencies();
        let mut legacy_store = asset_configs_old_store(&mut deps.storage);

        let asset_config = LegacyAssetConfig {
            token: deps.api.addr_canonicalize("mAPPL").unwrap(),
            min_collateral_ratio: Decimal::percent(150),
            end_price: None,
        };

        legacy_store
            .save(asset_config.token.as_slice(), &asset_config)
            .unwrap();

        migrate_asset_configs(deps.as_mut().storage).unwrap();

        let new_asset_config: AssetConfig =
            read_asset_config(deps.as_mut().storage, &asset_config.token).unwrap();

        assert_eq!(
            new_asset_config,
            AssetConfig {
                token: asset_config.token,
                min_collateral_ratio: asset_config.min_collateral_ratio,
                end_price: asset_config.end_price,
            }
        );
    }
}
