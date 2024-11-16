use crate::{
    auction::{
        get_current_auction, get_current_auction_value_using_exchange,
        get_current_auction_value_using_router,
    },
    exchange::simulate,
    lp::{get_max_tokens, update_user_reward},
    state::{CONFIG, GLOBAL, USER_ACCOUNTS},
};
use auction_dao::error::ContractError;
use cosmwasm_std::{to_json_binary, Binary, Deps, Uint128};
use injective_cosmwasm::InjectiveQueryWrapper;

pub fn query_current_auction_basket(
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<Binary, ContractError> {
    let current_auction_round_response = get_current_auction(deps)?;

    Ok(to_json_binary(&current_auction_round_response)?)
}

pub fn query_user(
    deps: Deps<InjectiveQueryWrapper>,
    address: String,
) -> Result<Binary, ContractError> {
    let mut user_account = USER_ACCOUNTS.load(deps.storage, &address)?;
    let global = GLOBAL.load(deps.storage)?;

    update_user_reward(&mut user_account, &global.index)?;

    Ok(to_json_binary(&user_account)?)
}

pub fn query_current_auction_value_using_router(
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<Binary, ContractError> {
    let current_auction_round_response = get_current_auction_value_using_router(deps)?;

    Ok(to_json_binary(&current_auction_round_response)?)
}

pub fn query_simulation_using_exchange(
    deps: Deps<InjectiveQueryWrapper>,
    amount: Uint128,
    market_id: &str,
    asset: &str,
) -> Result<Binary, ContractError> {
    let (quantity, _) = simulate(deps, amount, market_id, asset)?;

    Ok(to_json_binary(&quantity)?)
}

pub fn query_current_auction_value_using_exchange(
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<Binary, ContractError> {
    let total_value = get_current_auction_value_using_exchange(deps)?;

    Ok(to_json_binary(&total_value)?)
}

pub fn query_max_tokens(deps: Deps<InjectiveQueryWrapper>) -> Result<Binary, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let max_tokens = get_max_tokens(deps, &config)?;

    Ok(to_json_binary(&max_tokens)?)
}
