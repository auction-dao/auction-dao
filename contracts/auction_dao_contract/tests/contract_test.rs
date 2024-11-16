mod util;

mod tests {
    use std::str::FromStr;

    use crate::util::tests::{
        assert_approx_eq_uint128, create_realistic_inj_usdt_buy_orders_from_spreadsheet,
        create_realistic_inj_usdt_sell_orders_from_spreadsheet, init, init_contract_inj,
        init_router_contract_inj, launch_realistic_inj_usdt_spot_market, AUCTION_VAULT_ADDRESS,
        ONE_18, ONE_6,
    };
    use auction_dao::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

    use cosmwasm_std::{Coin, Uint128};
    use injective_math::FPDecimal;
    use injective_std::types::{
        cosmos::bank::v1beta1::MsgSend,
        injective::{
            auction::v1beta1::QueryCurrentAuctionBasketResponse,
            exchange::v1beta1::QuerySpotMarketRequest,
        },
    };
    use injective_test_tube::{Bank, Exchange, InjectiveTestApp, Wasm};
    use test_tube_inj::{Account, Module};

    #[test]
    fn querry_current_basket() {
        let app = init();
        let admin = &app
            .init_accounts(&[Coin::new(10 * ONE_18, "inj")], 1)
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        wasm.query::<QueryMsg, QueryCurrentAuctionBasketResponse>(
            &contract_addr,
            &QueryMsg::CurrentAuctionBasket {},
        )
        .expect("Query failed; unable to fetch the current auction basket.");
    }

    #[test]
    fn test_update_config() {
        let app = init();
        let admin = &app
            .init_accounts(&[Coin::new(10 * ONE_18, "inj")], 1)
            .unwrap()[0];
        let user = &app
            .init_accounts(&[Coin::new(10 * ONE_18, "inj")], 1)
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let try_config_attack = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::UpdateConfig {
                new_config: InstantiateMsg {
                    admin: admin.address(),
                    accepted_denom: "inj".to_string(),
                    swap_router: router_contract_add.to_string(),
                    time_buffer: 5,
                    max_inj_offset_bps: Uint128::from(15900u128),
                    winning_bidder_reward_bps: Uint128::from(1000u128),
                },
            },
            &[Coin::new(Uint128::one(), "inj")],
            user,
        );

        assert!(try_config_attack.is_err());

        let try_config_update = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::UpdateConfig {
                new_config: InstantiateMsg {
                    admin: admin.address(),
                    accepted_denom: "inj".to_string(),
                    swap_router: router_contract_add.to_string(),
                    time_buffer: 5,
                    max_inj_offset_bps: Uint128::from(14000u128),
                    winning_bidder_reward_bps: Uint128::from(500u128),
                },
            },
            &[Coin::new(Uint128::one(), "inj")],
            admin,
        );

        assert!(try_config_update.is_ok());

        let try_config_update_wrong_fields = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::UpdateConfig {
                new_config: InstantiateMsg {
                    admin: admin.address(),
                    accepted_denom: "inj".to_string(),
                    swap_router: "gdssfsasa".to_string(),
                    time_buffer: 5,
                    max_inj_offset_bps: Uint128::from(15000u128),
                    winning_bidder_reward_bps: Uint128::from(1000u128),
                },
            },
            &[Coin::new(Uint128::one(), "inj")],
            admin,
        );

        assert!(try_config_update_wrong_fields.is_err());
    }

    #[test]
    fn querry_auction_value() {
        let app = init();
        let admin = &app
            .init_accounts(
                &[
                    Coin::new(10000000000000000000 * ONE_18, "inj"),
                    Coin::new(100000000000000000000 * ONE_6, "usdt"),
                ],
                1,
            )
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let exchange = Exchange::new(&app);
        let bank = Bank::new(&app);

        // First of all we launch the market and create orders as those send some contributions to the basket
        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_buy_orders_from_spreadsheet(&exchange, &market_id, &admin);
        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        // Define amounts to send to the basket and equivalent usdt value in inj; max inj is 10k inj

        let inj_amount_to_basket = Coin::new(10000 * ONE_18, "inj");
        let usdt_amount_to_basket = Coin::new(200000 * ONE_6, "usdt");

        let usdt_price_scaled = 21_217_000u128;

        let usdt_amount_scaled = usdt_amount_to_basket
            .amount
            .checked_mul(ONE_18.into())
            .unwrap();

        let usdt_inj_value = usdt_amount_scaled
            .checked_div(usdt_price_scaled.into())
            .unwrap();

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        // We get the current basket valiue
        let current_value = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::RouterCurrentAuctionValue {})
            .unwrap();

        // Calculate the expected next value by adding to the current value the amount of injs sent to basket
        let next_expected_value = current_value
            .checked_add(inj_amount_to_basket.clone().amount)
            .unwrap();

        let send_inj_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![inj_amount_to_basket.clone().into()],
            },
            admin,
        );

        assert!(send_inj_to_basket.is_ok());

        // We get the updated basket value and check eq
        let current_value = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::RouterCurrentAuctionValue {})
            .unwrap();

        assert_eq!(current_value, next_expected_value);

        // We update the expected value to account for the about to send usdt

        let next_expected_value = current_value.checked_add(usdt_inj_value.clone()).unwrap();

        // Now we also send the usdt, get the basket and check value; should fail cause no route setted
        let send_usdt_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![usdt_amount_to_basket.into()],
            },
            admin,
        );

        assert!(send_usdt_to_basket.is_ok());

        let current_value = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::RouterCurrentAuctionValue {})
            .unwrap();

        assert_ne!(current_value, next_expected_value);

        // We now set the route and try again
        let set_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "inj".to_string(),
                target_denom: "usdt".to_string(),
                market_id,
            },
            &[],
            admin,
        );

        assert!(set_route_response.is_ok());

        let current_value = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::RouterCurrentAuctionValue {})
            .unwrap();

        assert_approx_eq_uint128(current_value, next_expected_value, 500);
    }

    #[test]
    fn querry_auction_value_using_router_and_exchange() {
        let app = init();
        let admin = &app
            .init_accounts(
                &[
                    Coin::new(10000000 * ONE_18, "inj"),
                    Coin::new(1000000000000000 * ONE_6, "usdt"),
                ],
                1,
            )
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let exchange = Exchange::new(&app);
        let bank = Bank::new(&app);

        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_buy_orders_from_spreadsheet(&exchange, &market_id, &admin);
        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        // Set the route in the contract
        let set_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "inj".to_string(),
                target_denom: "usdt".to_string(),
                market_id: market_id.clone(),
            },
            &[],
            admin,
        );

        assert!(set_route_response.is_ok());

        let current_auction_response = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::RouterCurrentAuctionValue {})
            .unwrap();

        let expected_value = current_auction_response
            .checked_add(Uint128::new(100000000))
            .unwrap();

        let send_inj_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![injective_std::types::cosmos::base::v1beta1::Coin {
                    denom: "inj".to_string(),                    // INJ token denom
                    amount: Uint128::new(100000000).to_string(), // Adjust this to the amount in INJ's smallest unit (wei)
                }],
            },
            admin,
        );
        assert!(send_inj_to_basket.is_ok());

        let current_auction_response = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::RouterCurrentAuctionValue {})
            .unwrap();

        assert_eq!(current_auction_response, expected_value);

        // Send 10000 usdt to auction basket
        let send_usd_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![injective_std::types::cosmos::base::v1beta1::Coin {
                    denom: "usdt".to_string(),                       // INJ token denom
                    amount: Uint128::new(10000 * ONE_6).to_string(), // Adjust this to the amount in INJ's smallest unit (wei)
                }],
            },
            admin,
        );

        assert!(send_usd_to_basket.is_ok());

        let current_auction_response = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::RouterCurrentAuctionValue {})
            .unwrap();

        let exchange_current_auction_response = wasm
            .query::<QueryMsg, Uint128>(&contract_addr, &QueryMsg::ExchangeCurrentAuctionValue {})
            .unwrap();

        let spot_market = exchange
            .query_spot_market(&QuerySpotMarketRequest {
                market_id: market_id.to_string(),
            })
            .unwrap();

        // the router round fractional value, I better expect worse and round it down to
        // the min quantity tick size
        let diff = current_auction_response.abs_diff(exchange_current_auction_response);

        let tick: &Result<u128, _> =
            &FPDecimal::from_str(&spot_market.market.unwrap().min_quantity_tick_size)
                .unwrap()
                .try_into();

        assert!(diff.le(&tick.unwrap().into()));
    }

    #[test]
    fn test_routs() {
        let app = init();
        let admin = &app
            .init_accounts(
                &[
                    Coin::new(10000000 * ONE_18, "inj"),
                    Coin::new(1000000000000000 * ONE_6, "usdt"),
                ],
                1,
            )
            .unwrap()[0];
        let exchange = Exchange::new(&app);
        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);
        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let set_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "usdt".to_string(),
                target_denom: "insj".to_string(),
                market_id: market_id.clone(),
            },
            &[],
            admin,
        );
        assert!(
            set_route_response.is_err(),
            "set route should have failed cause wrong denoms"
        );

        let set_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "usdt".to_string(),
                target_denom: "inj".to_string(),
                market_id: market_id.clone(),
            },
            &[],
            admin,
        );
        assert!(set_route_response.is_ok());

        let set_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "usdt".to_string(),
                target_denom: "inj".to_string(),
                market_id: market_id.clone(),
            },
            &[],
            admin,
        );
        assert!(
            set_route_response.is_err(),
            "set route should have failed cause already exist"
        );

        let delete_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::DeleteRoute {
                source_denom: "usdt".to_string(),
                target_denom: "inj".to_string(),
            },
            &[],
            admin,
        );
        assert!(delete_route_response.is_ok());

        let set_fake_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "usdt".to_string(),
                target_denom: "inj".to_string(),
                market_id: "0xa508cb32923323679f29a032c70342c147c17d0145625922b0ef22e955c844c0"
                    .to_string(),
            },
            &[],
            admin,
        );

        assert!(
            set_fake_route_response.is_err(),
            "set route should have failed"
        );

        let set_fake_route_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "usdt".to_string(),
                target_denom: "inj".to_string(),
                market_id: "0xa508cb329233236hs2c70342c147c17d0145625922b0ef22e955c844c0"
                    .to_string(),
            },
            &[],
            admin,
        );
        assert!(
            set_fake_route_response.is_err(),
            "set route should have failed"
        );
    }
}
