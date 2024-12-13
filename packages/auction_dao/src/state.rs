use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal256, Uint128};
use injective_cosmwasm::{MarketId, SubaccountId};
use injective_std::types::cosmos::base::v1beta1::Coin;

#[cw_serde]
pub struct Config {
    pub accepted_denom: String,
    pub swap_router: Addr,
    pub admin: Addr,
    pub bid_time_buffer_secs: u64,
    pub withdraw_time_buffer_secs: u64,
    pub max_inj_offset_bps: Uint128,
    pub winning_bidder_reward_bps: Uint128,
    pub contract_subaccount_id: SubaccountId,
}

#[cw_serde]
pub struct UserAccount {
    pub deposited: Uint128,
    pub index: Decimal256,
    pub pending_reward: Uint128,
}

impl Default for UserAccount {
    fn default() -> Self {
        UserAccount {
            deposited: Uint128::zero(),
            index: Decimal256::zero(),
            pending_reward: Uint128::zero(),
        }
    }
}

#[cw_serde]
pub struct Global {
    pub index: Decimal256,
    // profit before updated index
    pub profit_to_distribute: Uint128,
    // sum of the profit already distributed
    pub accumulated_profit: Uint128,
    pub total_supply: Uint128,
}

impl Default for Global {
    fn default() -> Self {
        Global {
            index: Decimal256::zero(),
            profit_to_distribute: Uint128::zero(),
            accumulated_profit: Uint128::zero(),
            total_supply: Uint128::zero(),
        }
    }
}

#[cw_serde]
pub struct SwapRoute {
    pub market_id: MarketId,
    pub source_denom: String,
    pub target_denom: String,
}

#[cw_serde]
pub struct BidAttempt {
    pub amount: Uint128,
    pub submitted_by: Addr,
    pub round: u64,
    pub basket: Vec<Coin>,
}

impl BidAttempt {
    pub fn manual_swap(contract_addr: Addr) -> Self {
        let mut bid_attempt = BidAttempt::default();
        bid_attempt.submitted_by = contract_addr;

        return bid_attempt;
    }
}

impl Default for BidAttempt {
    fn default() -> Self {
        BidAttempt {
            amount: Uint128::zero(),
            submitted_by: Addr::unchecked(""),
            round: 0,
            basket: vec![],
        }
    }
}

#[cw_serde]
pub enum SellType {
    Base,
    Quote,
}

#[cw_serde]
pub struct SellAssetPayload {
    pub coin: Coin,
    pub sell_type: SellType,
}
