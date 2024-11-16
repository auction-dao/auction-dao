use crate::{
    exchange::swap,
    state::{read_swap_route, remove_swap_route, store_swap_route, CONFIG},
};

use auction_dao::error::ContractError;
use auction_dao::{msg::SELL_ASSET_SUCCESS_REPLY_ID, state::SwapRoute};

use auction_dao::msg::InstantiateMsg;

use cosmwasm_std::{ensure, ensure_eq, Addr, Deps, DepsMut, Env, Response, SubMsg, Uint128};
use injective_cosmwasm::{InjectiveMsgWrapper, InjectiveQuerier, InjectiveQueryWrapper, MarketId};

pub fn verify_sender_is_admin(
    deps: Deps<InjectiveQueryWrapper>,
    sender: &Addr,
) -> Result<(), ContractError> {
    let config = CONFIG.load(deps.storage)?;
    ensure_eq!(&config.admin, sender, ContractError::Unauthorized {});
    Ok(())
}

pub fn update_config(
    deps: DepsMut<InjectiveQueryWrapper>,
    sender: &Addr,
    new_config: InstantiateMsg,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    verify_sender_is_admin(deps.as_ref(), &sender)?;

    CONFIG.update(deps.storage, |mut c| -> Result<_, ContractError> {
        c.accepted_denom = new_config.accepted_denom;
        c.swap_router = deps.api.addr_validate(&new_config.swap_router)?;
        c.admin = deps.api.addr_validate(&new_config.admin)?;
        c.time_buffer = new_config.time_buffer;
        c.winning_bidder_reward_bps = new_config.winning_bidder_reward_bps;
        c.max_inj_offset_bps = new_config.max_inj_offset_bps;

        Ok(c)
    })?;

    Ok(Response::new().add_attribute("method", "update_config"))
}

pub fn set_route(
    deps: DepsMut<InjectiveQueryWrapper>,
    sender: &Addr,
    source_denom: String,
    target_denom: String,
    market_id_s: String,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    verify_sender_is_admin(deps.as_ref(), sender)?;

    if source_denom == target_denom {
        return Err(ContractError::CustomError {
            val: "Cannot set a route with the same denom being source and target".to_string(),
        });
    }

    let market_id = MarketId::new(market_id_s).map_err(|_| ContractError::CustomError {
        val: "Invalid market_id".to_string(),
    })?;

    let route_already_exist =
        read_swap_route(deps.as_ref(), source_denom.as_str(), target_denom.as_str());

    if route_already_exist.is_ok() {
        return Err(ContractError::CustomError {
            val: "Route already exist".to_string(),
        });
    }

    let route = SwapRoute {
        market_id,
        source_denom,
        target_denom,
    };

    verify_route_exists(deps.as_ref(), &route)?;
    store_swap_route(deps.storage, &route)?;

    Ok(Response::new().add_attribute("method", "set_route"))
}

fn verify_route_exists(
    deps: Deps<InjectiveQueryWrapper>,
    route: &SwapRoute,
) -> Result<(), ContractError> {
    let querier = InjectiveQuerier::new(&deps.querier);

    let market = querier
        .query_spot_market(&route.market_id.clone())?
        .market
        .ok_or(ContractError::CustomError {
            val: format!("Market {} not found", &route.market_id.as_str()).to_string(),
        })?;

    ensure!(
        &market.quote_denom == &route.source_denom || &market.base_denom == &route.source_denom,
        ContractError::CustomError {
            val: "Source denom not found".to_string()
        }
    );
    ensure!(
        &market.quote_denom == &route.target_denom || &market.base_denom == &route.target_denom,
        ContractError::CustomError {
            val: "Target denom not found".to_string()
        }
    );

    Ok(())
}

pub fn delete_route(
    deps: DepsMut<InjectiveQueryWrapper>,
    sender: &Addr,
    source_denom: String,
    target_denom: String,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    verify_sender_is_admin(deps.as_ref(), sender)?;
    remove_swap_route(deps, &source_denom, &target_denom);

    Ok(Response::new()
        .add_attribute("method", "delete_route")
        .add_attribute("source_denom", source_denom)
        .add_attribute("target_denom", target_denom))
}

pub fn manual_swap(
    deps: DepsMut<InjectiveQueryWrapper>,
    env: Env,
    sender: &Addr,
    amount: Uint128,
    market_id: &str,
    asset: &str,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    verify_sender_is_admin(deps.as_ref(), sender)?;

    let config = CONFIG.load(deps.storage)?;

    if asset == config.accepted_denom {
        return Err(ContractError::CannotManuallySwap {});
    }

    let msg = swap(
        deps.as_ref(),
        &env.contract.address,
        amount,
        market_id,
        asset,
    )?;

    let submsg = SubMsg::reply_on_success(msg, SELL_ASSET_SUCCESS_REPLY_ID);

    Ok(Response::new()
        .add_submessage(submsg)
        .add_attribute("method", "manual_swap")
        .add_attribute("asset", asset)
        .add_attribute("amount", amount)
        .add_attribute("market_id", market_id))
}
