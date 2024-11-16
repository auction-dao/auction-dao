mod util;
mod tests {

    use crate::util::tests::{
        create_realistic_inj_usdt_buy_orders_from_spreadsheet,
        create_realistic_inj_usdt_sell_orders_from_spreadsheet, init, init_contract_inj,
        init_router_contract_inj, launch_realistic_inj_usdt_spot_market, AUCTION_VAULT_ADDRESS,
        ONE_18, ONE_6,
    };
    use auction_dao::{
        error::ContractError,
        msg::{ExecuteMsg, QueryMsg},
        state::Global,
    };

    use cosmwasm_std::{Coin, Uint128};
    use injective_std::types::cosmos::{bank::v1beta1::MsgSend, base::v1beta1::Coin as BaseCoin};
    use injective_test_tube::{Bank, Exchange, InjectiveTestApp, Wasm};
    use test_tube_inj::{Account, Module};

    #[test]
    fn unauthorized_manual_swap() {
        let app = init();
        let accounts = &app
            .init_accounts(
                &[
                    Coin::new(10000000 * ONE_18, "inj"),
                    Coin::new(1000000000000000 * ONE_6, "usdt"),
                ],
                2,
            )
            .unwrap();

        let admin = &accounts[0];
        let user = &accounts[1];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let exchange = Exchange::new(&app);
        let bank = Bank::new(&app);

        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

        // Set the route in the contract
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "inj".to_string(),
                target_denom: "usdt".to_string(),
                market_id: market_id.clone(),
            },
            &[],
            admin,
        )
        .unwrap();

        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![BaseCoin {
                    amount: (ONE_6 * 1000).to_string(),
                    denom: "usdt".to_string(),
                }],
            },
            &admin,
        )
        .unwrap();

        // Set the route in the contract
        let r = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: (ONE_6 * 1000).into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            user,
        );

        assert!(r.is_err(), "Expected unauthorized error");

        // Extract the error and assert the error type and message
        if let Err(err) = &r {
            match err {
                test_tube_inj::RunnerError::ExecuteError { msg } => {
                    assert!(
                        msg.contains(ContractError::Unauthorized {}.to_string().as_str()),
                        "Unexpected error message: {}",
                        msg
                    );
                }
                _ => panic!("Unexpected error type: {:?}", err),
            }
        }
    }

    #[test]
    fn manual_swap() {
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
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::SetRoute {
                source_denom: "inj".to_string(),
                target_denom: "usdt".to_string(),
                market_id: market_id.clone(),
            },
            &[],
            admin,
        )
        .unwrap();

        // send to auction module
        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![BaseCoin {
                    amount: (ONE_18 * 1).to_string(),
                    denom: "inj".to_string(),
                }],
            },
            &admin,
        )
        .unwrap();

        // initial deposit
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(1 * ONE_18, "inj")],
            admin,
        )
        .unwrap();

        bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: contract_addr.clone(),
                amount: vec![BaseCoin {
                    amount: (ONE_6 * 1000).to_string(),
                    denom: "usdt".to_string(),
                }],
            },
            &admin,
        )
        .unwrap();

        // Swap
        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::ManualExchangeSwap {
                amount: (ONE_6 * 1000).into(),
                market_id: market_id.clone(),
                asset: "usdt".to_string(),
            },
            &[],
            admin,
        )
        .unwrap();

        let state = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        assert_eq!(
            state.accumulated_profit,
            Uint128::new(47100000000000000000),
            "accumulated profit not equal"
        );
        assert_ne!(
            state.accumulated_profit,
            Uint128::new(0),
            "reward index not updated"
        );
    }
}
