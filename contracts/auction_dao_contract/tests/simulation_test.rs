mod util;

mod tests {

    use crate::util::tests::{
        decimal_str_to_big_int_str, init, init_contract_inj, init_router_contract_inj,
        launch_custom_spot_market, ONE_18, ONE_6,
    };
    use auction_dao::msg::QueryMsg;

    use cosmwasm_std::{Addr, Coin, Uint128, Uint256};
    use injective_cosmwasm::{get_default_subaccount_id_for_checked_address, OrderType};
    use injective_std::types::injective::exchange::v1beta1;
    use injective_test_tube::{Exchange, InjectiveTestApp, Wasm};
    use test_tube_inj::{Account, Module};

    #[test]
    fn launch_market_test() {
        let app = init();

        let trader = app
            .init_account(&[
                Coin::new(10000 * ONE_18, "inj"),
                Coin::new(10000 * ONE_6, "usdt"),
            ])
            .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_custom_spot_market(
            &exchange,
            &trader,
            "inj",
            "usdt",
            "1000",
            &decimal_str_to_big_int_str("1000000000000000"),
            &decimal_str_to_big_int_str("1000000"),
        );

        let spot_market = exchange
            .query_spot_market(&v1beta1::QuerySpotMarketRequest {
                market_id: market_id.to_string(),
            })
            .unwrap();

        let expected_response = v1beta1::QuerySpotMarketResponse {
            market: Some(v1beta1::SpotMarket {
                ticker: "inj/usdt".to_string(),
                base_denom: "inj".to_string(),
                quote_denom: "usdt".to_string(),
                maker_fee_rate: "-100000000000000".to_string(),
                taker_fee_rate: "500000000000000".to_string(),
                relayer_fee_share_rate: "400000000000000000".to_string(),
                market_id: "0xd5a22be807011d5e42d5b77da3f417e22676efae494109cd01c242ad46630115"
                    .to_string(),
                status: v1beta1::MarketStatus::Active as i32,
                min_price_tick_size: "1000".to_string(),
                min_quantity_tick_size: decimal_str_to_big_int_str("1000000000000000"),
                min_notional: decimal_str_to_big_int_str("1000000"),
                admin: "".to_string(),
                admin_permissions: 0u32,
            }),
        };
        assert_eq!(spot_market, expected_response);
    }

    #[test]
    fn query_quote_simulation() {
        let app = init();
        let admin = &app
            .init_accounts(&[Coin::new(10 * ONE_18, "inj")], 1)
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let trader = app
            .init_account(&[
                Coin::new(10000 * ONE_18, "inj"),
                Coin::new(10000 * ONE_6, "usdt"),
            ])
            .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_custom_spot_market(
            &exchange,
            &trader,
            "inj",
            "usdt",
            "1000",
            &decimal_str_to_big_int_str("1000000000000000"),
            &decimal_str_to_big_int_str("1000000"),
        );

        exchange
            .create_spot_limit_order(
                v1beta1::MsgCreateSpotLimitOrder {
                    sender: trader.address(),
                    order: Some(v1beta1::SpotOrder {
                        market_id: market_id.to_string(),
                        order_info: Some(v1beta1::OrderInfo {
                            subaccount_id: get_default_subaccount_id_for_checked_address(
                                &Addr::unchecked(trader.address()),
                            )
                            .as_str()
                            .to_string(),
                            fee_recipient: trader.address(),
                            price: "21007000".to_string(),
                            quantity: decimal_str_to_big_int_str("357032000000000000000"),
                            cid: "".to_string(),
                        }),
                        order_type: OrderType::Sell as i32,
                        trigger_price: "".to_string(),
                    }),
                },
                &trader,
            )
            .unwrap();

        let simulation_response = wasm
            .query::<QueryMsg, Uint256>(
                &contract_addr,
                &QueryMsg::ExchangeSimulateSwap {
                    amount: Uint128::new(ONE_6),
                    market_id: "0xd5a22be807011d5e42d5b77da3f417e22676efae494109cd01c242ad46630115"
                        .to_string(),
                    asset: "usdt".to_string(),
                },
            )
            .unwrap();

        assert_eq!(simulation_response, Uint256::from_u128(47000000000000000));
    }

    #[test]
    fn query_base_simulation() {
        let app = init();
        let admin = &app
            .init_accounts(&[Coin::new(10 * ONE_18, "inj")], 1)
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let trader = app
            .init_account(&[
                Coin::new(10000 * ONE_18, "inj"),
                Coin::new(10000 * ONE_6, "usdt"),
            ])
            .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_custom_spot_market(
            &exchange,
            &trader,
            "inj",
            "usdt",
            "1000",
            &decimal_str_to_big_int_str("1000000000000000"),
            &decimal_str_to_big_int_str("1000000"),
        );

        exchange
            .create_spot_limit_order(
                v1beta1::MsgCreateSpotLimitOrder {
                    sender: trader.address(),
                    order: Some(v1beta1::SpotOrder {
                        market_id: market_id.to_string(),
                        order_info: Some(v1beta1::OrderInfo {
                            subaccount_id: get_default_subaccount_id_for_checked_address(
                                &Addr::unchecked(trader.address()),
                            )
                            .as_str()
                            .to_string(),
                            fee_recipient: trader.address(),
                            price: decimal_str_to_big_int_str("0.000000000021006000"),
                            quantity: decimal_str_to_big_int_str("357032000000000000000"),
                            cid: "".to_string(),
                        }),
                        order_type: OrderType::Buy as i32,
                        trigger_price: "".to_string(),
                    }),
                },
                &trader,
            )
            .unwrap();

        let simulation_response = wasm
            .query::<QueryMsg, Uint256>(
                &contract_addr,
                &QueryMsg::ExchangeSimulateSwap {
                    amount: Uint128::new(ONE_18),
                    market_id: market_id.to_string(),
                    asset: "inj".to_string(),
                },
            )
            .unwrap();

        assert_eq!(simulation_response, Uint256::from_u128(20984994));
    }
}
