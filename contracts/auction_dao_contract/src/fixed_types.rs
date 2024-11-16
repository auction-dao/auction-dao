// following injective-cosmwasm types not working with serde
// SpotMarket -> status, admin_permissions are not properly deserialized and throw error

use injective_std_derive::CosmwasmExt;

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(
    Clone,
    PartialEq,
    Eq,
    ::prost::Message,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::schemars::JsonSchema,
    CosmwasmExt,
)]
#[proto_message(type_url = "/injective.exchange.v1beta1.QuerySpotMarketResponse")]
pub struct QuerySpotMarketResponse {
    #[prost(message, optional, tag = "1")]
    pub market: ::core::option::Option<SpotMarket>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(
    Clone,
    PartialEq,
    Eq,
    ::prost::Message,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::schemars::JsonSchema,
    CosmwasmExt,
)]
#[proto_message(type_url = "/injective.exchange.v1beta1.SpotMarket")]
pub struct SpotMarket {
    /// A name of the pair in format AAA/BBB, where AAA is base asset, BBB is quote
    /// asset.
    #[prost(string, tag = "1")]
    pub ticker: ::prost::alloc::string::String,
    /// Coin denom used for the base asset
    #[prost(string, tag = "2")]
    pub base_denom: ::prost::alloc::string::String,
    /// Coin used for the quote asset
    #[prost(string, tag = "3")]
    pub quote_denom: ::prost::alloc::string::String,
    /// maker_fee_rate defines the fee percentage makers pay when trading
    #[prost(string, tag = "4")]
    pub maker_fee_rate: ::prost::alloc::string::String,
    /// taker_fee_rate defines the fee percentage takers pay when trading
    #[prost(string, tag = "5")]
    pub taker_fee_rate: ::prost::alloc::string::String,
    /// relayer_fee_share_rate defines the percentage of the transaction fee shared
    /// with the relayer in a derivative market
    #[prost(string, tag = "6")]
    pub relayer_fee_share_rate: ::prost::alloc::string::String,
    /// Unique market ID.
    #[prost(string, tag = "7")]
    #[serde(alias = "marketID")]
    pub market_id: ::prost::alloc::string::String,
    /// Status of the market
    #[prost(string, tag = "8")]
    pub status: ::prost::alloc::string::String,
    /// min_price_tick_size defines the minimum tick size that the price required
    /// for orders in the market
    #[prost(string, tag = "9")]
    pub min_price_tick_size: ::prost::alloc::string::String,
    /// min_quantity_tick_size defines the minimum tick size of the quantity
    /// required for orders in the market
    #[prost(string, tag = "10")]
    pub min_quantity_tick_size: ::prost::alloc::string::String,
    /// min_notional defines the minimum notional (in quote asset) required for
    /// orders in the market
    #[prost(string, tag = "11")]
    pub min_notional: ::prost::alloc::string::String,
    /// current market admin
    #[prost(string, tag = "12")]
    pub admin: ::prost::alloc::string::String,
    /// level of admin permissions
    #[prost(uint32, tag = "13")]
    pub admin_permissions: u32,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(
    Clone,
    PartialEq,
    Eq,
    ::prost::Message,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::schemars::JsonSchema,
    CosmwasmExt,
)]
#[proto_message(type_url = "/injective.exchange.v1beta1.QueryExchangeParamsResponse")]
pub struct QueryExchangeParamsResponse {
    #[prost(message, optional, tag = "1")]
    pub params: ::core::option::Option<Params>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(
    Clone,
    PartialEq,
    Eq,
    ::prost::Message,
    ::serde::Serialize,
    ::serde::Deserialize,
    ::schemars::JsonSchema,
    CosmwasmExt,
)]
#[proto_message(type_url = "/injective.exchange.v1beta1.Params")]
pub struct Params {
    #[prost(string, tag = "20")]
    pub spot_atomic_market_order_fee_multiplier: ::prost::alloc::string::String,
}
