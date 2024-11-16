use auction_dao::{
    error::ContractError,
    state::{BidAttempt, Config, Global, SwapRoute, UserAccount},
};
use cosmwasm_std::{Deps, DepsMut, Order, StdResult, Storage};
use cw_storage_plus::{Item, Map};
use injective_cosmwasm::InjectiveQueryWrapper;

pub const CONFIG: Item<Config> = Item::new("config");
pub const GLOBAL: Item<Global> = Item::new("global");
pub const BID_ATTEMPT: Item<BidAttempt> = Item::new("bid_attempt");
pub const BID_ATTEMPT_TRANSIENT: Item<BidAttempt> = Item::new("bid_attempt");
pub const USER_ACCOUNTS: Map<&str, UserAccount> = Map::new("user_accounts");
pub const SWAP_ROUTES: Map<(String, String), SwapRoute> = Map::new("swap_routes");

pub fn store_swap_route(storage: &mut dyn Storage, route: &SwapRoute) -> StdResult<()> {
    let key = route_key(&route.source_denom, &route.target_denom);
    SWAP_ROUTES.save(storage, key, route)
}

pub fn read_swap_route(
    deps: Deps<InjectiveQueryWrapper>,
    source_denom: &str,
    target_denom: &str,
) -> Result<SwapRoute, ContractError> {
    let key = route_key(source_denom, target_denom);
    SWAP_ROUTES.load(deps.storage, key).map_err(|_| {
        ContractError::NoSwapRouteFound(source_denom.to_string(), target_denom.to_string())
    })
}

pub fn get_all_swap_routes(deps: Deps<InjectiveQueryWrapper>) -> StdResult<Vec<SwapRoute>> {
    let routes = SWAP_ROUTES
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| item.unwrap().1)
        .collect();

    Ok(routes)
}

pub fn remove_swap_route(
    deps: DepsMut<InjectiveQueryWrapper>,
    source_denom: &str,
    target_denom: &str,
) {
    let key = route_key(source_denom, target_denom);
    SWAP_ROUTES.remove(deps.storage, key)
}

fn route_key<'a>(source_denom: &'a str, target_denom: &'a str) -> (String, String) {
    if source_denom < target_denom {
        (source_denom.to_string(), target_denom.to_string())
    } else {
        (target_denom.to_string(), source_denom.to_string())
    }
}
