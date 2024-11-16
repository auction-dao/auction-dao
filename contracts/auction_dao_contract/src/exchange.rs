// methods for interacting with the exchange module directly

use core::str;
use std::str::FromStr;

use crate::{
    fixed_types::{Params, QueryExchangeParamsResponse, QuerySpotMarketResponse, SpotMarket},
    state::CONFIG,
};
use auction_dao::error::ContractError;
use cosmwasm_std::{Addr, CosmosMsg, Decimal256, Deps, QueryRequest, Uint128};
use injective_cosmwasm::{
    create_spot_market_order_msg, InjectiveMsgWrapper, InjectiveQueryWrapper, MarketId, OrderSide,
    OrderType, SpotOrder, SubaccountId,
};
use injective_math::FPDecimal;
use injective_std::types::injective::exchange::v1beta1::{
    QueryExchangeParamsRequest, QuerySpotMarketRequest, QuerySpotOrderbookRequest,
    QuerySpotOrderbookResponse,
};

pub fn get_market(
    market_id: &str,
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<SpotMarket, ContractError> {
    // I'm leaving here raw query example for future reference

    // #[allow(deprecated)]
    // let raw = to_json_vec::<QueryRequest>(&QueryRequest::Stargate {
    //     path: "/injective.exchange.v1beta1.Query/SpotMarket".to_string(),
    //     data: QuerySpotMarketRequest {
    //         market_id: market_id.to_string(),
    //     }
    //     .into(),
    // })?;

    // let raw_query_result = deps.querier.raw_query(&raw).unwrap().unwrap();
    // let raw_query_str = str::from_utf8(&raw_query_result)?;
    // deps.api.debug(&format!("raw_query_str: {}", raw_query_str));

    #[allow(deprecated)]
    let spot_market: QuerySpotMarketResponse = deps.querier.query(&QueryRequest::Stargate {
        path: "/injective.exchange.v1beta1.Query/SpotMarket".to_string(),
        data: QuerySpotMarketRequest {
            market_id: market_id.to_string(),
        }
        .into(),
    })?;

    if let Some(market) = spot_market.market {
        return Ok(market);
    } else {
        return Err(ContractError::MarketNotFound {});
    }
}

pub fn get_exchange_params(deps: Deps<InjectiveQueryWrapper>) -> Result<Params, ContractError> {
    #[allow(deprecated)]
    let exchange_params: QueryExchangeParamsResponse =
        deps.querier.query(&QueryRequest::Stargate {
            path: "/injective.exchange.v1beta1.Query/QueryExchangeParams".to_string(),
            data: QueryExchangeParamsRequest {}.into(),
        })?;

    if let Some(params) = exchange_params.params {
        return Ok(params);
    } else {
        return Err(ContractError::ExchangeParamsNotFound {});
    }
}

// INJ/USDT market
// INJ -> base , USDT -> quote
// buying INJ with USDT, check sell side orderbook, create buy order
pub fn simulate_quote_offer(
    amount: Uint128,
    market: &SpotMarket,
    params: &Params,
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<(Uint128, String), ContractError> {
    // Possible alternative with some tweaks needed to avoid deprecated? not sure lmao
    /*   let querrier = InjectiveQuerier::new(&deps.querier);
    let order_book = querrier.query_spot_market_orderbook(
        &market.market_id.clone().as_str(),
        OrderSide::Buy,
        Some(FPDecimal::from(amount)),
        Some(FPDecimal::default())
    ).unwrap(); */

    #[allow(deprecated)]
    let order_book: QuerySpotOrderbookResponse = deps.querier.query(&QueryRequest::Stargate {
        path: "/injective.exchange.v1beta1.Query/SpotOrderbook".to_string(),
        data: QuerySpotOrderbookRequest {
            market_id: market.market_id.clone(),
            order_side: OrderSide::Sell as i32,
            limit_cumulative_notional: amount.to_string(),
            ..Default::default()
        }
        .into(),
    })?;

    // quote asset to be swapped e.g. 10000 USDT
    let mut amount = Decimal256::from_atomics(amount, 0)?;
    let min_amount_tick = Decimal256::from_str(&market.min_price_tick_size)?;
    amount = strip_min_tick(amount, min_amount_tick);
    // taker fee as we execute the order with market price
    let fee = Decimal256::from_str(&market.taker_fee_rate)?
        * Decimal256::from_str(&params.spot_atomic_market_order_fee_multiplier)?;
    // amount after fee
    amount = amount * (Decimal256::one() - fee);

    // received quantity of base asset, e.g. INJ
    let mut quantity = Decimal256::zero();
    let min_quantity_tick = Decimal256::from_str(&market.min_quantity_tick_size)?;

    if order_book.sells_price_level.len() == 0 {
        return Err(ContractError::NotEnoughLiquidity {});
    }

    for price_level in order_book.sells_price_level.iter() {
        let price = Decimal256::from_str(&price_level.p)?;
        let level_quantity = Decimal256::from_str(&price_level.q)?;

        let price_level_notional = price * level_quantity;

        // if amount is greater than the notional of the price level
        // then we can buy all the quantity at this price level
        if amount > price_level_notional {
            amount -= price_level_notional;
            quantity += level_quantity;

        // if amount is less than the notional of the price level
        // then calculate the quantity we can buy at this price level
        } else {
            quantity += amount / price;
            amount = Decimal256::zero();
            break;
        }
    }

    quantity = strip_min_tick(quantity, min_quantity_tick);

    if amount > Decimal256::zero() {
        return Err(ContractError::NotEnoughLiquidity {});
    }

    let worst_acceptable_price = order_book.sells_price_level.last().unwrap().p.clone();

    let quantity_int = Uint128::from_str(&quantity.to_uint_floor().to_string())?;

    return Ok((quantity_int, worst_acceptable_price));
}

// potentially in future ASSET/INJ market, e.g.
// HINJ/INJ market
// HINJ -> base , INJ -> quote
// selling HINJ for INJ, check buy side orderbook, create sell order
pub fn simulate_base_offer(
    quantity: Uint128,
    market: &SpotMarket,
    params: &Params,
    deps: Deps<InjectiveQueryWrapper>,
) -> Result<(Uint128, String), ContractError> {
    // Possible alternative with some tweaks needed to avoid deprecated? not sure lmao
    /*   let querrier = InjectiveQuerier::new(&deps.querier);
    let order_book = querrier.query_spot_market_orderbook(
        &market.market_id.clone().as_str(),
        OrderSide::Buy,
        Some(FPDecimal::from(quantity)),
        Some(FPDecimal::default())
    ).unwrap(); */

    #[allow(deprecated)]
    let order_book: QuerySpotOrderbookResponse = deps.querier.query(&QueryRequest::Stargate {
        path: "/injective.exchange.v1beta1.Query/SpotOrderbook".to_string(),
        data: QuerySpotOrderbookRequest {
            market_id: market.market_id.clone(),
            order_side: OrderSide::Buy as i32,
            limit_cumulative_quantity: quantity.to_string(),
            ..Default::default()
        }
        .into(),
    })?;

    // quote asset to be swapped e.g. 10000 USDT
    let mut quantity = Decimal256::from_atomics(quantity, 0)?;
    let min_quantity_tick = Decimal256::from_str(&market.min_quantity_tick_size)?;
    // strip the quantity to the minimum tick size
    quantity = strip_min_tick(quantity, min_quantity_tick);

    let mut quote_amount = Decimal256::zero();

    if order_book.buys_price_level.len() == 0 {
        return Err(ContractError::NotEnoughLiquidity {});
    }

    for price_level in order_book.buys_price_level.iter() {
        let price = Decimal256::from_str(&price_level.p)?;
        let level_quantity = Decimal256::from_str(&price_level.q)?;

        // if quantity is greater than price level quantity
        // then we can sell all the quantity at this price level
        if quantity > level_quantity {
            quantity -= level_quantity;
            quote_amount += price * level_quantity

        // if quantity is less than price level quantity
        // then calculate the quote we can get at this price level
        } else {
            quote_amount += price * quantity;
            quantity = Decimal256::zero();

            break;
        }
    }

    if quantity > Decimal256::zero() {
        return Err(ContractError::NotEnoughLiquidity {});
    }

    let fee = Decimal256::from_str(&market.taker_fee_rate)?
        * Decimal256::from_str(&params.spot_atomic_market_order_fee_multiplier)?;

    quote_amount = quote_amount * (Decimal256::one() - fee);

    let worst_acceptable_price = order_book.buys_price_level.last().unwrap().p.clone();

    let quote_amount_int = Uint128::from_str(&quote_amount.to_uint_floor().to_string())?;

    return Ok((quote_amount_int, worst_acceptable_price));
}

pub fn simulate(
    deps: Deps<InjectiveQueryWrapper>,
    amount: Uint128,
    market_id: &str,
    asset: &str,
) -> Result<(Uint128, String), ContractError> {
    let market = get_market(&market_id, deps)?;
    let params = get_exchange_params(deps)?;

    if market.base_denom == asset {
        // we simulate selling base asset for quote asset
        // e.g. selling hINJ for INJ
        let (quantity, worst_price) = simulate_base_offer(amount, &market, &params, deps)?;

        Ok((quantity, worst_price))
    } else if market.quote_denom == asset {
        // we simulate buying base asset with quote asset
        // e.g. buying INJ with USDT
        let (quantity, worst_price) = simulate_quote_offer(amount, &market, &params, deps)?;

        Ok((quantity, worst_price))
    } else {
        return Err(ContractError::AssetNotFound {});
    }
}

pub fn strip_min_tick(price: Decimal256, min_tick: Decimal256) -> Decimal256 {
    let price = (price / min_tick).floor();
    price * min_tick
}

pub fn create_spot_order_msg(
    _deps: Deps<InjectiveQueryWrapper>,
    sender: &Addr,
    subaccount_id: SubaccountId,
    market_id: &str,
    quantity: &str,
    worst_price: &str,
    order_type: OrderType,
) -> Result<CosmosMsg<InjectiveMsgWrapper>, ContractError> {
    let order = SpotOrder::new(
        FPDecimal::from_str(worst_price).unwrap(),
        FPDecimal::from_str(quantity).unwrap(),
        order_type,
        &MarketId::new(market_id)?,
        subaccount_id,
        Some(sender.to_owned()),
        None,
    );

    // deps.api.debug(&format!("order: {:?}", order));

    Ok(create_spot_market_order_msg(sender.to_owned(), order))
}

pub fn swap(
    deps: Deps<InjectiveQueryWrapper>,
    contract_addr: &Addr,
    amount: Uint128,
    market_id: &str,
    asset: &str,
) -> Result<CosmosMsg<InjectiveMsgWrapper>, ContractError> {
    let market = get_market(&market_id, deps)?;
    let params = get_exchange_params(deps)?;
    let config = CONFIG.load(deps.storage)?;

    if market.base_denom == asset {
        // swapping base asset for quote asset aka sell order
        // e.g. selling hINJ for INJ
        let (_, worst_price) = simulate_base_offer(amount, &market, &params, deps)?;
        let quantity = amount;
        let msg = create_spot_order_msg(
            deps,
            contract_addr,
            config.contract_subaccount_id,
            market_id,
            &quantity.to_string(),
            &worst_price,
            OrderType::SellAtomic,
        )?;
        Ok(msg)
    } else if market.quote_denom == asset {
        // swapping quote asset for base asset aka buy order
        // e.g. buying INJ with USDT
        let (quantity, worst_price) = simulate_quote_offer(amount, &market, &params, deps)?;
        let msg = create_spot_order_msg(
            deps,
            contract_addr,
            config.contract_subaccount_id,
            market_id,
            &quantity.to_string(),
            &worst_price,
            OrderType::BuyAtomic,
        )?;
        Ok(msg)
    } else {
        return Err(ContractError::AssetNotFound {});
    }
}
