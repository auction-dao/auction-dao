use std::str::FromStr;

use crate::state::{read_swap_route, CONFIG};
use auction_dao::types::{
    AssetInfo, OfferAsset, RouterSimulation, RouterSimulationQuerry, RouterSimulationQuerryResponse,
};
use cosmwasm_std::{Deps, StdResult, Uint128};
use injective_cosmwasm::InjectiveQueryWrapper;

pub(crate) fn get_inj_value_asset(
    deps: Deps<InjectiveQueryWrapper>,
    source_denom: String,
    target_denom: String,
    amount: String,
) -> StdResult<Uint128> {
    if source_denom == "inj" {
        return Uint128::from_str(&amount);
    }
    let market_id = match read_swap_route(deps, &source_denom, &target_denom) {
        Ok(route) => route,
        Err(_e) => {
            /*             deps.api.debug(&format!(
                "Error, Probably market_id not registered  {:?}",
                _e
            )); */
            return Uint128::from_str(&"0");
        }
    }
    .market_id;

    let config = CONFIG.load(deps.storage)?;

    let is_cw20 = deps.api.addr_validate(&source_denom).is_ok();

    let asset_info = if is_cw20 {
        AssetInfo::Token {
            contract_addr: source_denom,
        }
    } else {
        AssetInfo::NativeToken {
            denom: source_denom,
        }
    };

    let querry_output_message = RouterSimulationQuerry {
        simulation: RouterSimulation {
            market_id: market_id.into(),
            offer_asset: OfferAsset {
                info: asset_info,
                amount,
            },
        },
    };

    let output_amount_response: RouterSimulationQuerryResponse = match deps
        .querier
        .query_wasm_smart(config.swap_router, &querry_output_message)
    {
        Ok(response) => response,
        Err(e) => {
            deps.api.debug(&format!("query_wasm_smart error: {:?}", e));

            RouterSimulationQuerryResponse::default()
        }
    };

    Ok(Uint128::from_str(&output_amount_response.return_amount)?)
}
