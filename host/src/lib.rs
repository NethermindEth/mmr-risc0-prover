pub mod accumulator;
pub mod proof_generator;
pub mod types;

pub use accumulator::AccumulatorBuilder;
use eyre::Result;
use methods::{MMR_GUEST_ELF, MMR_GUEST_ID};
use mmr_accumulator::BlockHeader;
pub use proof_generator::{ProofGenerator, ProofType};
use starknet_crypto::Felt;
use starknet_handler::verify_groth16_proof_onchain;
use tracing::info;

pub async fn update_mmr_and_verify_onchain(
    db_file: &str,                 // Path to the existing SQLite database file
    new_headers: Vec<BlockHeader>, // New block headers to update the MMR
    rpc_url: &str,                 // RPC URL for Starknet
    verifier_address: &str,        // Verifier contract address
) -> Result<(bool, String)> {
    // Initialize proof generator
    let proof_generator = ProofGenerator::new(MMR_GUEST_ELF, MMR_GUEST_ID);

    // Initialize accumulator builder
    let mut builder = AccumulatorBuilder::new(db_file, proof_generator, 0).await?;

    info!("MMR successfully updated with new block headers");

    // Update the MMR with new block headers and get the proof calldata
    let (proof_calldata, new_mmr_root_hash) =
        builder.update_mmr_with_new_headers(new_headers).await?;

    // Call the verification function with the provided RPC URL and verifier address
    let result = verify_groth16_proof_onchain(rpc_url, verifier_address, &proof_calldata);

    let verification_result = result.await.expect("Failed to verify final Groth16 proof");

    let verified = verification_result[0] == Felt::from(1);

    Ok((verified, new_mmr_root_hash))
}
