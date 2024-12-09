mod util;

mod tests {

    use crate::util::tests::{
        assert_approx_eq_uint128, create_realistic_inj_usdt_buy_orders_from_spreadsheet,
        create_realistic_inj_usdt_sell_orders_from_spreadsheet, init, init_contract_inj,
        init_router_contract_inj, launch_realistic_inj_usdt_spot_market, AUCTION_VAULT_ADDRESS,
        INJ, ONE_18, ONE_6,
    };
    use auction_dao::{
        msg::{ExecuteMsg, QueryMsg},
        state::Global,
    };

    use injective_std::types::cosmos::{
        bank::v1beta1::QueryBalanceRequest, base::v1beta1::Coin as BidCoin,
    };

    use cosmwasm_std::{Coin, Uint128};
    use injective_std::types::{
        cosmos::bank::v1beta1::MsgSend,
        injective::auction::v1beta1::{MsgBid, QueryCurrentAuctionBasketResponse},
    };
    use injective_test_tube::{Auction, Bank, Exchange, InjectiveTestApp, Wasm};
    use test_tube_inj::{Account, Module};

    #[test]
    fn test_scenario_1() {
        let app = init();
        let admin_initial_inj = 10000000 * ONE_18;
        let initial_inj = 10005 * ONE_18;
        let accounts = &app
            .init_accounts(
                &[
                    Coin::new(admin_initial_inj, "inj"),
                    Coin::new(10000000 * ONE_6, "usdt"),
                ],
                1,
            )
            .unwrap();

        let admin = &accounts[0];

        let accounts = &app
            .init_accounts(&[Coin::new(initial_inj, "inj")], 2)
            .unwrap();
        let user1 = &accounts[0];
        let user2 = &accounts[1];

        let wasm: Wasm<'_, InjectiveTestApp> = Wasm::new(&app);
        let exchange = Exchange::new(&app);
        let auction = Auction::new(&app);
        let bank = Bank::new(&app);

        // First of all we launch the market and create orders as those send some contributions to the basket
        let market_id = launch_realistic_inj_usdt_spot_market(&exchange, &admin);

        create_realistic_inj_usdt_buy_orders_from_spreadsheet(&exchange, &market_id, &admin);
        create_realistic_inj_usdt_sell_orders_from_spreadsheet(&exchange, &market_id, &admin);

        // Define amounts to send to the basket and equivalent usdt value in inj; max inj is 10k inj
        let inj_amount_to_basket = Coin::new(10000 * ONE_18, "inj");
        let usdt_amount_to_basket = Coin::new(200000 * ONE_6, "usdt");

        let router_contract_add = init_router_contract_inj(&wasm, admin);
        let contract_addr = init_contract_inj(&wasm, admin, &router_contract_add);

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

        // Users cant deposit to the contract - auction basket empty
        let deposit_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(
                inj_amount_to_basket.amount.multiply_ratio(1u128, 2u128),
                "inj",
            )],
            user1,
        );

        assert!(deposit_response.is_err());

        assert!(
            deposit_response
                .unwrap_err()
                .to_string()
                .contains("Cannot exceed max tokens"),
            "incorrect query result error message"
        );

        // We send some inj to make it worth it
        let send_inj_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![inj_amount_to_basket.clone().into()],
            },
            admin,
        );

        assert!(send_inj_to_basket.is_ok());

        // Now users can deposit

        let deposit_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(
                inj_amount_to_basket.amount.multiply_ratio(1u128, 2u128),
                "inj",
            )],
            user1,
        );

        assert!(deposit_response.is_ok());

        let deposit_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Deposit {},
            &[Coin::new(
                inj_amount_to_basket.amount.multiply_ratio(1u128, 2u128),
                "inj",
            )],
            user2,
        );

        assert!(deposit_response.is_ok());

        // We also simulate someone else is bidding
        let random_bid_response = auction.msg_bid(
            MsgBid {
                bid_amount: Some(BidCoin {
                    amount: inj_amount_to_basket.amount.to_string(),
                    denom: "inj".to_string(),
                }),
                round: 0,
                sender: admin.address(),
            },
            admin,
        );

        assert!(random_bid_response.is_ok());

        let current_auction_response = wasm
            .query::<QueryMsg, QueryCurrentAuctionBasketResponse>(
                &contract_addr,
                &QueryMsg::CurrentAuctionBasket {},
            )
            .unwrap();

        let current_auction_round = current_auction_response.auctionRound;

        let auction_end_time = current_auction_response.auctionClosingTime;
        let current_time = app.get_block_time_seconds();

        // We set the blockchain time to auction_end_time - 20; bid should fail for time_buffer

        let time_increase = u64::try_from(auction_end_time - current_time - 20).unwrap();
        app.increase_time(time_increase);

        let try_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryBid {
                round: current_auction_round,
            },
            &[Coin::new(Uint128::one(), "inj")],
            admin,
        );

        assert!(try_bid_response.is_err());
        assert!(
            try_bid_response
                .unwrap_err()
                .to_string()
                .contains("Its not yet time buddy"),
            "incorrect query result error message"
        );

        // We now increase time of 15 more seconds so it should pass the time_buffer
        app.increase_time(15);

        // First try_bid should fail as not worth it yet
        let try_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryBid {
                round: current_auction_round,
            },
            &[],
            admin,
        );

        assert!(try_bid_response.is_err());
        assert!(
            try_bid_response
                .unwrap_err()
                .to_string()
                .contains("The next minimum bid is to high to be worth it"),
            "incorrect query result error message"
        );

        // We send some inj to make it worth it
        let send_inj_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![inj_amount_to_basket.clone().into()],
            },
            admin,
        );

        assert!(send_inj_to_basket.is_ok());

        // We also send some usdt
        let send_usdt_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![usdt_amount_to_basket.clone().into()],
            },
            admin,
        );

        assert!(send_usdt_to_basket.is_ok());

        //Increase time to get to the next auction
        app.increase_time(50);

        // We bid; should fail for wrong round
        let try_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryBid {
                round: current_auction_round,
            },
            &[Coin::new(Uint128::one(), "inj")],
            admin,
        );
        assert!(try_bid_response.is_err());
        assert!(
            try_bid_response
                .unwrap_err()
                .to_string()
                .contains("Not the active round"),
            "incorrect query result error message"
        );

        // We send some inj to make it worth it
        let send_inj_to_basket = bank.send(
            MsgSend {
                from_address: admin.address(),
                to_address: AUCTION_VAULT_ADDRESS.to_string(),
                amount: vec![inj_amount_to_basket.clone().into()],
            },
            admin,
        );
        assert!(send_inj_to_basket.is_ok());

        // Fetch new auction round ecc...
        let current_auction_response = wasm
            .query::<QueryMsg, QueryCurrentAuctionBasketResponse>(
                &contract_addr,
                &QueryMsg::CurrentAuctionBasket {},
            )
            .unwrap();

        let current_auction_round = current_auction_response.auctionRound;
        let auction_end_time = current_auction_response.auctionClosingTime;
        let current_time = app.get_block_time_seconds();

        // We set the blockchain time to auction_end_time - 5; Should pass time buffer

        let time_increase = u64::try_from(auction_end_time - current_time - 5).unwrap();
        app.increase_time(time_increase);

        // We bid; should pass
        let try_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryBid {
                round: current_auction_round,
            },
            &[],
            admin,
        );

        assert!(try_bid_response.is_ok());

        // try clear bid fails, because contract is highest bidder and current auction is still active
        let try_clear_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryClearCurrentBid {},
            &[],
            admin,
        );
        assert!(try_clear_bid_response.is_err());
        assert!(
            try_clear_bid_response
                .unwrap_err()
                .to_string()
                .contains("Contract is the highest bidder"),
            "incorrect query result error message"
        );

        // We do it again to ensure now it fails cause already the winner
        let try_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryBid {
                round: current_auction_round,
            },
            &[],
            admin,
        );

        assert!(try_bid_response.is_err());
        assert!(
            try_bid_response
                .unwrap_err()
                .to_string()
                .contains("Contract is the highest bidder"),
            "incorrect query result error message"
        );

        // Make a bid from another address
        let random_bid_respone = auction.msg_bid(
            MsgBid {
                bid_amount: Some(BidCoin {
                    amount: (100).to_string(),
                    denom: "inj".to_string(),
                }),
                round: current_auction_round,
                sender: admin.address(),
            },
            admin,
        );
        assert!(random_bid_respone.is_ok());

        let try_clear_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryClearCurrentBid {},
            &[],
            admin,
        );
        assert!(try_clear_bid_response.is_ok());

        // We bid again, should succed now
        let try_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryBid {
                round: current_auction_round,
            },
            &[],
            admin,
        );
        assert!(try_bid_response.is_ok());

        // We try to settle the last auction; should fail as its still active
        let try_settle_response =
            wasm.execute::<ExecuteMsg>(&contract_addr, &ExecuteMsg::TrySettle {}, &[], admin);

        assert!(try_settle_response.is_err());
        assert!(
            try_settle_response
                .unwrap_err()
                .to_string()
                .contains("Bid attempt round not finished"),
            "incorrect query result error message"
        );

        let current_auction_response = wasm
            .query::<QueryMsg, QueryCurrentAuctionBasketResponse>(
                &contract_addr,
                &QueryMsg::CurrentAuctionBasket {},
            )
            .unwrap();

        // print!("Auction response: {:?}", current_auction_response);

        let auction_end_time = current_auction_response.auctionClosingTime;
        let current_time = app.get_block_time_seconds();

        // We set the blockchain time to auction_end_time + 5; Should pass time buffer
        let time_increase = u64::try_from(auction_end_time - current_time + 5).unwrap();
        app.increase_time(time_increase);

        // try bid for the next auction, it should fail as not settled previous round
        let try_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryBid {
                round: current_auction_round + 1,
            },
            &[Coin::new(Uint128::one(), "inj")],
            admin,
        );
        assert!(try_bid_response.is_err());
        assert!(
            try_bid_response
                .unwrap_err()
                .to_string()
                .contains("Bid from previous needs to be settled"),
            "incorrect try bid response"
        );

        // try clear bid fails, because there is next auction active
        let try_clear_bid_response = wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::TryClearCurrentBid {},
            &[],
            admin,
        );
        assert!(try_clear_bid_response.is_err());
        assert!(
            try_clear_bid_response
                .unwrap_err()
                .to_string()
                .contains("Bid from previous needs to be settled"),
            "incorrect query result error message"
        );

        // try settle again
        let try_settle_response =
            wasm.execute::<ExecuteMsg>(&contract_addr, &ExecuteMsg::TrySettle {}, &[], admin);
        // print!("Try_settle response: {:?}", try_settle_response);

        assert!(try_settle_response.is_ok());

        let global = wasm
            .query::<QueryMsg, Global>(&contract_addr, &QueryMsg::State {})
            .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: inj_amount_to_basket.amount.multiply_ratio(1u128, 2u128),
            },
            &[],
            user1,
        )
        .unwrap();

        wasm.execute::<ExecuteMsg>(
            &contract_addr,
            &ExecuteMsg::Withdraw {
                amount: inj_amount_to_basket.amount.multiply_ratio(1u128, 2u128),
            },
            &[],
            user2,
        )
        .unwrap();

        let contract_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: contract_addr.clone(),
                denom: INJ.to_string(),
            })
            .unwrap();

        let contract_balance = Uint128::from(
            u128::from_str_radix(&contract_balance.balance.unwrap().amount, 10).unwrap(),
        );

        assert_approx_eq_uint128(contract_balance, Uint128::new(10000), 50000);

        let user1_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: user1.address(),
                denom: INJ.to_string(),
            })
            .unwrap();

        let user1_balance = Uint128::from(
            u128::from_str_radix(&user1_balance.balance.unwrap().amount, 10).unwrap(),
        );

        assert_approx_eq_uint128(
            user1_balance,
            Uint128::from(initial_inj) + global.accumulated_profit.multiply_ratio(1u128, 2u128),
            50,
        );

        let user2_balance = bank
            .query_balance(&QueryBalanceRequest {
                address: user2.address(),
                denom: INJ.to_string(),
            })
            .unwrap();

        let user2_balance = Uint128::from(
            u128::from_str_radix(&user2_balance.balance.unwrap().amount, 10).unwrap(),
        );

        assert_approx_eq_uint128(
            user2_balance,
            Uint128::from(initial_inj) + global.accumulated_profit.multiply_ratio(1u128, 2u128),
            50,
        );
    }
}
