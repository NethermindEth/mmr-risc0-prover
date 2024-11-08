use std::env;

use anyhow::Result;
use dotenv::dotenv;
use starknet::{
    core::{
        types::{BlockId, BlockTag, FunctionCall},
        utils::get_selector_from_name,
    },
    providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider, Url},
};
use starknet_crypto::Felt;
use tracing::info;

pub async fn verify_groth16_proof_onchain(calldata: &Vec<Felt>) -> Result<Vec<Felt>> {
    dotenv().ok();

    let rpc_url =
        env::var("STARKNET_RPC_URL").expect("STARKNET_RPC_URL should be provided as env vars.");

    let provider = JsonRpcClient::new(HttpTransport::new(
        Url::parse(&rpc_url).expect("Invalid rpc url provided"),
    ));

    let verifier_address_from_env =
        env::var("VERIFIER_ADDRESS").expect("VERIFIER_ADDRESS should be provided as env vars.");
    let contract_address =
        Felt::from_hex(&verifier_address_from_env).expect("Invalid verifier address provided");
    info!("contract_address: {:?}", contract_address);

    let result = provider
        .call(
            FunctionCall {
                contract_address,
                entry_point_selector: get_selector_from_name("verify_groth16_proof_bn254").unwrap(),
                calldata: calldata.clone(),
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await
        .expect("failed to call contract");

    Ok(result)
}
