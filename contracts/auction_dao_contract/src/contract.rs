use crate::admins::{delete_route, manual_swap, set_route};
use crate::auction::{self};
use crate::lp::{deposit, harvest, withdraw};
use crate::state::{BID_ATTEMPT, BID_ATTEMPT_TRANSIENT, CONFIG, GLOBAL, SETTLED_AMOUNT_TRANSIENT};
use crate::{admins, callback::callback, queries};
use auction_dao::error::ContractError;
use auction_dao::msg::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SELL_ASSET_SUCCESS_REPLY_ID,
    TRY_BID_SUCCESS_REPLY_ID,
};
use auction_dao::state::{Config, Global};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, Uint128,
};
use cosmwasm_std::{from_json, Decimal256, Uint256};
use cw2::{get_contract_version, set_contract_version};
use injective_cosmwasm::{
    get_default_subaccount_id_for_checked_address, InjectiveMsgWrapper, InjectiveQueryWrapper,
};
use injective_std::types::cosmos::base::v1beta1::Coin;
use injective_std::types::injective::exchange::v1beta1 as Exchange;
use prost::Message;
use std::str::FromStr;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:auction_dao";
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
            bid_time_buffer_secs: msg.bid_time_buffer,
            withdraw_time_buffer_secs: msg.withdraw_time_buffer,
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
        ExecuteMsg::Callback(msg) => callback(deps, env, info, msg),
        ExecuteMsg::Deposit {} => deposit(deps, info),
        ExecuteMsg::Harvest {} => harvest(deps, info),
        ExecuteMsg::ManualExchangeSwap {
            amount,
            market_id,
            asset,
        } => manual_swap(deps, env, &info.sender, amount, &market_id, &asset),
        ExecuteMsg::Withdraw { amount } => withdraw(deps, env, info, amount),
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
        QueryMsg::Config {} => {
            let config = CONFIG.load(deps.storage)?;
            Ok(to_json_binary(&config)?)
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

            SETTLED_AMOUNT_TRANSIENT
                .update(deps.storage, |amount| -> Result<_, ContractError> {
                    Ok(amount + quantity_128)
                })?;

            let mut response = Response::new();

            if !msg.payload.is_empty() {
                let asset = from_json::<Coin>(msg.payload)?;
                response = response.add_attribute(
                    format!("received_inj::{}", asset.denom),
                    quantity_128.to_string(),
                );
            }

            return Ok(response);
        }
        _ => Err(ContractError::InvalidReply(msg.id)),
    }
}

#[entry_point]
pub fn migrate(
    deps: DepsMut<InjectiveQueryWrapper>,
    _env: Env,
    _msg: MigrateMsg,
) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;

    match contract_version.contract.as_ref() {
        CONTRACT_NAME => match contract_version.version.as_ref() {
            "0.1.0" => {
                set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
            }
            _ => return Err(ContractError::MigrationError {}),
        },
        _ => return Err(ContractError::MigrationError {}),
    }

    Ok(Response::new()
        .add_attribute("previous_contract_name", &contract_version.contract)
        .add_attribute("previous_contract_version", &contract_version.version)
        .add_attribute("new_contract_name", format!("crates.io:{CONTRACT_NAME}"))
        .add_attribute("new_contract_version", CONTRACT_VERSION))
}
