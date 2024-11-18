use cosmwasm_schema::{cw_serde, QueryResponses};

#[allow(unused_imports)]
use crate::state::{Global, UserAccount};
use cosmwasm_std::Uint128;
#[allow(unused_imports)]
use injective_std::types::injective::auction::v1beta1::QueryCurrentAuctionBasketResponse;

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub accepted_denom: String,
    pub swap_router: String,
    pub bid_time_buffer: u64,
    pub withdraw_time_buffer: u64,
    pub winning_bidder_reward_bps: Uint128,
    pub max_inj_offset_bps: Uint128,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {},
    Harvest {},
    Withdraw {
        amount: Uint128,
    },
    TryBid {
        round: u64,
    },
    TrySettle {},
    UpdateConfig {
        new_config: InstantiateMsg,
    },
    SetRoute {
        source_denom: String,
        target_denom: String,
        market_id: String,
    },
    DeleteRoute {
        source_denom: String,
        target_denom: String,
    },
    ManualExchangeSwap {
        amount: Uint128,
        market_id: String,
        asset: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Global)]
    State {},
    #[returns(UserAccount)]
    User { address: String },
    #[returns(QueryCurrentAuctionBasketResponse)]
    CurrentAuctionBasket {},
    #[returns(Uint128)]
    ExchangeSimulateSwap {
        amount: Uint128,
        market_id: String,
        asset: String,
    },
    #[returns(Uint128)]
    ExchangeCurrentAuctionValue {},
    #[returns(Uint128)]
    RouterCurrentAuctionValue {},
    #[returns(Uint128)]
    MaxAllowedTokensToDeposit {},
}

#[cw_serde]
pub struct MigrateMsg {}

pub const TRY_BID_SUCCESS_REPLY_ID: u64 = 1;
pub const SELL_ASSET_SUCCESS_REPLY_ID: u64 = 2;
