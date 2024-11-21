[ -f .env ] && export $(grep -v '^#' .env | xargs)

echo "chain_id:" $CHAIN_ID
echo "rpc:" $RPC

echo "admin and deployer: ${ADMIN}"
echo "storing ${ARTIFACT}"
TX_HASH=$(echo $KEYPASSWD | injectived --chain-id ${CHAIN_ID} --node ${RPC} \
    tx wasm store ${ARTIFACT} \
    --from ${ADMIN} \
    --gas-prices 160000000inj \
    --gas auto \
    --gas-adjustment 1.3 \
    -o json -y | jq '.txhash' -r)

echo "storing artifacts tx hash: ${TX_HASH}"

sleep 3

CODE_ID=$(injectived query tx "$TX_HASH" -o json --chain-id ${CHAIN_ID} --node ${RPC} | \
  jq '.events[] | select(.type=="cosmwasm.wasm.v1.EventCodeStored") | .attributes[] | select(.key=="code_id") | .value' -r)
CODE_ID=${CODE_ID:1:-1}

echo "contract id: ${CODE_ID}"

# HELIX_ROUTER_INSTANTIATE_MSG
# INSTANTIATE_MSG=$(cat <<-END
#     {
#       "admin": "${ADMIN}",
#       "fee_recipient": "${ADMIN}"
#     }
# END
# )



# Check if CONTRACT_ADDR exists
if [ -n "$CONTRACT_ADDR" ]; then
    MIGRATE_MSG=$(cat <<-END
    {}
END
)

    echo "CONTRACT_ADDR exists: $CONTRACT_ADDR"
    # Perform actions if CONTRACT_ADDR exists
    TX_HASH=$(echo $KEYPASSWD | injectived tx wasm migrate \
      $CONTRACT_ADDR $CODE_ID "$MIGRATE_MSG" --from $ADMIN --gas-prices 160000000inj \
      --gas auto --gas-adjustment 1.3 -o json -y | jq '.txhash' -r)
    echo "contract migrated, tx hash ${TX_HASH}"
else
    INSTANTIATE_MSG=$(cat <<-END
        {
          "admin": "${ADMIN}",
          "accepted_denom": "inj",
          "swap_router": "${HELIX_ROUTER}",
          "bid_time_buffer": 5,
          "withdraw_time_buffer": 18000,
          "max_inj_offset_bps": "15000",
          "winning_bidder_reward_bps": "500"
        }
END
)
    echo "CONTRACT_ADDR does not exist"
    # Perform actions if CONTRACT_ADDR does not exist
    echo "instantiating the code id ${CODE_ID}"
    TX_HASH=$(echo $KEYPASSWD | injectived tx wasm instantiate \
      $CODE_ID "$INSTANTIATE_MSG" --from $ADMIN --admin $ADMIN \
      --gas-prices 160000000inj --gas auto --gas-adjustment 1.3 \
      --label "$LABEL" -o json -y | jq '.txhash' -r)
    echo "contract instantiated, tx hash ${TX_HASH}"

    sleep 3

    CONTRACT_ADDR=$(injectived query tx "$TX_HASH" -o json | \
      jq 'last(.events[] | .attributes[] | select(.key=="contract_address") | .value)' -r)
    CONTRACT_ADDR=${CONTRACT_ADDR:1:-1}
    echo "contract address:" $CONTRACT_ADDR
fi