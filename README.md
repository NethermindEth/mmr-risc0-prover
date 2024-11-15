<readme>

# Fossil Light Client Local Testing Setup

This README provides instructions for setting up and running the `fossil-light-client` for local testing. These steps will guide you through configuring a simulated Ethereum environment (Anvil), deploying necessary contracts, and initializing a local Starknet development network (Katana) for integrated testing. Each section corresponds to a separate terminal session to keep services organized and running simultaneously.

## Terminal 1: Start Anvil Ethereum Devnet

In this terminal, you'll set up an Ethereum development environment using Anvil, which will simulate an Ethereum network locally.

1. Navigate to the Ethereum directory:
   ```bash
   cd config
   ```

2. Load the environment variables:
   ```bash
   source anvil.env
   ```

3. Start the Anvil Ethereum development network:
   ```bash
   anvil --fork-url $MAINNET_ETH_RPC_URL --auto-impersonate
   ```

> **Note:** `${MAINNET_ETH_RPC_URL}` should be configured in `anvil.env` to point to the desired RPC provider (e.g., Infura or Alchemy) for forking mainnet data.

## Terminal 2: Deploy L1MessageSender.sol

In this terminal, you will deploy the `L1MessageSender.sol` contract to the Anvil development network, which is essential for message relaying between Ethereum and Starknet in this testing setup.

1. Navigate to the Ethereum directory:
   ```bash
   cd contracts/ethereum
   ```

2. Load the environment variables:
   ```bash
   cp ../../config/anvil.env .env
   source .env
   ```

3. Deploy the contract:
   ```bash
   forge script script/LocalTesting.s.sol:LocalSetup --broadcast --rpc-url $ANVIL_URL
   ```

> **Note:** This deployment requires `forge` and should be configured to point to the `ANVIL_URL` as specified in `anvil.env`.

## Terminal 3: Start Katana Starknet Devnet and Deploy Contracts

In this terminal, you'll initialize Katana, a local Starknet development environment. Katana will work in tandem with Anvil for cross-chain interactions in your testing setup.

1. Navigate to the Starknet directory:
   ```bash
   cd scripts/katana
   ```

2. Source the environment variables:
   ```bash
   source ../../config/katana.env
   ```
3. Update the `anvil.messaging.json` file with the correct values for `from_block` taken from the Anvil logs.
   ```
   Fork
   ==================
   Endpoint:       http://xxx.x.x.x:x
   Block number:   21168847 <---
   Block hash:     0x67bc863205b5cd53f11d78bccb7a722db1b598bb24f4e11239598825bfb3e4d3
   Chain ID:       1
   ```

4. Start Katana with messaging integration for Anvil:
   ```bash
   katana --messaging ../../config/anvil.messaging.json --disable-fee
   ```

> **Note:** `--messaging` enables communication between Anvil and Katana, and `--disable-fee` allows for testing without transaction fees.

## Terminal 4: Deploy Starknet Contracts

In this terminal, you will deploy all necessary Starknet contracts to the Katana development network.

1. Navigate to the Starknet directory:
   ```bash
   cd scripts/katana
   ```

2. Run the deployment script:
   ```bash
   ./deploy.sh
   ```

> **Note:** Ensure the `deploy.sh` script is configured correctly to deploy the required contracts for testing on Katana.
>

## Terminal 5: Send Finalized Block Hash to L2

In this terminal, you will send the finalized block hash from the Ethereum network to the Starknet network.

1. Navigate to the Ethereum directory:
   ```bash
   cd contracts/ethereum
   source .env
   ```

2. Send the finalized block hash to the Starknet network:
   ```bash
   forge script script/SendMessage.s.sol:FinalizedBlockHash --broadcast --rpc-url $ANVIL_URL
   ```

## Next Steps

Once all terminals are set up, your local testing environment should be fully operational. You can now proceed with testing cross-chain messaging or other interactions between the simulated Ethereum and Starknet environments. 

For further customization or troubleshooting, refer to individual configuration files and environment variables in each service directory.

</readme>