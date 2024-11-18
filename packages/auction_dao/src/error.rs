use std::{num::TryFromIntError, str::Utf8Error};

use cosmwasm_std::{Decimal256RangeExceeded, DecimalRangeExceeded, StdError};
use prost::{DecodeError, EncodeError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Insufficient funds")]
    InsufficientFunds {},

    #[error("Invalid denom")]
    InvalidDenom {},

    #[error("Decimal range exceeded")]
    DecimalRangeExceeded {},

    #[error("Decimal256 range exceeded")]
    Decimal256RangeExceeded {},

    #[error("Not the active round")]
    WrongRound {},

    #[error("Cannot exceed max tokens")]
    MaxTokensExceeded {},

    #[error("Market not found")]
    MarketNotFound {},

    #[error("Asset not found")]
    AssetNotFound {},

    #[error("Not implemented")]
    NotImplemented {},

    #[error("Not enough liquidity")]
    NotEnoughLiquidity {},

    #[error("Exchange params not found")]
    ExchangeParamsNotFound {},

    #[error("Auction params not found")]
    AuctionParamsNotFound {},

    #[error("Last auction result not found")]
    LastAuctionResultNotFound {},

    #[error("Contract is the highest bidder")]
    AlreadyHighestBidder {},

    #[error("No bid attempt found")]
    BidAttemptNotFound {},

    #[error("Bid from previous needs to be settled")]
    UnsettledPreviousBid {},

    #[error("Invalid asset for direct swap")]
    BidAttemptAlreadyFinished {},

    #[error("No swap route not found from {0} to {1}")]
    NoSwapRouteFound(String, String),

    #[error("Cannot manually swap deposit asset")]
    CannotManuallySwap {},

    #[error("Invalid reply from sub-message {id}, {err}")]
    ReplyParseFailure { id: u64, err: String },

    #[error("Bid attempt round not finished: {0} != {1} (last result round)")]
    BidAttemptRoundNotFinished(u64, u64),

    #[error("Failure response from submsg: {0}")]
    SubMsgFailure(String),

    #[error("Invalid reply: {0}")]
    InvalidReply(u64),

    #[error("The next minimum bid is to high to be worth it")]
    MinBidToHigh(),

    #[error("Its not yet time buddy")]
    NotInBidTime {},

    #[error("Withdraw is disabled {0} minutes before the auctions end (the auction ends in {1} minutes)")]
    NotInWithdrawTime(u64, u64),

    #[error("Custom Error: {val:?}")]
    CustomError { val: String },
}

impl From<TryFromIntError> for ContractError {
    fn from(_: TryFromIntError) -> Self {
        ContractError::Std(StdError::generic_err("Invalid number"))
    }
}

// Implement From<DecimalRangeExceeded> for ContractError
impl From<DecimalRangeExceeded> for ContractError {
    fn from(_: DecimalRangeExceeded) -> Self {
        ContractError::DecimalRangeExceeded {}
    }
}

impl From<Decimal256RangeExceeded> for ContractError {
    fn from(_: Decimal256RangeExceeded) -> Self {
        ContractError::Decimal256RangeExceeded {}
    }
}

impl From<Utf8Error> for ContractError {
    fn from(_: Utf8Error) -> Self {
        ContractError::Std(StdError::generic_err("Invalid UTF-8 string"))
    }
}

impl From<EncodeError> for ContractError {
    fn from(err: EncodeError) -> Self {
        ContractError::Std(StdError::generic_err(err.to_string()))
    }
}

impl From<DecodeError> for ContractError {
    fn from(err: DecodeError) -> Self {
        ContractError::Std(StdError::generic_err(err.to_string()))
    }
}
