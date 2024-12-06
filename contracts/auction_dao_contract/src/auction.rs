use crate::exchange::{simulate, swap};
use crate::router::get_inj_value_asset;
use crate::state::{
    read_swap_route, BID_ATTEMPT, BID_ATTEMPT_TRANSIENT, CONFIG, SETTLED_AMOUNT_TRANSIENT,
};
use auction_dao::msg::{ExecuteMsg, SELL_ASSET_SUCCESS_REPLY_ID, TRY_BID_SUCCESS_REPLY_ID};
use auction_dao::state::BidAttempt;

use auction_dao::{error::ContractError, types::BidResult};
use cosmwasm_std::{
    to_json_binary, Addr, CosmosMsg, Decimal256, Deps, DepsMut, Env, Event, MessageInfo,
    QueryRequest, Response, StdResult, SubMsg, Uint128, Uint256, WasmMsg,
};
use injective_cosmwasm::{InjectiveMsgWrapper, InjectiveQueryWrapper};
use injective_std::types::injective::auction::v1beta1::{
    LastAuctionResult, Params as AuctionParams, QueryAuctionParamsRequest,
    QueryAuctionParamsResponse, QueryCurrentAuctionBasketResponse, QueryLastAuctionResultRequest,
    QueryLastAuctionResultResponse,
};
use injective_std::types::{
    cosmos::base::v1beta1::Coin as ProstCoin, injective::auction::v1beta1::MsgBid,
};
use prost::Message;
use std::str::FromStr;

pub(crate) fn get_current_auction(
    deps: Deps<InjectiveQueryWrapper>,
) -> StdResult<QueryCurrentAuctionBasketResponse> {
    //QueryRequest::Stargate is deprecated, but GrpcQuerry not working in test env with test-tube
    #[allow(deprecated)]
    let current_auction_basket_response: QueryCurrentAuctionBasketResponse =
        deps.querier.query(&QueryRequest::Stargate {
            path: "/injective.auction.v1beta1.Query/CurrentAuctionBasket".to_string(),
            data: [].into(),
        })?;
    /*
        let current_auction_basket_response: QueryCurrentAuctionBasketResponse = deps.querier.query(
        &QueryRequest::Grpc(GrpcQuery {
            path: "/injective.auction.v1beta1.Query/CurrentAuctionBasket".to_string(),
            data: Binary::default(), // Use Binary::default() if no data is required
        })
    )?; */

    Ok(current_auction_basket_response)
}

pub(crate) fn get_last_auction_result(
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<LastAuctionResult, ContractError> {
    #[allow(deprecated)]
    let last_auction_result_response: QueryLastAuctionResultResponse =
        deps.querier.query(&QueryRequest::Stargate {
            path: "/injective.auction.v1beta1.Query/LastAuctionResult".to_string(),
            data: QueryLastAuctionResultRequest {}.into(),
        })?;

    if let Some(last_auction) = last_auction_result_response.last_auction_result {
        return Ok(last_auction);
    } else {
        return Err(ContractError::LastAuctionResultNotFound {});
    }
}

pub(crate) fn get_auction_params(
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<AuctionParams, ContractError> {
    #[allow(deprecated)]
    let auction_params: QueryAuctionParamsResponse =
        deps.querier.query(&QueryRequest::Stargate {
            path: "/injective.auction.v1beta1.Query/AuctionParams".to_string(),
            data: QueryAuctionParamsRequest {}.into(),
        })?;

    if let Some(params) = auction_params.params {
        return Ok(params);
    } else {
        return Err(ContractError::AuctionParamsNotFound {});
    }
}

pub(crate) fn get_current_auction_value_using_router(
    deps: Deps<InjectiveQueryWrapper>,
) -> StdResult<Uint128> {
    let current_auction_basket_response = get_current_auction(deps)?;

    let basket_assets = current_auction_basket_response.amount;

    let mut total_inj_value = Uint128::new(0);

    for asset in basket_assets.iter() {
        let asset_inj_value = get_inj_value_asset(
            deps,
            asset.denom.clone(),
            "inj".to_string(),
            asset.amount.clone(),
        )?;
        total_inj_value = total_inj_value.checked_add(asset_inj_value)?;
    }

    Ok(total_inj_value)
}

pub fn get_current_auction_value_using_exchange(
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<Uint128, ContractError> {
    let current_auction_basket_response = get_current_auction(deps)?;
    let basket_assets = current_auction_basket_response.amount;

    let mut total_inj_value = Uint128::new(0);

    for asset in basket_assets.iter() {
        if asset.denom == "inj" {
            total_inj_value += Uint128::from_str(&asset.amount.as_str())?;
        }

        let market_id = match read_swap_route(deps, &asset.denom, "inj") {
            Ok(route) => route.market_id,
            Err(_) => continue,
        };

        let (asset_inj_value, _) = simulate(
            deps,
            Uint128::from_str(&asset.amount.as_str())?,
            market_id.as_str(),
            &asset.denom,
        )?;

        total_inj_value += asset_inj_value;
    }

    Ok(total_inj_value)
}

pub(crate) fn try_bid(
    deps: DepsMut<InjectiveQueryWrapper>,
    env: Env,
    info: MessageInfo,
    round: u64,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    let current_auction = get_current_auction(deps.as_ref())?;

    // Check if the round is the same as the current auction
    if current_auction.auctionRound != round {
        return Err(ContractError::WrongRound {});
    }

    // Check if the contract is the highest bidder
    // This is to prevent outbidding ourselves
    if current_auction.highestBidder == env.contract.address.as_str() {
        return Err(ContractError::AlreadyHighestBidder {});
    }

    // Check if there is a bid from the previous round that needs to be settled
    if let Some(bid_attempt) = BID_ATTEMPT.may_load(deps.storage)? {
        if bid_attempt.round < current_auction.auctionRound {
            return Err(ContractError::UnsettledPreviousBid {});
        }
    }

    let config = CONFIG.load(deps.storage)?;

    // Check if it is time to bid
    // We want to push the bid as close to the end of the auction as possible
    if env
        .block
        .time
        .plus_seconds(config.bid_time_buffer_secs)
        .seconds()
        < u64::try_from(current_auction.auctionClosingTime).unwrap()
    {
        return Err(ContractError::NotInBidTime {});
    }

    // Calculate the minimum bid size
    let auction_params = get_auction_params(deps.as_ref())?;
    let highest_bid = current_auction.highestBidAmount.as_str();
    let mut min_bid_size = Decimal256::from_str(&highest_bid)?
        * (Decimal256::one() + Decimal256::from_str(&auction_params.min_next_bid_increment_rate)?);
    if min_bid_size.is_zero() {
        min_bid_size = Decimal256::one();
    }
    let min_bid_size = min_bid_size.to_uint_ceil();

    //Get the value with the router
    let basket_current_value = get_current_auction_value_using_router(deps.as_ref())?;

    //Get the value from directly EXCHANGE module
    // let basket_current_value = get_exchange_current_auction_value(deps.as_ref())?;

    if Uint256::from_uint128(basket_current_value) <= min_bid_size {
        return Err(ContractError::MinBidToHigh {});
    }

    let bid_msg = MsgBid {
        bid_amount: Some(ProstCoin {
            denom: config.accepted_denom,
            amount: min_bid_size.to_string(),
        }),
        sender: env.contract.address.into(),
        round,
    };

    let mut buf: Vec<u8> = Vec::new();
    bid_msg.encode(&mut buf)?;
    #[allow(deprecated)]
    let msg = CosmosMsg::Stargate {
        type_url: "/injective.auction.v1beta1.MsgBid".to_string(),
        value: buf.into(),
    };

    let submsg = SubMsg::reply_on_success(msg, TRY_BID_SUCCESS_REPLY_ID);

    BID_ATTEMPT_TRANSIENT.save(
        deps.storage,
        &BidAttempt {
            round,
            amount: Uint128::from_str(&min_bid_size.to_string())?,
            submitted_by: info.sender,
            basket: current_auction.amount,
        },
    )?;

    Ok(Response::new()
        .add_submessage(submsg)
        .add_attribute("method", "try_bid")
        .add_attribute("round", round.to_string())
        .add_attribute("min_bid_size", min_bid_size.to_string()))
}

pub fn try_settle(
    deps: DepsMut<InjectiveQueryWrapper>,
    env: Env,
    _sender: &Addr,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    let bid_attempt = match BID_ATTEMPT.may_load(deps.storage)? {
        Some(bid_attempt) => bid_attempt,
        None => return Err(ContractError::BidAttemptNotFound {}),
    };

    let last_auction_result = get_last_auction_result(deps.as_ref())?;

    /* deps.api.debug(&format!(
        "This the last auction from inside contract {:?}",
        &last_auction_result
    )); */

    /*     This works fine as its impossible to create a new bid_attempt untile the last one is deleted
    by settling his round */

    if &bid_attempt.round != &last_auction_result.round {
        return Err(ContractError::BidAttemptRoundNotFinished(
            bid_attempt.round,
            last_auction_result.round,
        ));
    }

    // at this point results of the round we attended are available
    // clear the bid attempt state
    BID_ATTEMPT_TRANSIENT.remove(deps.storage);
    BID_ATTEMPT.remove(deps.storage);

    let mut response = Response::new()
        .add_attribute("method", "try_settle")
        .add_attribute("round", bid_attempt.round.to_string())
        .add_event(
            Event::new("bid_info")
                .add_attribute("winner", &last_auction_result.winner)
                .add_attribute("bid_amount", &last_auction_result.amount.unwrap().amount),
        );

    if env.contract.address.as_str() != &last_auction_result.winner {
        response = response
            .add_event(Event::new("bid_result").add_attribute("result", BidResult::Loss))
            .add_attribute("winning_bidder", "");

        //If we lose we may delete the rewards ?
        return Ok(response);
    }

    response = response
        .add_event(Event::new("bid_result").add_attribute("result", BidResult::Win))
        .add_attribute("winning_bidder", bid_attempt.submitted_by.to_string());

    let config = CONFIG.load(deps.storage)?;

    for asset in bid_attempt.basket.iter() {
        // skip accepted denom (inj)
        if &asset.denom == &config.accepted_denom {
            continue;
        }

        // skip 0 amounts
        if asset.amount == "0" {
            continue;
        }

        let swap_route = read_swap_route(deps.as_ref(), &asset.denom, "inj");

        // in case of error skip the asset
        if swap_route.is_err() {
            continue;
        }

        let amount = Uint128::from_str(&asset.amount)?;
        let market_id = swap_route?.market_id;
        let msg = swap(
            deps.as_ref(),
            &env.contract.address,
            amount,
            market_id.as_str(),
            &asset.denom,
        )?;

        let mut submsg = SubMsg::reply_on_success(msg, SELL_ASSET_SUCCESS_REPLY_ID);
        submsg.payload = to_json_binary(asset)?;

        response = response
            .add_submessage(submsg)
            .add_attribute(format!("swap_out::{}", asset.denom), amount.to_string())
    }

    response = response.add_message(create_after_settle_message(
        deps,
        env.contract.address.as_str(),
        &bid_attempt,
    )?);

    Ok(response)
}

pub fn create_after_settle_message(
    deps: DepsMut<InjectiveQueryWrapper>,
    contract_addr: &str,
    bid_attempt: &BidAttempt,
) -> Result<CosmosMsg<InjectiveMsgWrapper>, ContractError> {
    // set the transient settled amount to zero
    // each of the sell asset submessages will add to the settled amount
    SETTLED_AMOUNT_TRANSIENT.save(deps.storage, &Uint128::zero())?;

    return Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_json_binary(&ExecuteMsg::Callback(
            auction_dao::msg::CallbackMsg::BidSettledSuccess {
                bid_attempt: bid_attempt.to_owned(),
            },
        ))
        .unwrap(),
        funds: vec![],
    }));
}

// method for clearing attempt bid which is outbid by another user
// if we enable contract bidding in last seconds, this method
// won't be most probably used

// only in cases where we have large bid_time_buffer_secs, we
// bid and then clear the bid if we are outbid in order to enable
// users withdraw and deposit
pub fn try_clear_current_bid(
    deps: DepsMut<InjectiveQueryWrapper>,
    env: Env,
    _sender: &Addr,
) -> Result<Response<InjectiveMsgWrapper>, ContractError> {
    let bid_attempt = match BID_ATTEMPT.may_load(deps.storage)? {
        Some(bid_attempt) => bid_attempt,
        None => return Err(ContractError::BidAttemptNotFound {}),
    };

    let current_auction = get_current_auction(deps.as_ref())?;

    if &bid_attempt.round != &current_auction.auctionRound {
        return Err(ContractError::BidAttemptRoundNotFinished(
            bid_attempt.round,
            current_auction.auctionRound,
        ));
    }

    if &current_auction.highestBidder == &env.contract.address.to_string() {
        return Err(ContractError::AlreadyHighestBidder {});
    }

    BID_ATTEMPT.remove(deps.storage);

    return Ok(Response::new()
        .add_attribute("method", "try_clear_current_bid")
        .add_attribute("round", bid_attempt.round.to_string()));
}
