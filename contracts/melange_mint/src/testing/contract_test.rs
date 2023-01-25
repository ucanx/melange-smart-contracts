use crate::contract::{execute, instantiate, query};
use crate::testing::mock_querier::mock_dependencies;
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{from_binary, to_binary, CosmosMsg, Deps, DepsMut, Decimal, StdError, SubMsg, WasmMsg};
use melange_protocol::collateral_oracle::{ExecuteMsg::RegisterCollateralAsset, SourceType};
use melange_protocol::mint::{
    AssetConfigResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use terraswap::asset::AssetInfo;
use sei_cosmwasm::{
    BulkOrderPlacementsResponse, ContractOrderResult, DepositInfo, DexTwapsResponse, EpochResponse,
    ExchangeRatesResponse, GetLatestPriceResponse, GetOrderByIdResponse, GetOrdersResponse,
    LiquidationRequest, LiquidationResponse, MsgPlaceOrdersResponse, OracleTwapsResponse, Order,
    OrderSimulationResponse, OrderType, PositionDirection, SeiMsg, SeiQuerier, SeiQueryWrapper,
    SettlementEntry, SudoMsg,
};

static TOKEN_CODE_ID: u64 = 10u64;


#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        oracle: "oracle0000".to_string(),
        collector: "collector0000".to_string(),
        collateral_oracle: "collateraloracle0000".to_string(),
        staking: "staking0000".to_string(),
        tswap_factory: "tswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        token_code_id: TOKEN_CODE_ID,
        protocol_fee_rate: Decimal::percent(1),
    };
    let info = mock_info("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0000", config.owner.as_str());
    assert_eq!("uusd", config.base_denom);
    assert_eq!("oracle0000", config.oracle.as_str());
    assert_eq!("staking0000", config.staking.as_str());
    assert_eq!("collector0000", config.collector.as_str());
    assert_eq!("tswap_factory", config.tswap_factory.as_str());
    assert_eq!(TOKEN_CODE_ID, config.token_code_id);
    assert_eq!(Decimal::percent(1), config.protocol_fee_rate);
}
#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        oracle: "oracle0000".to_string(),
        collector: "collector0000".to_string(),
        collateral_oracle: "collateraloracle0000".to_string(),
        staking: "staking0000".to_string(),
        tswap_factory: "tswap_factory".to_string(),
        base_denom: "uusd".to_string(),
        token_code_id: TOKEN_CODE_ID,
        protocol_fee_rate: Decimal::percent(1),
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    // update owner
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner0001".to_string()),
        oracle: None,
        collector: None,
        tswap_factory: None,
        token_code_id: Some(100u64),
        protocol_fee_rate: None,
        collateral_oracle: None,
        staking: None,
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());
    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!("owner0001", config.owner.as_str());
    assert_eq!(100u64, config.token_code_id);
    // Unauthorized err
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        oracle: None,
        collector: None,
        tswap_factory: None,
        token_code_id: None,
        protocol_fee_rate: None,
        collateral_oracle: None,
        staking: None,
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}
#[test]
fn register_asset() {
    let mut deps = mock_dependencies(&[]);
    let base_denom = "uusd".to_string();
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        oracle: "oracle0000".to_string(),
        collector: "collector0000".to_string(),
        collateral_oracle: "collateraloracle0000".to_string(),
        staking: "staking0000".to_string(),
        tswap_factory: "tswap_factory".to_string(),
        base_denom,
        token_code_id: TOKEN_CODE_ID,
        protocol_fee_rate: Decimal::percent(1),
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Decimal::percent(150),
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "collateraloracle0000".to_string(),
            funds: vec![],
            msg: to_binary(&RegisterCollateralAsset {
                asset: AssetInfo::Token {
                    contract_addr: "asset0000".to_string(),
                },
                multiplier: Decimal::one(),
                price_source: SourceType::MelangeOracle {
                    oracle_addr: "oracle0000".to_string(),
                },
            })
                .unwrap(),
        }))]
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::AssetConfig {
            asset_token: "asset0000".to_string(),
        },
    )
        .unwrap();
    let asset_config: AssetConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        asset_config,
        AssetConfigResponse {
            token: "asset0000".to_string(),
            min_collateral_ratio: Decimal::percent(150),
            end_price: None,
        }
    );
    // must be failed with the already registered token error
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Decimal::percent(150),
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "Asset was already registered"),
        _ => panic!("DO NOT ENTER HERE"),
    }
    // must be failed with unauthorized error
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Decimal::percent(150),
    };
    let info = mock_info("owner0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }
    // must be failed with unauthorized error
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Decimal::percent(50),
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "min_collateral_ratio must be bigger or equal than 1.1")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }
}
#[test]
fn update_asset() {
    let mut deps = mock_dependencies(&[]);
    let base_denom = "uusd".to_string();
    let msg = InstantiateMsg {
        owner: "owner0000".to_string(),
        oracle: "oracle0000".to_string(),
        collector: "collector0000".to_string(),
        collateral_oracle: "collateraloracle0000".to_string(),
        staking: "staking0000".to_string(),
        tswap_factory: "tswap_factory".to_string(),
        base_denom,
        token_code_id: TOKEN_CODE_ID,
        protocol_fee_rate: Decimal::percent(1),
    };
    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::RegisterAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Decimal::percent(150),
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg = ExecuteMsg::UpdateAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Some(Decimal::percent(200)),
    };
    let info = mock_info("owner0000", &[]);
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::AssetConfig {
            asset_token: "asset0000".to_string(),
        },
    )
        .unwrap();
    let asset_config: AssetConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        asset_config,
        AssetConfigResponse {
            token: "asset0000".to_string(),
            min_collateral_ratio: Decimal::percent(200),
            end_price: None,
        }
    );
    let msg = ExecuteMsg::UpdateAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Some(Decimal::percent(50)),
    };
    let info = mock_info("owner0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "min_collateral_ratio must be bigger or equal than 1.1")
        }
        _ => panic!("Must return unauthorized error"),
    }
    let msg = ExecuteMsg::UpdateAsset {
        asset_token: "asset0000".to_string(),
        min_collateral_ratio: Some(Decimal::percent(200)),
    };
    let info = mock_info("owner0001", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(msg, "unauthorized"),
        _ => panic!("Must return unauthorized error"),
    }
}
