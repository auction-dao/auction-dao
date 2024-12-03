#!/bin/bash
[ -f .env ] && export $(grep -v '^#' .env | xargs)

echo "chain_id:" $CHAIN_ID
echo "rpc:" $RPC

echo "admin and deployer: ${ADMIN}"
echo "gov ${ARTIFACT}"

PROPOSAL_MSG=$(cat <<-END
We propose to store the Auction DAO smart contract code on the Injective blockchain.
Auction DAO is an open-source decentralized application that enables permissionless bidding in the weekly Injective burn auctions. 
Anyone can join by depositing their INJ tokens into the smart contract, which collectively bids on behalf of all participants.
This approach maximizes potential profits through strategic, last-second bidding and allows users to participate without barriers.


Key Features:
=============

Transparent and Open Source: The project is fully open-sourced on GitHub, promoting transparency and community collaboration.
You can find the repository here: https://github.com/auction-dao/auction-dao

Tested and Ready: The smart contract has been thoroughly tested using Injective's testing tools, including injective-test-tube.
It has been deployed on both private and public testnets to ensure reliability and security. Please follow https://dev.auctiondao.bid/

Permissionless Participation: The platform is open to all users.
Anyone can deposit INJ tokens and become part of the collective bidding process without any restrictions.

Collective Bidding Power: By pooling resources, participants increase their chances of winning auctions and maximizing profits.

Strategic Last-Second Bidding: The smart contract submits bids in the final moments of the auction to minimize exposure to price volatility and reduce the risk of overbidding.

Incentive Mechanism: Sending the bid transaction is permissionless, and if the DAO wins the auction, 5% of the profit is rewarded to the sender of the winning bid transaction, encouraging community participation.

Basket Sell via Exchange Module: After winning an auction, the acquired non-INJ asset basket is sold using the Exchange Module.
This ensures efficient and secure swapping of assets back into INJ tokens, leveraging the exchange's capabilities.


Benefits to the Injective Ecosystem:
===============================

Enhanced Accessibility: Auction DAO lowers the barriers to entry for participating in the Injective burn auctions, allowing more users to engage in the process.

Community Empowerment: The permissionless and open nature of the platform fosters greater community involvement and collaboration.

We kindly request the community's support in approving this proposal to store the Auction DAO smart contract code on-chain.
By doing so, we enable a permissionless, community-driven platform that enhances participation in the Injective ecosystem.

Thank you for your consideration.
Auction DAO Team
https://auctiondao.bid/

END
)


TX_HASH=$(echo $KEYPASSWD | injectived --chain-id ${CHAIN_ID} --node ${RPC} \
    tx wasm submit-proposal wasm-store ${ARTIFACT} \
    --title="Store Auction DAO Smart Contract Code" \
    --summary="${PROPOSAL_MSG}" \
    --instantiate-anyof-addresses "${ADMIN}" \
    --broadcast-mode=sync \
    --from ${ADMIN} \
    --deposit 50000000000000000000inj \
    --gas-prices 500000000inj \
    --gas auto \
    --gas-adjustment 1.3 \
    -o json -y | jq '.txhash' -r)

echo "gov proposal tx hash: ${TX_HASH}"
