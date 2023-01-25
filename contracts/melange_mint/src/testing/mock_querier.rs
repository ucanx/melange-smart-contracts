use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{from_binary, from_slice, to_binary, Coin, ContractResult, Decimal, OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery, Empty};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use melange_protocol::collateral_oracle::CollateralPriceResponse;
use terraswap::{asset::AssetInfo, asset::PairInfo};
use sei_cosmwasm::SeiQueryWrapper;

use std::marker::PhantomData;

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier, SeiQueryWrapper> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        api: MockApi::default(),
        storage: MockStorage::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<>,
    tax_querier: TaxQuerier,
    oracle_price_querier: OraclePriceQuerier,
    collateral_oracle_querier: CollateralOracleQuerier,
    tswap_pair_querier: TswapPairQuerier,
}

#[derive(Clone, Default)]
pub struct TaxQuerier {
    rate: Decimal,
    // this lets us iterate over all pairs that match the first string
    caps: HashMap<String, Uint128>,
}

impl TaxQuerier {
    pub fn new(rate: Decimal, caps: &[(&String, &Uint128)]) -> Self {
        TaxQuerier {
            rate,
            caps: caps_to_map(caps),
        }
    }
}

pub(crate) fn caps_to_map(caps: &[(&String, &Uint128)]) -> HashMap<String, Uint128> {
    let mut owner_map: HashMap<String, Uint128> = HashMap::new();
    for (denom, cap) in caps.iter() {
        owner_map.insert(denom.to_string(), **cap);
    }
    owner_map
}

#[derive(Clone, Default)]
pub struct OraclePriceQuerier {
    // this lets us iterate over all pairs that match the first string
    oracle_price: HashMap<String, Decimal>,
}

impl OraclePriceQuerier {
    pub fn new(oracle_price: &[(&String, &Decimal)]) -> Self {
        OraclePriceQuerier {
            oracle_price: oracle_price_to_map(oracle_price),
        }
    }
}

pub(crate) fn oracle_price_to_map(
    oracle_price: &[(&String, &Decimal)],
) -> HashMap<String, Decimal> {
    let mut oracle_price_map: HashMap<String, Decimal> = HashMap::new();
    for (base_quote, oracle_price) in oracle_price.iter() {
        oracle_price_map.insert((*base_quote).clone(), **oracle_price);
    }

    oracle_price_map
}

#[derive(Clone, Default)]
pub struct CollateralOracleQuerier {
    // this lets us iterate over all pairs that match the first string
    collateral_infos: HashMap<String, (Decimal, Decimal, bool)>,
}

impl CollateralOracleQuerier {
    pub fn new(collateral_infos: &[(&String, &Decimal, &Decimal, &bool)]) -> Self {
        CollateralOracleQuerier {
            collateral_infos: collateral_infos_to_map(collateral_infos),
        }
    }
}

pub(crate) fn collateral_infos_to_map(
    collateral_infos: &[(&String, &Decimal, &Decimal, &bool)],
) -> HashMap<String, (Decimal, Decimal, bool)> {
    let mut collateral_infos_map: HashMap<String, (Decimal, Decimal, bool)> = HashMap::new();
    for (collateral, collateral_price, collateral_multiplier, is_revoked) in collateral_infos.iter()
    {
        collateral_infos_map.insert(
            (*collateral).clone(),
            (**collateral_price, **collateral_multiplier, **is_revoked),
        );
    }

    collateral_infos_map
}

#[derive(Clone, Default)]
pub struct TswapPairQuerier {
    // this lets us iterate over all pairs that match the first string
    pairs: HashMap<String, String>,
}

impl TswapPairQuerier {
    pub fn new(pairs: &[(&String, &String, &String)]) -> Self {
        TswapPairQuerier {
            pairs: paris_to_map(pairs),
        }
    }
}

pub(crate) fn paris_to_map(pairs: &[(&String, &String, &String)]) -> HashMap<String, String> {
    let mut pairs_map: HashMap<String, String> = HashMap::new();
    for (asset1, asset2, pair) in pairs.iter() {
        pairs_map.insert((asset1.to_string() + asset2).clone(), pair.to_string());
    }

    pairs_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {:?}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TerraQueryWrapper {
    pub route: TerraRoute,
    pub query_data: TerraQuery,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerraRoute {
    Market,
    Treasury,
    Oracle,
    Wasm,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerraQuery {
    Swap {
        offer_coin: Coin,
        ask_denom: String,
    },
    TaxRate {},
    TaxCap {
        denom: String,
    },
    ExchangeRates {
        base_denom: String,
        quote_denoms: Vec<String>,
    },
    ContractInfo {
        contract_address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceResponse {
    pub rate: Decimal,
    pub last_updated: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MockQueryMsg {
    Price {
        asset_token: String,
        timeframe: Option<u64>,
    },
    CollateralPrice {
        asset: String,
    },
    Pair {
        asset_infos: [AssetInfo; 2],
    },
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart {
                                   contract_addr: _,
                                   msg,
                               }) => match from_binary(&msg).unwrap() {
                MockQueryMsg::Price {
                    asset_token,
                    timeframe: _,
                } => match self.oracle_price_querier.oracle_price.get(&asset_token) {
                    Some(base_price) => {
                        SystemResult::Ok(ContractResult::from(to_binary(&PriceResponse {
                            rate: *base_price,
                            last_updated: 1000u64,
                        })))
                    }
                    None => SystemResult::Err(SystemError::InvalidRequest {
                        error: "No oracle price exists".to_string(),
                        request: msg.as_slice().into(),
                    }),
                },
                MockQueryMsg::CollateralPrice { asset } => {
                    match self.collateral_oracle_querier.collateral_infos.get(&asset) {
                        Some(collateral_info) => SystemResult::Ok(ContractResult::from(to_binary(
                            &CollateralPriceResponse {
                                asset,
                                rate: collateral_info.0,
                                last_updated: 1000u64,
                                multiplier: collateral_info.1,
                                is_revoked: collateral_info.2,
                            },
                        ))),
                        None => SystemResult::Err(SystemError::InvalidRequest {
                            error: "Collateral info does not exist".to_string(),
                            request: msg.as_slice().into(),
                        }),
                    }
                }
                MockQueryMsg::Pair { asset_infos } => {
                    match self
                        .tswap_pair_querier
                        .pairs
                        .get(&(asset_infos[0].to_string() + &asset_infos[1].to_string()))
                    {
                        Some(pair) => {
                            SystemResult::Ok(ContractResult::from(to_binary(&PairInfo {
                                asset_infos,
                                contract_addr: pair.to_string(),
                                liquidity_token: "liquidity".to_string(),
                                asset_decimals: [2u8, 2u8],
                            })))
                        }
                        None => SystemResult::Err(SystemError::InvalidRequest {
                            error: "No pair exists".to_string(),
                            request: msg.as_slice().into(),
                        }),
                    }
                }
            },
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier) -> Self {
        WasmMockQuerier {
            base,
            tax_querier: TaxQuerier::default(),
            oracle_price_querier: OraclePriceQuerier::default(),
            collateral_oracle_querier: CollateralOracleQuerier::default(),
            tswap_pair_querier: TswapPairQuerier::default(),
        }
    }

    // configure the token owner mock querier
    pub fn with_tax(&mut self, rate: Decimal, caps: &[(&String, &Uint128)]) {
        self.tax_querier = TaxQuerier::new(rate, caps);
    }

    // configure the oracle price mock querier
    pub fn with_oracle_price(&mut self, oracle_price: &[(&String, &Decimal)]) {
        self.oracle_price_querier = OraclePriceQuerier::new(oracle_price);
    }

    // configure the collateral oracle mock querier
    pub fn with_collateral_infos(
        &mut self,
        collateral_infos: &[(&String, &Decimal, &Decimal, &bool)],
    ) {
        self.collateral_oracle_querier = CollateralOracleQuerier::new(collateral_infos);
    }

    // configure the tswap factory pair mock querier
    pub fn with_tswap_pair(&mut self, pairs: &[(&String, &String, &String)]) {
        self.tswap_pair_querier = TswapPairQuerier::new(pairs);
    }
}
