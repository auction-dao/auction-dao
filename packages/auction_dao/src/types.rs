use cosmwasm_schema::cw_serde;

#[cw_serde]
pub struct InstantiateMitoRouterMsg {
    pub admin: String,
    pub fee_recipient: FeeRecipient,
}

#[cw_serde]
pub struct InstantiateHelixRouterMsg {
    pub admin: String,
    pub fee_recipient: String,
}

#[cw_serde]
pub struct FeeRecipient {
    pub address: String,
}


#[cw_serde]
pub struct RouterSwap {
    pub swap: SwapDetails,
}

#[cw_serde]
pub struct SwapDetails {
    pub market_id: String,
}

#[cw_serde]
pub struct ExpectedFee {
    pub amount: String,
    pub denom: String,
}

#[cw_serde]
pub struct SetRouteMsg {
    pub source_denom: String,
    pub target_denom: String,
    pub market_id: String,
}

#[cw_serde]
pub struct RouterSimulationQuerry {
    pub simulation: RouterSimulation,
}

#[cw_serde]
pub struct RouterSimulationQuerryResponse {
    pub return_amount: String,
    pub commission_amount: String,
}

impl Default for RouterSimulationQuerryResponse {
    fn default() -> Self {
        RouterSimulationQuerryResponse {
            return_amount: "0".to_string(),
            commission_amount: "0".to_string(),
        }
    }
}

#[cw_serde]
pub struct RouterSimulation {
    pub market_id: String,
    pub offer_asset: OfferAsset,
}

#[cw_serde]
pub struct OfferAsset {
    pub info: AssetInfo,
    pub amount: String,
}

#[cw_serde]
pub enum AssetInfo {
    NativeToken { denom: String },
    Token { contract_addr: String },
}

#[cw_serde]
pub enum BidResult {
    Win,
    Loss,
}

impl Into<String> for BidResult {
    fn into(self) -> String {
        match self {
            BidResult::Win => "win".to_string(),
            BidResult::Loss => "loss".to_string(),
        }
    }
}
