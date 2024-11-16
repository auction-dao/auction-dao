use std::str::FromStr;

use auction_dao::state::{Config, Global};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
};
use cosmwasm_std::{Decimal256, Uint256};
use cw2::set_contract_version;

use injective_cosmwasm::{
    get_default_subaccount_id_for_checked_address, InjectiveMsgWrapper, InjectiveQueryWrapper,
};

use auction_dao::error::ContractError;
use auction_dao::msg::{
    ExecuteMsg, InstantiateMsg, QueryMsg, SELL_ASSET_SUCCESS_REPLY_ID, TRY_BID_SUCCESS_REPLY_ID,
};
use prost::Message;

use crate::admins::{delete_route, manual_swap, set_route};
use crate::auction::{self};
use crate::lp::{deposit, harvest, update_global_index, withdraw};
use crate::state::{BID_ATTEMPT, BID_ATTEMPT_TRANSIENT, CONFIG, GLOBAL};
use crate::{admins, queries};
use injective_std::types::injective::exchange::v1beta1 as Exchange;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:auction_dao:auction_dao";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(&msg.admin))?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    GLOBAL.save(deps.storage, &Global::default())?;
    CONFIG.save(
        deps.storage,
        &Config {
            accepted_denom: msg.accepted_denom,
            swap_router: deps.api.addr_validate(&msg.swap_router)?,
            admin: deps.api.addr_validate(&msg.admin)?,
            time_buffer: msg.time_buffer,
            max_inj_offset_bps: msg.max_inj_offset_bps,
            contract_subaccount_id: get_default_subaccount_id_for_checked_address(
                &env.contract.address,
            ),
            winning_bidder_reward_bps: msg.winning_bidder_reward_bps,
        },
    )?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", &msg.admin)
        .add_attribute("router", &msg.swap_router))
}

#[entry_point]
pub fn execute(
    deps: DepsMut<InjectiveQueryWrapper>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => deposit(deps, info),
        ExecuteMsg::Harvest {} => harvest(deps, info),
        ExecuteMsg::ManualExchangeSwap {
            amount,
            market_id,
            asset,
        } => manual_swap(deps, env, &info.sender, amount, &market_id, &asset),
        ExecuteMsg::Withdraw { amount } => withdraw(deps, info, amount),
        ExecuteMsg::TryBid { round } => auction::try_bid(deps, env, info, round),
        ExecuteMsg::TrySettle {} => auction::try_settle(deps, env, &info.sender),
        ExecuteMsg::UpdateConfig { new_config } => {
            admins::update_config(deps, &info.sender, new_config)
        }
        ExecuteMsg::SetRoute {
            source_denom,
            target_denom,
            market_id,
        } => set_route(deps, &info.sender, source_denom, target_denom, market_id),
        ExecuteMsg::DeleteRoute {
            source_denom,
            target_denom,
        } => delete_route(deps, &info.sender, source_denom, target_denom),
    }
}

#[entry_point]
pub fn query(
    deps: Deps<InjectiveQueryWrapper>,
    _env: Env,
    msg: QueryMsg,
) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::State {} => {
            let global = GLOBAL.load(deps.storage)?;
            Ok(to_json_binary(&global)?)
        }
        QueryMsg::User { address } => queries::query_user(deps, address),
        QueryMsg::CurrentAuctionBasket {} => queries::query_current_auction_basket(deps),
        QueryMsg::ExchangeSimulateSwap {
            amount,
            market_id,
            asset,
        } => queries::query_simulation_using_exchange(deps, amount, &market_id, &asset),
        QueryMsg::ExchangeCurrentAuctionValue {} => {
            queries::query_current_auction_value_using_exchange(deps)
        }
        QueryMsg::RouterCurrentAuctionValue {} => {
            queries::query_current_auction_value_using_router(deps)
        }
        QueryMsg::MaxAllowedTokensToDeposit {} => queries::query_max_tokens(deps),
    }
}

#[entry_point]
pub fn reply(
    deps: DepsMut<InjectiveQueryWrapper>,
    _env: Env,
    msg: cosmwasm_std::Reply,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    match msg.id {
        TRY_BID_SUCCESS_REPLY_ID => {
            // MsgBidResponse is just empty struct, no reason to decode
            let bid_attempt_cache = BID_ATTEMPT_TRANSIENT.load(deps.storage)?;
            BID_ATTEMPT.save(deps.storage, &bid_attempt_cache)?;

            return Ok(Response::new());
        }
        SELL_ASSET_SUCCESS_REPLY_ID => {
            let binding = msg
                .result
                .into_result()
                .map_err(ContractError::SubMsgFailure)?;

            let first_messsage = binding.msg_responses.first();

            let order_response = Exchange::MsgCreateSpotMarketOrderResponse::decode(
                first_messsage
                    .ok_or_else(|| {
                        ContractError::SubMsgFailure("No message responses found".to_string())
                    })?
                    .value
                    .as_slice(),
            )
            .map_err(|err| ContractError::ReplyParseFailure {
                id: msg.id,
                err: err.to_string(),
            })?;

            let trade_data = order_response
                .results
                .ok_or_else(|| ContractError::SubMsgFailure("No trade data".to_owned()))
                .unwrap();

            let quantity = Decimal256::from_atomics(Uint256::from_str(&trade_data.quantity)?, 18)?
                .to_uint_floor();

            let quantity_128 = Uint128::from_str(quantity.to_string().as_str())?;

            // deps.api.debug(&format!("Trade data: {:?}", trade_data));

            let mut global = GLOBAL.load(deps.storage)?;
            global.profit_to_distribute += quantity_128;
            update_global_index(&mut global);
            GLOBAL.save(deps.storage, &global)?;

            return Ok(Response::new());
        }
        _ => Err(ContractError::InvalidReply(msg.id)),
    }
}
