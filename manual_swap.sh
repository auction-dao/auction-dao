#!/bin/bash
[ -f .env ] && export $(grep -v '^#' .env | xargs)

echo "chain_id:" $CHAIN_ID
echo "rpc:" $RPC
echo "admin and deployer: $ADMIN"
echo "contract: $CONTRACT_ADDR"

if [ -z "$CONTRACT_ADDR" ]; then
    echo "CONTRACT_ADDR is not set"
    exit 1
fi

AMOUNT=1000000000 # 1000 USDT
ASSET="peggy0x87aB3B4C8661e07D6372361211B96ed4Dc36B1B5" 
MARKET_ID="0x0611780ba69656949525013d947713300f56c37b6175e02f26bffa495c3208fe"

BALANCE=$(injectived query bank balances \
    $CONTRACT_ADDR \
    --chain-id ${CHAIN_ID} --node ${RPC} \
    -o json | jq -r '.balances[] | select(.denom=="'$ASSET'") | .amount')

echo "contract balance before transfer: $BALANCE$ASSET"

TX_HASH=$(echo $KEYPASSWD | \
    injectived tx bank send \
        "$ADMIN" "$CONTRACT_ADDR" "${AMOUNT}${ASSET}" \
        --chain-id ${CHAIN_ID} --node ${RPC} \
        --gas-prices 500000000inj --gas auto --gas-adjustment 1.5 \
        -o json -y \
    | jq '.txhash' -r)
    
echo "funds sent to contract, tx hash ${TX_HASH}"

sleep 3

MSG=$(cat <<-END
    {
        "manual_exchange_swap": {
            "amount": "${AMOUNT}",
            "market_id": "${MARKET_ID}",
            "asset": "${ASSET}"
        }
    }
END
)


TX_HASH=$(echo $KEYPASSWD | \
    injectived tx wasm execute \
        "$CONTRACT_ADDR" "$MSG" \
        --chain-id ${CHAIN_ID} --node ${RPC} \
        --from $ADMIN \
        --gas-prices 500000000inj --gas auto --gas-adjustment 1.5 \
        -o json -y \
    | jq '.txhash' -r)
    
echo "manual swap executed, tx hash ${TX_HASH}"