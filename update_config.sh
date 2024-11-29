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
        "update_config": {
            "new_config": {
                "admin": "${ADMIN}",
                "accepted_denom": "inj",
                "swap_router": "${HELIX_ROUTER}",
                "bid_time_buffer": 5,
                "withdraw_time_buffer": 7200,
                "max_inj_offset_bps": "12500",
                "winning_bidder_reward_bps": "500"
            }
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
    
echo "config update executed, tx hash ${TX_HASH}"