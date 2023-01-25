use crate::{
    asserts::{assert_min_collateral_ratio, assert_protocol_fee},
    migration::migrate_asset_configs,
    positions::{
        burn, deposit, mint, open_position, query_next_position_idx, query_position,
        query_positions, withdraw,
    },
    state::{
        read_asset_config, read_config, store_asset_config, store_config, store_position_idx,
        AssetConfig, Config,
    },
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut,
    Env, MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ReceiveMsg;
use melange_protocol::mint::{
    AssetConfigResponse, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg,
    QueryMsg,
};
use melange_protocol::{
    collateral_oracle::{ExecuteMsg as CollateralOracleExecuteMsg, SourceType},
    mint::MigrateMsg,
};
use terraswap::asset::{Asset, AssetInfo};

use sei_cosmwasm::{
    BulkOrderPlacementsResponse, ContractOrderResult, DepositInfo, DexTwapsResponse, EpochResponse,
    ExchangeRatesResponse, GetLatestPriceResponse, GetOrderByIdResponse, GetOrdersResponse,
    LiquidationRequest, LiquidationResponse, MsgPlaceOrdersResponse, OracleTwapsResponse, Order,
    OrderSimulationResponse, OrderType, PositionDirection, SeiMsg, SeiQuerier, SeiQueryWrapper,
    SettlementEntry, SudoMsg,
};

pub const MIN_CR_ALLOWED: &str = "1.1";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: deps.api.addr_canonicalize(&msg.owner)?,
        oracle: deps.api.addr_canonicalize(&msg.oracle)?,
        collector: deps.api.addr_canonicalize(&msg.collector)?,
        collateral_oracle: deps.api.addr_canonicalize(&msg.collateral_oracle)?,
        staking: deps.api.addr_canonicalize(&msg.staking)?,
        tswap_factory: deps.api.addr_canonicalize(&msg.tswap_factory)?,
        base_denom: msg.base_denom,
        token_code_id: msg.token_code_id,
        protocol_fee_rate: assert_protocol_fee(msg.protocol_fee_rate)?,
    };

    store_config(deps.storage, &config)?;
    store_position_idx(deps.storage, Uint128::from(1u128))?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            oracle,
            collector,
            collateral_oracle,
            tswap_factory,
            token_code_id,
            protocol_fee_rate,
            staking,
        } => update_config(
            deps,
            info,
            owner,
            oracle,
            collector,
            collateral_oracle,
            tswap_factory,
            token_code_id,
            protocol_fee_rate,
            staking,
        ),
        ExecuteMsg::UpdateAsset {
            asset_token,
            min_collateral_ratio,
        } => {
            let asset_addr = deps.api.addr_validate(asset_token.as_str())?;
            update_asset(
                deps,
                info,
                asset_addr,
                min_collateral_ratio,
            )
        }
        ExecuteMsg::RegisterAsset {
            asset_token,
            min_collateral_ratio,
        } => {
            let asset_addr = deps.api.addr_validate(asset_token.as_str())?;
            register_asset(
                deps,
                info,
                asset_addr,
                min_collateral_ratio,
            )
        }
        ExecuteMsg::RegisterMigration {
            asset_token,
            end_price,
        } => {
            let asset_addr = deps.api.addr_validate(asset_token.as_str())?;
            register_migration(deps, info, asset_addr, end_price)
        }
        ExecuteMsg::OpenPosition {
            collateral,
            asset_info,
            collateral_ratio,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::generic_err("unauthorized"));
            }

            // Check the actual deposit happens
            collateral.assert_sent_native_token_balance(&info)?;

            open_position(
                deps,
                info.sender,
                collateral,
                asset_info,
                collateral_ratio,
            )
        }
        ExecuteMsg::Deposit {
            position_idx,
            collateral,
        } => {
            // only native token can be deposited directly
            if !collateral.is_native_token() {
                return Err(StdError::generic_err("unauthorized"));
            }

            // Check the actual deposit happens
            collateral.assert_sent_native_token_balance(&info)?;

            deposit(deps, info.sender, position_idx, collateral)
        }
        ExecuteMsg::Withdraw {
            position_idx,
            collateral,
        } => withdraw(deps, info.sender, position_idx, collateral),
        ExecuteMsg::Mint {
            position_idx,
            asset,
        } => mint(deps, info.sender, position_idx, asset),
    }
}

pub fn receive_cw20(
    deps: DepsMut<SeiQueryWrapper>,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let passed_asset: Asset = Asset {
        info: AssetInfo::Token {
            contract_addr: info.sender.to_string(),
        },
        amount: cw20_msg.amount,
    };

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::OpenPosition {
               asset_info,
               collateral_ratio,
           }) => {
            let cw20_sender = deps.api.addr_validate(cw20_msg.sender.as_str())?;
            open_position(
                deps,
                cw20_sender,
                passed_asset,
                asset_info,
                collateral_ratio,
            )
        }
        Ok(Cw20HookMsg::Deposit { position_idx }) => {
            let cw20_sender = deps.api.addr_validate(cw20_msg.sender.as_str())?;
            deposit(deps, cw20_sender, position_idx, passed_asset)
        }
        Ok(Cw20HookMsg::Burn { position_idx }) => {
            let cw20_sender = deps.api.addr_validate(cw20_msg.sender.as_str())?;
            burn(deps, cw20_sender, position_idx, passed_asset)
        }
        Err(_) => Err(StdError::generic_err("invalid cw20 hook message")),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut<SeiQueryWrapper>,
    info: MessageInfo,
    owner: Option<String>,
    oracle: Option<String>,
    collector: Option<String>,
    collateral_oracle: Option<String>,
    tswap_factory: Option<String>,
    token_code_id: Option<u64>,
    protocol_fee_rate: Option<Decimal>,
    staking: Option<String>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;

    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(oracle) = oracle {
        config.oracle = deps.api.addr_canonicalize(&oracle)?;
    }

    if let Some(collector) = collector {
        config.collector = deps.api.addr_canonicalize(&collector)?;
    }

    if let Some(collateral_oracle) = collateral_oracle {
        config.collateral_oracle = deps.api.addr_canonicalize(&collateral_oracle)?;
    }

    if let Some(tswap_factory) = tswap_factory {
        config.tswap_factory = deps.api.addr_canonicalize(&tswap_factory)?;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(protocol_fee_rate) = protocol_fee_rate {
        assert_protocol_fee(protocol_fee_rate)?;
        config.protocol_fee_rate = protocol_fee_rate;
    }

    if let Some(staking) = staking {
        config.staking = deps.api.addr_canonicalize(&staking)?;
    }

    store_config(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn update_asset(
    deps: DepsMut<SeiQueryWrapper>,
    info: MessageInfo,
    asset_token: Addr,
    min_collateral_ratio: Option<Decimal>,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;
    let mut asset: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;

    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(min_collateral_ratio) = min_collateral_ratio {
        assert_min_collateral_ratio(min_collateral_ratio)?;
        asset.min_collateral_ratio = min_collateral_ratio;
    }

    store_asset_config(deps.storage, &asset_token_raw, &asset)?;
    Ok(Response::new().add_attribute("action", "update_asset"))
}

pub fn register_asset(
    deps: DepsMut<SeiQueryWrapper>,
    info: MessageInfo,
    asset_token: Addr,
    min_collateral_ratio: Decimal,
) -> StdResult<Response> {
    assert_min_collateral_ratio(min_collateral_ratio)?;

    let config: Config = read_config(deps.storage)?;

    // permission check
    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;
    if read_asset_config(deps.storage, &asset_token_raw).is_ok() {
        return Err(StdError::generic_err("Asset was already registered"));
    }

    let mut messages: Vec<CosmosMsg> = vec![];


    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps
            .api
            .addr_humanize(&config.collateral_oracle)?
            .to_string(),
        funds: vec![],
        msg: to_binary(&CollateralOracleExecuteMsg::RegisterCollateralAsset {
            asset: AssetInfo::Token {
                contract_addr: asset_token.to_string(),
            },
            multiplier: Decimal::one(), // default collateral multiplier for new mAssets
            price_source: SourceType::MelangeOracle {
                oracle_addr: deps.api.addr_humanize(&config.oracle)?.to_string(),
            },
        })?,
    }));

    // Store temp info into base asset store
    store_asset_config(
        deps.storage,
        &asset_token_raw,
        &AssetConfig {
            token: deps.api.addr_canonicalize(asset_token.as_str())?,
            min_collateral_ratio,
            end_price: None,
        },
    )?;

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "register"),
            attr("asset_token", asset_token),
        ])
        .add_messages(messages))
}

pub fn register_migration(
    deps: DepsMut<SeiQueryWrapper>,
    info: MessageInfo,
    asset_token: Addr,
    end_price: Decimal,
) -> StdResult<Response> {
    let config = read_config(deps.storage)?;
    if config.owner != deps.api.addr_canonicalize(info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    let asset_token_raw = deps.api.addr_canonicalize(asset_token.as_str())?;
    let asset_config: AssetConfig = read_asset_config(deps.storage, &asset_token_raw)?;

    // update asset config
    store_asset_config(
        deps.storage,
        &asset_token_raw,
        &AssetConfig {
            end_price: Some(end_price),
            min_collateral_ratio: Decimal::percent(100),
            ..asset_config
        },
    )?;

    // flag asset as revoked in the collateral oracle
    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps
                .api
                .addr_humanize(&config.collateral_oracle)?
                .to_string(),
            funds: vec![],
            msg: to_binary(&CollateralOracleExecuteMsg::RevokeCollateralAsset {
                asset: AssetInfo::Token {
                    contract_addr: asset_token.to_string(),
                },
            })?,
        })])
        .add_attributes(vec![
            attr("action", "migrate_asset"),
            attr("asset_token", asset_token.as_str()),
            attr("end_price", end_price.to_string()),
        ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps<SeiQueryWrapper>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::AssetConfig { asset_token } => to_binary(&query_asset_config(deps, asset_token)?),
        QueryMsg::Position { position_idx } => to_binary(&query_position(deps, position_idx)?),
        QueryMsg::Positions {
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        } => to_binary(&query_positions(
            deps,
            owner_addr,
            asset_token,
            start_after,
            limit,
            order_by,
        )?),
        QueryMsg::NextPositionIdx {} => to_binary(&query_next_position_idx(deps)?),
    }
}

pub fn query_config(deps: Deps<SeiQueryWrapper>) -> StdResult<ConfigResponse> {
    let state = read_config(deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.addr_humanize(&state.owner)?.to_string(),
        oracle: deps.api.addr_humanize(&state.oracle)?.to_string(),
        staking: deps.api.addr_humanize(&state.staking)?.to_string(),
        collector: deps.api.addr_humanize(&state.collector)?.to_string(),
        collateral_oracle: deps
            .api
            .addr_humanize(&state.collateral_oracle)?
            .to_string(),
        tswap_factory: deps
            .api
            .addr_humanize(&state.tswap_factory)?
            .to_string(),
        base_denom: state.base_denom,
        token_code_id: state.token_code_id,
        protocol_fee_rate: state.protocol_fee_rate,
    };

    Ok(resp)
}

pub fn query_asset_config(deps: Deps<SeiQueryWrapper>, asset_token: String) -> StdResult<AssetConfigResponse> {
    let asset_config: AssetConfig = read_asset_config(
        deps.storage,
        &deps.api.addr_canonicalize(asset_token.as_str())?,
    )?;

    let resp = AssetConfigResponse {
        token: deps
            .api
            .addr_humanize(&asset_config.token)
            .unwrap()
            .to_string(),
        min_collateral_ratio: asset_config.min_collateral_ratio,
        end_price: asset_config.end_price,
    };

    Ok(resp)
}
