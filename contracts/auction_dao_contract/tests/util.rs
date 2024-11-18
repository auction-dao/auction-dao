#[cfg(test)]
#[allow(dead_code)]
pub mod tests {

    use std::{env, path::PathBuf, str::FromStr};

    use auction_dao::{msg::InstantiateMsg, types::InstantiateHelixRouterMsg};
    use cosmwasm_std::{Addr, Coin, Decimal256, Uint128};
    use injective_cosmwasm::get_default_subaccount_id_for_checked_address;
    use injective_math::scale::Scaled;
    use injective_math::FPDecimal;
    use injective_std::types::injective::exchange::v1beta1::{
        MsgCreateSpotLimitOrder, MsgInstantSpotMarketLaunch, OrderInfo, OrderSide, OrderType,
        QuerySpotMarketsRequest, SpotOrder,
    };

    use injective_test_tube::{Account, Exchange, InjectiveTestApp, SigningAccount, Wasm};

    pub const ONE_18: u128 = 1_000_000_000_000_000_000u128;
    pub const ONE_6: u128 = 1_000_000u128;
    pub const USDT: &str = "usdt";
    pub const INJ: &str = "inj";
    //
    pub const AUCTION_VAULT_ADDRESS: &str = "inj1j4yzhgjm00ch3h0p9kel7g8sp6g045qf32pzlj";

    pub enum Decimals {
        Eighteen = 18,
        Six = 6,
    }
    impl Decimals {
        pub fn get_decimals(&self) -> i32 {
            match self {
                Decimals::Eighteen => 18,
                Decimals::Six => 6,
            }
        }
    }

    pub fn decimal_str_to_big_int_str(n: &str) -> String {
        Decimal256::from_str(n).unwrap().atomics().to_string()
    }

    pub fn assert_approx_eq_uint128(a: Uint128, b: Uint128, percentage_tolerance: u128) {
        // Determine the larger of the two values as the base for calculating the difference
        let max_value = if a > b { a } else { b };

        // Calculate the absolute difference
        let diff = if a > b { a - b } else { b - a };

        // Calculate the allowed tolerance as a percentage of the max_value
        let tolerance =
            max_value * Uint128::from(percentage_tolerance) / Uint128::from(1_000_000u128);

        // Check if the difference is within the tolerance
        assert!(
            diff <= tolerance,
            "Values are not approximately equal: {} vs {} with tolerance of {} millionths ({}%)",
            a,
            b,
            percentage_tolerance,
            percentage_tolerance as f64 / 10_000.0 // Convert to a readable percentage
        );
    }

    pub fn dec_to_proto(val: FPDecimal) -> String {
        val.scaled(18).to_string()
    }

    pub fn init_rich_account(app: &InjectiveTestApp) -> SigningAccount {
        app.init_account(&[
            Coin::new(1000000000 * ONE_18, "inj"),
            Coin::new(100000000000000 * ONE_6, "usdt"),
        ])
        .unwrap()
    }

    pub fn launch_custom_spot_market(
        exchange: &Exchange<InjectiveTestApp>,
        signer: &SigningAccount,
        base: &str,
        quote: &str,
        min_price_tick_size: &str,
        min_quantity_tick_size: &str,
        min_notional: &str,
    ) -> String {
        let ticker = format!("{base}/{quote}");
        exchange
            .instant_spot_market_launch(
                MsgInstantSpotMarketLaunch {
                    sender: signer.address(),
                    ticker: ticker.clone(),
                    base_denom: base.to_string(),
                    quote_denom: quote.to_string(),
                    min_price_tick_size: min_price_tick_size.to_string(),
                    min_quantity_tick_size: min_quantity_tick_size.to_string(),
                    min_notional: min_notional.to_string(),
                },
                signer,
            )
            .unwrap();

        get_spot_market_id(exchange, ticker)
    }

    pub fn get_spot_market_id(exchange: &Exchange<InjectiveTestApp>, ticker: String) -> String {
        let spot_markets = exchange
            .query_spot_markets(&QuerySpotMarketsRequest {
                status: "Active".to_string(),
                market_ids: vec![],
            })
            .unwrap()
            .markets;

        let market = spot_markets.iter().find(|m| m.ticker == ticker).unwrap();

        market.market_id.to_string()
    }

    pub fn launch_realistic_inj_usdt_spot_market(
        exchange: &Exchange<InjectiveTestApp>,
        signer: &SigningAccount,
    ) -> String {
        launch_custom_spot_market(
            exchange,
            signer,
            INJ,
            USDT,
            dec_to_proto(FPDecimal::must_from_str("0.000000000000001")).as_str(),
            dec_to_proto(FPDecimal::must_from_str("1000000000000000")).as_str(),
            dec_to_proto(FPDecimal::must_from_str("1000000")).as_str(),
        )
    }

    pub fn scale_price_quantity_for_market(
        price: &str,
        quantity: &str,
        base_decimals: &Decimals,
        quote_decimals: &Decimals,
    ) -> (String, String) {
        let price_dec = FPDecimal::must_from_str(price.replace('_', "").as_str());
        let quantity_dec = FPDecimal::must_from_str(quantity.replace('_', "").as_str());

        let scaled_price =
            price_dec.scaled(quote_decimals.get_decimals() - base_decimals.get_decimals());
        let scaled_quantity = quantity_dec.scaled(base_decimals.get_decimals());
        (dec_to_proto(scaled_price), dec_to_proto(scaled_quantity))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_realistic_limit_order(
        exchange: &Exchange<InjectiveTestApp>,
        trader: &SigningAccount,
        market_id: &str,
        order_side: OrderSide,
        price: &str,
        quantity: &str,
        base_decimals: Decimals,
        quote_decimals: Decimals,
    ) {
        let (price_to_send, quantity_to_send) =
            scale_price_quantity_for_market(price, quantity, &base_decimals, &quote_decimals);
        exchange
            .create_spot_limit_order(
                MsgCreateSpotLimitOrder {
                    sender: trader.address(),
                    order: Some(SpotOrder {
                        market_id: market_id.to_string(),
                        order_info: Some(OrderInfo {
                            subaccount_id: get_default_subaccount_id_for_checked_address(
                                &Addr::unchecked(trader.address()),
                            )
                            .to_string(),
                            fee_recipient: trader.address(),
                            price: price_to_send,
                            quantity: quantity_to_send,
                            cid: "".to_string(),
                        }),
                        order_type: if order_side == OrderSide::Buy {
                            OrderType::BuyAtomic.into()
                        } else {
                            OrderType::SellAtomic.into()
                        },
                        trigger_price: "".to_string(),
                    }),
                },
                trader,
            )
            .unwrap();
    }

    pub fn create_realistic_inj_usdt_buy_orders_from_spreadsheet(
        exchange: &Exchange<InjectiveTestApp>,
        market_id: &str,
        trader1: &SigningAccount,
    ) {
        create_realistic_limit_order(
            exchange,
            trader1,
            market_id,
            OrderSide::Buy,
            "21.217",
            "8189.001",
            Decimals::Eighteen,
            Decimals::Six,
        );

        create_realistic_limit_order(
            exchange,
            trader1,
            market_id,
            OrderSide::Buy,
            "21.202",
            "8145.65",
            Decimals::Eighteen,
            Decimals::Six,
        );

        create_realistic_limit_order(
            exchange,
            trader1,
            market_id,
            OrderSide::Buy,
            "21.198",
            "8100.607",
            Decimals::Eighteen,
            Decimals::Six,
        );
    }

    pub fn create_realistic_inj_usdt_sell_orders_from_spreadsheet(
        exchange: &Exchange<InjectiveTestApp>,
        market_id: &str,
        trader1: &SigningAccount,
    ) {
        create_realistic_limit_order(
            exchange,
            trader1,
            market_id,
            OrderSide::Sell,
            "21.217",
            "58100.001",
            Decimals::Eighteen,
            Decimals::Six,
        );
        create_realistic_limit_order(
            exchange,
            trader1,
            market_id,
            OrderSide::Sell,
            "21.214",
            "858091.001",
            Decimals::Eighteen,
            Decimals::Six,
        );
        create_realistic_limit_order(
            exchange,
            trader1,
            market_id,
            OrderSide::Sell,
            "21.210",
            "899501.001",
            Decimals::Eighteen,
            Decimals::Six,
        );
    }

    pub fn init() -> InjectiveTestApp {
        let app = InjectiveTestApp::new();

        return app;
    }

    pub fn wasm_file_path() -> PathBuf {
        // Get the current working directory
        let current_dir = env::current_dir().unwrap();
        // Construct the path to the wasm file
        let wasm_file_path = current_dir.join("../../artifacts/auction_dao_contract.wasm");

        return wasm_file_path;
    }

    pub fn router_wasm_file_path() -> PathBuf {
        // Get the current working directory
        let current_dir = env::current_dir().unwrap();
        // Construct the path to the wasm file
        let wasm_file_path = current_dir.join("../../wasmBytecodes/helix_router.wasm");

        return wasm_file_path;
    }

    pub fn init_router_contract_inj(
        wasm: &Wasm<'_, InjectiveTestApp>,
        admin: &SigningAccount,
    ) -> String {
        let router_wasm_byte_code = std::fs::read(router_wasm_file_path()).unwrap();

        let router_code_id = wasm
            .store_code(&router_wasm_byte_code, None, &admin)
            .unwrap()
            .data
            .code_id;

        let router_contract_addr = wasm
            .instantiate(
                router_code_id,
                &InstantiateHelixRouterMsg {
                    fee_recipient: admin.address(),
                    admin: admin.address(),
                },
                None,
                Some("helix_router"),
                &[],
                admin,
            )
            .unwrap()
            .data
            .address;

        return router_contract_addr;
    }

    pub fn init_contract_inj(
        wasm: &Wasm<'_, InjectiveTestApp>,
        admin: &SigningAccount,
        router_address: &String,
    ) -> String {
        let wasm_byte_code = std::fs::read(wasm_file_path()).unwrap();

        let code_id = wasm
            .store_code(&wasm_byte_code, None, &admin)
            .unwrap()
            .data
            .code_id;

        let contract_addr = wasm
            .instantiate(
                code_id,
                &InstantiateMsg {
                    admin: admin.address(),
                    accepted_denom: "inj".to_string(),
                    swap_router: router_address.to_string(),
                    bid_time_buffer: 5,
                    withdraw_time_buffer: 18000,
                    max_inj_offset_bps: Uint128::from(15000u128),
                    winning_bidder_reward_bps: Uint128::from(500u128),
                },
                None,
                Some("auction_dao_inj"),
                &[],
                admin,
            )
            .unwrap()
            .data
            .address;

        return contract_addr;
    }
}
