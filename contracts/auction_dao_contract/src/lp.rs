use auction_dao::state::{Config, Global, UserAccount};
use cosmwasm_std::{
    BankMsg, Coin, CosmosMsg, Decimal, Deps, DepsMut, MessageInfo, Response, Uint128,
};
use injective_cosmwasm::{InjectiveMsgWrapper, InjectiveQueryWrapper};

use auction_dao::error::ContractError;

use crate::{
    auction::get_current_auction_value_using_exchange,
    state::{CONFIG, GLOBAL, USER_ACCOUNTS},
};

/*   Dynamic max_tokens based on current basket value
Biggest risk here is if the next auction has way less value and people dont withdraw
the total_commited injs will exced a lot the max preventivated */

pub fn get_max_tokens(
    deps: Deps<InjectiveQueryWrapper>,
    config: &Config,
) -> Result<Uint128, ContractError> {
    let basket_value = get_current_auction_value_using_exchange(deps)?;
    let max_tokens =
        basket_value.multiply_ratio(config.max_inj_offset_bps, Uint128::from(10000u128));

    return Ok(max_tokens);
}

pub fn deposit(
    deps: DepsMut<InjectiveQueryWrapper>,
    info: MessageInfo,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.funds.len() != 1 || info.funds[0].denom != config.accepted_denom {
        return Err(ContractError::InvalidDenom {});
    }

    let max_tokens = get_max_tokens(deps.as_ref(), &config)?;

    let amount = info.funds[0].amount;
    let mut global = GLOBAL.load(deps.storage)?;

    if global.total_supply + amount > max_tokens {
        return Err(ContractError::MaxTokensExceeded {});
    }

    let user_addr = info.sender.as_str();

    let mut user_account = USER_ACCOUNTS
        .may_load(deps.storage, user_addr)
        .unwrap()
        .unwrap_or_default();

    update_user_reward(&mut user_account, &global.index)?;

    increase_supply(&mut user_account, &mut global, &amount);

    USER_ACCOUNTS.save(deps.storage, user_addr, &user_account)?;
    GLOBAL.save(deps.storage, &global)?;

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("owner", user_addr)
        .add_attribute("amount", amount))
}

pub fn withdraw(
    deps: DepsMut<InjectiveQueryWrapper>,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    let user_addr = info.sender.as_str();

    let mut user_account = USER_ACCOUNTS
        .may_load(deps.storage, user_addr)
        .unwrap()
        .unwrap_or_default();

    if user_account.deposited < amount {
        return Err(ContractError::InsufficientFunds {});
    }

    let mut global = GLOBAL.load(deps.storage)?;

    update_user_reward(&mut user_account, &global.index)?;

    let config = CONFIG.load(deps.storage)?;

    let amount_with_reward = amount + user_account.pending_reward;
    let msgs: Vec<CosmosMsg<_>> = vec![CosmosMsg::Bank(BankMsg::Send {
        to_address: user_addr.to_string(),
        amount: vec![Coin {
            denom: config.accepted_denom.clone(),
            amount: amount_with_reward,
        }],
    })];

    user_account.pending_reward = Uint128::zero();

    decrease_supply(&mut user_account, &mut global, &amount);

    if user_account.deposited.is_zero() {
        USER_ACCOUNTS.remove(deps.storage, user_addr);
    } else {
        USER_ACCOUNTS.save(deps.storage, user_addr, &user_account)?;
    }

    GLOBAL.save(deps.storage, &global)?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("method", "withdraw")
        .add_attribute("owner", user_addr)
        .add_attribute("amount", amount)
        .add_attribute("rewards", amount_with_reward - amount))
}

pub fn harvest(
    deps: DepsMut<InjectiveQueryWrapper>,
    info: MessageInfo,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    let user_addr = info.sender.as_str();

    let mut user_account = USER_ACCOUNTS
        .may_load(deps.storage, user_addr)
        .unwrap()
        .unwrap_or_default();

    let global = GLOBAL.load(deps.storage)?;

    update_user_reward(&mut user_account, &global.index)?;

    let config = CONFIG.load(deps.storage)?;

    let mut msgs: Vec<CosmosMsg<_>> = vec![];
    let reward = user_account.pending_reward;
    if reward > Uint128::zero() {
        msgs.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: user_addr.to_string(),
            amount: vec![Coin {
                denom: config.accepted_denom.clone(),
                amount: reward,
            }],
        }));
    }

    user_account.pending_reward = Uint128::zero();

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("method", "harvest")
        .add_attribute("owner", user_addr)
        .add_attribute("amount", reward))
}

pub fn update_global_index(global: &mut Global) {
    if global.total_supply.is_zero() {
        return;
    }

    global.index = Decimal::from_ratio(global.profit_to_distribute, global.total_supply);
    global.accumulated_profit += global.profit_to_distribute;
    global.profit_to_distribute = Uint128::zero();
}

pub fn update_user_reward(
    user_account: &mut UserAccount,
    global_index: &Decimal,
) -> Result<(), ContractError> {
    let reward = Decimal::from_atomics(user_account.deposited.u128(), 0)?
        * (global_index - user_account.index);

    user_account.index = *global_index;
    user_account.pending_reward += reward.to_uint_floor();

    Ok(())
}

fn increase_supply(user_account: &mut UserAccount, global: &mut Global, amount: &Uint128) {
    user_account.deposited += amount;
    global.total_supply += amount;
}

fn decrease_supply(user_account: &mut UserAccount, global: &mut Global, amount: &Uint128) {
    user_account.deposited -= amount;
    global.total_supply -= amount;
}
