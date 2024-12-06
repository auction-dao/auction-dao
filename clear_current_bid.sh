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

MSG=$(cat <<-END
    {
        "try_clear_current_bid": {
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
    
echo "try clear current bid executed, tx hash ${TX_HASH}"