use cosmwasm_std::{BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, Uint128};
use injective_cosmwasm::{InjectiveMsgWrapper, InjectiveQueryWrapper};

use auction_dao::error::ContractError;
use auction_dao::msg::CallbackMsg;

use crate::lp::update_global_index;
use crate::state::{CONFIG, GLOBAL, SETTLED_AMOUNT_TRANSIENT};

pub fn callback(
    deps: DepsMut<InjectiveQueryWrapper>,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    return match msg {
        auction_dao::msg::CallbackMsg::BidSettledSuccess { bid_attempt } => {
            let bid_amount = bid_attempt.amount;
            let received_from_basket_sell = SETTLED_AMOUNT_TRANSIENT.load(deps.as_ref().storage)?;
            SETTLED_AMOUNT_TRANSIENT.remove(deps.storage);

            let profit = received_from_basket_sell.saturating_sub(bid_amount);
            let config = CONFIG.load(deps.storage)?;

            let mut winning_reward =
                profit.multiply_ratio(config.winning_bidder_reward_bps, Uint128::new(10000));
            let mut dao_profit = profit - winning_reward;

            if bid_attempt.submitted_by == env.contract.address {
                dao_profit = profit;
                winning_reward = Uint128::zero();
            }

            let mut response = Response::new()
                .add_attribute("bid_amount", bid_amount.to_string())
                .add_attribute(
                    "received_from_basket_sell",
                    received_from_basket_sell.to_string(),
                )
                .add_attribute("dao_profit", profit.to_string())
                .add_attribute("reward", winning_reward.to_string());

            if winning_reward > Uint128::zero() {
                response = response.add_message(CosmosMsg::Bank(BankMsg::Send {
                    to_address: bid_attempt.submitted_by.to_string(),
                    amount: vec![Coin {
                        denom: config.accepted_denom,
                        amount: winning_reward,
                    }],
                }));
            }

            let mut global = GLOBAL.load(deps.storage)?;
            global.profit_to_distribute += dao_profit;
            update_global_index(&mut global);
            GLOBAL.save(deps.storage, &global)?;

            Ok(response)
        }
    };
}
