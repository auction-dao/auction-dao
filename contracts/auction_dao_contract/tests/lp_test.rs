mod util;

mod tests {

    use std::str::FromStr;

    use crate::util::tests::{
        assert_approx_eq_uint128, create_realistic_inj_usdt_sell_orders_from_spreadsheet, init,
        init_contract_inj, init_router_contract_inj, launch_realistic_inj_usdt_spot_market,
        AUCTION_VAULT_ADDRESS, ONE_18, ONE_6,
    };
    use auction_dao::{
        error::ContractError,
        msg::{ExecuteMsg, QueryMsg},
        state::{Global, UserAccount},
    };

    use cosmwasm_std::{Coin, Uint128};
    use injective_std::types::{
        cosmos::bank::v1beta1::{MsgSend, QueryBalanceRequest},
        injective::auction::v1beta1::QueryCurrentAuctionBasketResponse,
    };
    use injective_test_tube::{Bank, Exchange, InjectiveTestApp, Wasm};
    use test_tube_inj::{Account, Module};

    #[test]
    fn total_supply_0_after_init() {
        let app = init();
        let admin = &app
            .init_accounts(&[Coin::new(10 * ONE_18, "inj")], 1)
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(state.total_supply, Uint128::zero());
    }
    #[test]
    fn deposit_wrong_asset() {
        let app = init();
        let admin = &app
            .init_accounts(&[Coin::new(10 * ONE_18, "inj")], 1)
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let user = &app
            .init_accounts(
                &[Coin::new(10 * ONE_18, "not_inj"), Coin::new(ONE_18, "inj")],
                1,
            )
            .unwrap()[0];

        let deposit_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(10 * ONE_18, "not_inj")],
            user,
        );

        assert!(deposit_response.is_err());
        // Extract the error and assert the error type and message
        if let Err(err) = &deposit_response {
            match err {
                test_tube_inj::RunnerError::ExecuteError { msg } => {
                    assert!(
                        msg.contains(ContractError::InvalidDenom {}.to_string().as_str()),
                        "Unexpected error message: {}",
                        msg
                    );
                }
                _ => panic!("Unexpected error type: {:?}", err),
            }
        }
    }

    #[test]
    fn deposit_inj() {
        let app = init();
        let admin = &app
            .init_accounts(&[Coin::new(100 * ONE_18, "inj")], 1)
            .unwrap()[0];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);
        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let deposited_amount = 10 * ONE_18;

        let user = &app
            .init_accounts(&[Coin::new(20 * ONE_18, "inj")], 1)
            .unwrap()[0];

        // We send 10 inj to the basket to let us commit
        let send_inj_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(10 * ONE_18, "inj".to_string()).into()],
            },
            admin,
        );

        assert!(send_inj_to_basket.is_ok());

        let deposit_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        );

        assert!(deposit_response.is_ok());

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::new(deposited_amount),
            "total supply not equal"
        );

        let user_account = wasm
            .query::<QueryMsg, UserAccount>(
                &contract_addr,
                &QueryMsg::User {
                    address: user.address(),
                },
            )
            .unwrap();

        assert_eq!(user_account.deposited, Uint128::new(deposited_amount));

        /* We try deposit again 6 more inj; should fail as baske value 10inj and we
        commitng 16 with a max_inj_offset of 150 (50%)
         */
        let deposit_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(6 * ONE_18, "inj")],
            user,
        );

        assert!(deposit_response.is_err());
        assert!(
            deposit_response
                .unwrap_err()
                .to_string()
                .contains("Cannot exceed max tokens"),
            "incorrect query result error message"
        );

        // If we try again with 5 inj should work
        let deposit_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(5 * ONE_18, "inj")],
            user,
        );
        assert!(deposit_response.is_ok());
    }

    #[test]
    fn deposit_inj_and_withdraw() {
        let app = init();
        let accounts = &app
            .init_accounts(&[Coin::new(1000 * ONE_18, "inj")], 3)
            .unwrap();

        let admin = &accounts[0];
        let user = &accounts[1];
        let user2 = &accounts[2];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);
        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let deposited_amount = 10 * ONE_18;

        // We send 2 * 10 inj to the basket to let us commit for the two users
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(2 * deposited_amount, "inj".to_string()).into()],
            },
            admin,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user2,
        )
        .unwrap();

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::new(2 * deposited_amount),
            "total supply not equal"
        );

        let user_account = wasm
            .query::<QueryMsg, UserAccount>(
                &contract_addr,
                &QueryMsg::User {
                    address: user.address(),
                },
            )
            .unwrap();

        assert_eq!(user_account.deposited, Uint128::new(deposited_amount));

        let user_account2 = wasm
            .query::<QueryMsg, UserAccount>(
                &contract_addr,
                &QueryMsg::User {
                    address: user2.address(),
                },
            )
            .unwrap();

        assert_eq!(user_account2.deposited, Uint128::new(deposited_amount));

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user2,
        )
        .unwrap();

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::zero(),
            "total supply not equal"
        );
    }

    #[test]
    fn deposit_inj_and_try_withdraw_before_auction() {
        let app = init();
        let accounts = &app
            .init_accounts(&[Coin::new(1000 * ONE_18, "inj")], 3)
            .unwrap();

        let admin = &accounts[0];
        let user = &accounts[1];
        let user2 = &accounts[2];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);
        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        let deposited_amount = 10 * ONE_18;

        // We send 2 * 10 inj to the basket to let us commit for the two users
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(2 * deposited_amount, "inj".to_string()).into()],
            },
            admin,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user2,
        )
        .unwrap();

        let current_auction_response = wasm
            .query::<QueryMsg, QueryCurrentAuctionBasketResponse>(
                &contract_addr,
                &QueryMsg::CurrentAuctionBasket {},
            )
            .unwrap();

        let auction_end_time = current_auction_response.auctionClosingTime;
        let current_time = app.get_block_time_seconds();

        // We set the blockchain time to auction_end_time - 5; Should pass time buffer

        let time_increase = u64::try_from(auction_end_time - current_time - 5).unwrap();
        app.increase_time(time_increase);

        let withdraw_r = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user,
        );

        assert!(withdraw_r.is_err());

        assert!(
            withdraw_r
                .unwrap_err()
                .to_string()
                .contains("Withdraw is disabled"),
            "incorrect query result error message"
        );

        app.increase_time(6);

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user2,
        )
        .unwrap();

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::zero(),
            "total supply not equal"
        );
    }

    #[test]
    fn deposit_inj_and_harvest_twice() {
        let app = init();
        let accounts = &app
            .init_accounts(
                &[
                    Coin::new(10000000 * ONE_18, "inj"),
                    Coin::new(100000 * ONE_6, "usdt"),
                ],
                3,
            )
            .unwrap();

        let admin = &accounts[0];
        let user = &accounts[1];
        let user2 = &accounts[2];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);

        let deposited_amount = 10 * ONE_18;

        // We send 2 * 10 inj to the basket to let us commit for the two users
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(2 * deposited_amount, "inj".to_string()).into()],
            },
            admin,
        )
        .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user2,
        )
        .unwrap();

        let usdt_profit = ONE_6 * 1000;

        // send USDT profit to the contract
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![Coin {
                    amount: usdt_profit.into(),
                    denom: "usdt".to_string(),
                }
                .into()],
            },
            &admin,
        )
        .unwrap();

        // Swap USDT to INJ
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: usdt_profit.into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            admin,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(&contract_addr, &ExecuteMsg::Harvest {}, &[], user)
            .unwrap();

        let ua = wasm
            .query::<QueryMsg, UserAccount>(
                &contract_addr,
                &QueryMsg::User {
                    address: user.address(),
                },
            )
            .unwrap();

        assert!(
            ua.pending_reward == Uint128::zero(),
            "pending reward not zero"
        );

        let contract_balance_before_next_harvest = bank
            .query_balance(&QueryBalanceRequest {
                address: contract_addr.clone(),
                denom: "inj".to_string(),
            })
            .unwrap();

        wasm.execute::<ExecuteMsg>(&contract_addr, &ExecuteMsg::Harvest {}, &[], user)
            .unwrap();

        let r = bank
            .query_balance(&QueryBalanceRequest {
                address: contract_addr.clone(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_eq!(
            r.balance.unwrap().amount,
            contract_balance_before_next_harvest.balance.unwrap().amount
        );
    }

    #[test]
    fn deposit_inj_and_harvest_and_withdraw() {
        let app = init();
        let accounts = &app
            .init_accounts(
                &[
                    Coin::new(10000000 * ONE_18, "inj"),
                    Coin::new(100000 * ONE_6, "usdt"),
                ],
                3,
            )
            .unwrap();

        let admin = &accounts[0];
        let user = &accounts[1];
        let user2 = &accounts[2];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);

        let deposited_amount = 10 * ONE_18;

        // We send 2 * 10 inj to the basket to let us commit for the two users
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(2 * deposited_amount, "inj".to_string()).into()],
            },
            admin,
        )
        .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user2,
        )
        .unwrap();

        let usdt_profit = ONE_6 * 1000;

        // send USDT profit to the contract
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![Coin {
                    amount: usdt_profit.into(),
                    denom: "usdt".to_string(),
                }
                .into()],
            },
            &admin,
        )
        .unwrap();

        // Swap USDT to INJ
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: usdt_profit.into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            admin,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user2,
        )
        .unwrap();

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::zero(),
            "total supply not equal"
        );

        let r = bank
            .query_balance(&QueryBalanceRequest {
                address: contract_addr.clone(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_eq!(r.balance.unwrap().amount, "0".to_string());
    }

    #[test]
    fn double_deposit_inj_and_harvest_and_withdraw() {
        let app = init();
        let initial_inj = 10000000 * ONE_18;
        let accounts = &app
            .init_accounts(
                &[
                    Coin::new(initial_inj, "inj"),
                    Coin::new(100000 * ONE_6, "usdt"),
                ],
                3,
            )
            .unwrap();

        let admin = &accounts[0];
        let user = &accounts[1];
        let user2 = &accounts[2];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);

        let deposited_amount = 10 * ONE_18;

        // We send 3 * 10 inj to the basket to let us commit for the two users
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(3 * deposited_amount, "inj".to_string()).into()],
            },
            admin,
        )
        .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user2,
        )
        .unwrap();

        let usdt_profit = ONE_6 * 10000;

        // send USDT profit to the contract
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![Coin {
                    amount: usdt_profit.into(),
                    denom: "usdt".to_string(),
                }
                .into()],
            },
            &admin,
        )
        .unwrap();

        // Swap USDT to INJ
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: usdt_profit.into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            admin,
        )
        .unwrap();

        let inj_profit = 471000000000000000000u128;

        // deposit some more after profit is added
        // it shouldn't have any effect on the previous profit distribution
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: (2 * deposited_amount).into(),
            },
            &[],
            user,
        )
        .unwrap();

        let user_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: user.address(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_approx_eq_uint128(
            Uint128::from_str(&user_balance.balance.unwrap().amount).unwrap(),
            (initial_inj + (inj_profit / 2)).into(),
            500,
        );

        let user_balance2 = bank
            .query_balance(&QueryBalanceRequest {
                address: user2.address(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_approx_eq_uint128(
            Uint128::from_str(&user_balance2.balance.unwrap().amount).unwrap(),
            (initial_inj + (inj_profit / 2)).into(),
            500,
        );

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user2,
        )
        .unwrap();

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::zero(),
            "total supply not equal"
        );

        let r = bank
            .query_balance(&QueryBalanceRequest {
                address: contract_addr.clone(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_eq!(r.balance.unwrap().amount, "0".to_string());
    }

    #[test]
    fn deposit_inj_and_harvest_and_deposit_and_withdraw() {
        let app = init();
        let admin_initial_inj = 10000000 * ONE_18;
        let initial_inj = 100 * ONE_18;
        let accounts = &app
            .init_accounts(
                &[
                    Coin::new(admin_initial_inj, "inj"),
                    Coin::new(100000 * ONE_6, "usdt"),
                ],
                1,
            )
            .unwrap();

        let admin = &accounts[0];

        let accounts = &app
            .init_accounts(&[Coin::new(initial_inj, "inj")], 2)
            .unwrap();
        let user = &accounts[0];
        let user2 = &accounts[1];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);

        let deposited_amount = 10 * ONE_18;

        // We send 3 * 10 inj to the basket to let us commit for the two users
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(3 * deposited_amount, "inj".to_string()).into()],
            },
            admin,
        )
        .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user2,
        )
        .unwrap();

        let usdt_profit = ONE_6 * 10000;

        // send USDT profit to the contract
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![Coin {
                    amount: usdt_profit.into(),
                    denom: "usdt".to_string(),
                }
                .into()],
            },
            &admin,
        )
        .unwrap();

        // Swap USDT to INJ
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: usdt_profit.into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            admin,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(&contract_addr, &ExecuteMsg::Harvest {}, &[], user)
            .unwrap();

        let inj_profit = 471000000000000000000u128;

        // deposit some more after profit is added
        // it shouldn't have any effect on the previous profit distribution
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(&contract_addr, &ExecuteMsg::Harvest {}, &[], user)
            .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: (2 * deposited_amount).into(),
            },
            &[],
            user,
        )
        .unwrap();

        let user_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: user.address(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_approx_eq_uint128(
            Uint128::from_str(&user_balance.balance.unwrap().amount).unwrap(),
            (initial_inj + (inj_profit / 2)).into(),
            500,
        );

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user2,
        )
        .unwrap();

        let user_balance2 = bank
            .query_balance(&QueryBalanceRequest {
                address: user2.address(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_approx_eq_uint128(
            Uint128::from_str(&user_balance2.balance.unwrap().amount).unwrap(),
            (initial_inj + (inj_profit / 2)).into(),
            500,
        );

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::zero(),
            "total supply not equal"
        );

        let r = bank
            .query_balance(&QueryBalanceRequest {
                address: contract_addr.clone(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_eq!(r.balance.unwrap().amount, "0".to_string());
    }

    #[test]
    fn deposit_inj_and_multiple_rewards_harvest_withdraw() {
        let app = init();
        let admin_initial_inj = 10000000 * ONE_18;
        let initial_inj = 100 * ONE_18;
        let accounts = &app
            .init_accounts(
                &[
                    Coin::new(admin_initial_inj, "inj"),
                    Coin::new(100000 * ONE_6, "usdt"),
                ],
                1,
            )
            .unwrap();

        let admin = &accounts[0];

        let accounts = &app
            .init_accounts(&[Coin::new(initial_inj, "inj")], 2)
            .unwrap();
        let user = &accounts[0];
        let user2 = &accounts[1];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let bank = Bank::new(&app);

        let deposited_amount = 10 * ONE_18;

        // We send 3 * 10 inj to the basket to let us commit for the two users
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![Coin::new(3 * deposited_amount, "inj".to_string()).into()],
            },
            admin,
        )
        .unwrap();

        let exchange = Exchange::new(&app);

        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(deposited_amount, "inj")],
            user2,
        )
        .unwrap();

        let usdt_profit = ONE_6 * 10000;

        // send USDT profit to the contract
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![Coin {
                    amount: usdt_profit.into(),
                    denom: "usdt".to_string(),
                }
                .into()],
            },
            &admin,
        )
        .unwrap();

        // Swap USDT to INJ
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: usdt_profit.into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            admin,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(&contract_addr, &ExecuteMsg::Harvest {}, &[], user)
            .unwrap();

        // send another USDT profit to the contract
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![Coin {
                    amount: usdt_profit.into(),
                    denom: "usdt".to_string(),
                }
                .into()],
            },
            &admin,
        )
        .unwrap();

        // Swap USDT to INJ
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: usdt_profit.into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            admin,
        )
        .unwrap();

        // twice usdt profit
        let inj_profit = 2 * 471000000000000000000u128;

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: (deposited_amount).into(),
            },
            &[],
            user,
        )
        .unwrap();

        let user_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: user.address(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_approx_eq_uint128(
            Uint128::from_str(&user_balance.balance.unwrap().amount).unwrap(),
            (initial_inj + (inj_profit / 2)).into(),
            500,
        );

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: deposited_amount.into(),
            },
            &[],
            user2,
        )
        .unwrap();

        let user_balance2 = bank
            .query_balance(&QueryBalanceRequest {
                address: user2.address(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_approx_eq_uint128(
            Uint128::from_str(&user_balance2.balance.unwrap().amount).unwrap(),
            (initial_inj + (inj_profit / 2)).into(),
            500,
        );

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.total_supply,
            Uint128::zero(),
            "total supply not equal"
        );

        let r = bank
            .query_balance(&QueryBalanceRequest {
                address: contract_addr.clone(),
                denom: "inj".to_string(),
            })
            .unwrap();

        assert_eq!(r.balance.unwrap().amount, "0".to_string());
    }
}
