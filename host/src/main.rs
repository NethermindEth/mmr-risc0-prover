// use alloy::primitives::hex::encode;
use db_access::rpc::get_block_headers_in_range;
use eyre::Result;
// use garaga_rs::{
//     calldata::full_proof_with_hints::groth16::{
//         get_groth16_calldata, risc0_utils::get_risc0_vk, Groth16Proof,
//     },
//     definitions::CurveID,
// };
use methods::{METHOD_ELF, METHOD_ID};
use mmr::PeaksOptions;
pub use mmr_accumulator::{ethereum::get_finalized_block_hash, processor_utils::*, BlockHeader};
use store::SubKey;
// use risc0_ethereum_contracts::encode_seal;
// use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use risc0_zkvm::{default_prover, ExecutorEnv};
// use risc0_zkvm::compute_image_id;
use serde::{Deserialize, Serialize};
use starknet_crypto::Felt;
// use starknet_handler::verify_groth16_proof_onchain;
use tokio::task;
use tracing_subscriber;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CombinedInput {
    headers: Vec<BlockHeader>,
    mmr_input: GuestInput,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GuestInput {
    initial_peaks: Vec<String>,
    elements_count: usize,
    leaves_count: usize,
    new_elements: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GuestOutput {
    final_peaks: Vec<String>,
    elements_count: usize,
    leaves_count: usize,
    append_results: Vec<AppendResult>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppendResult {
    leaves_count: usize,
    elements_count: usize,
    element_index: usize,
    root_hash: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    let (finalized_block_number, _finalized_block_hash) = get_finalized_block_hash().await?;

    // Initialize MMR
    let db_file = 0;
    let current_dir = ensure_directory_exists("db-instances")?;
    let store_path = create_database_file(&current_dir, db_file)?;
    let (store_manager, mut _mmr, pool) = initialize_mmr(&store_path).await?;

    let start_block = finalized_block_number.saturating_sub(15);

    let headers = get_block_headers_in_range(start_block, finalized_block_number)
        .await
        .unwrap();

    // Get current MMR state
    let current_peaks = _mmr.get_peaks(PeaksOptions::default()).await?;
    let current_elements_count = _mmr.elements_count.get().await?;
    let current_leaves_count = _mmr.leaves_count.get().await?;

    let mmr_input = GuestInput {
        initial_peaks: current_peaks.clone(),
        elements_count: current_elements_count,
        leaves_count: current_leaves_count,
        new_elements: headers.iter().map(|h| h.block_hash.clone()).collect(),
    };

    let combined_input = CombinedInput {
        headers: headers.clone(),
        mmr_input,
    };

    let (_calldata, receipt) =
        task::spawn_blocking(move || run_blocking_tasks(&combined_input)).await??;

    receipt.verify(METHOD_ID).unwrap();

    // let result = verify_groth16_proof_onchain(&calldata)
    //     .await
    //     .map_err(|e| eyre::eyre!(e.to_string()))?;

    // println!("Result: {:?}", result);

    if true {
        // Decode the guest output from the receipt
        let guest_output: GuestOutput = receipt.journal.decode()?;

        // Verify the MMR state transition
        if guest_output.elements_count < current_elements_count {
            return Err(eyre::eyre!(
                "Invalid MMR state transition: elements count decreased"
            ));
        }

        // Update MMR state with the verified results
        for result in &guest_output.append_results {
            store_manager
                .insert_value_index_mapping(&pool, &result.root_hash, result.element_index)
                .await?;
        }

        // Update MMR state
        for (idx, hash) in guest_output.final_peaks.iter().enumerate() {
            _mmr.hashes.set(hash, SubKey::Usize(idx)).await?;
        }
        _mmr.elements_count.set(guest_output.elements_count).await?;
        _mmr.leaves_count.set(guest_output.leaves_count).await?;
    }

    Ok(())
}

fn run_blocking_tasks(input: &CombinedInput) -> Result<(Vec<Felt>, risc0_zkvm::Receipt)> {
    let env = ExecutorEnv::builder()
        .write(&input)
        .unwrap()
        .build()
        .unwrap();

    let receipt = default_prover().prove(env, METHOD_ELF).unwrap().receipt;

    // let receipt = default_prover()
    //     .prove_with_ctx(
    //         env,
    //         &VerifierContext::default(),
    //         METHOD_ELF,
    //         &ProverOpts::groth16(),
    //     )
    //     .map_err(|e| eyre::eyre!(e.to_string()))?
    //     .receipt;

    // let encoded_seal = encode_seal(&receipt).map_err(|e| eyre::eyre!(e.to_string()))?;
    // println!("Solidity Encoded Seal: 0x{}", encode(encoded_seal.clone()));

    // let journal = receipt.journal.bytes.clone();
    // println!("Journal: 0x{}", encode(journal.clone()));

    // let image_id = compute_image_id(&METHOD_ELF).unwrap();
    // println!("Image ID: 0x{}", encode(image_id));

    // let proof = Groth16Proof::from_risc0(encoded_seal, image_id.as_bytes().to_vec(), journal);

    // let calldata = get_groth16_calldata(&proof, &get_risc0_vk(), CurveID::BN254)
    //     .map_err(|e| eyre::eyre!(e.to_string()))?;

    Ok((vec![], receipt))
}

// Remove the MMRUpdates trait as we're directly using the MMR methods now

#[cfg(test)]
mod tests {
    // use std::path::PathBuf;

    use super::*;
    use block_validity::utils::are_blocks_and_chain_valid;
    use eyre::Result;
    use tempfile;

    #[tokio::test]
    async fn test_mmr_state_transition() -> Result<()> {
        // Create a temporary directory with proper permissions
        let test_dir = tempfile::tempdir()?;
        let db_dir = test_dir.path().join("db-instances");
        std::fs::create_dir_all(&db_dir)?;

        // Create the database file first to ensure proper permissions
        let store_path = db_dir.join("test.db");
        if let Some(parent) = store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Touch the file to ensure it exists with proper permissions
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&store_path)?;

        // Initialize MMR with the prepared database
        let (_store_manager, mmr, _pool) = initialize_mmr(store_path.to_str().unwrap())
            .await
            .map_err(|e| eyre::eyre!("Failed to initialize MMR: {}", e))?;

        let test_headers = fetch_test_headers(17034870, 2).await?;

        let initial_peaks = mmr.get_peaks(PeaksOptions::default()).await?;
        let initial_elements_count = mmr.elements_count.get().await?;
        let initial_leaves_count = mmr.leaves_count.get().await?;

        let mmr_input = GuestInput {
            initial_peaks: initial_peaks.clone(),
            elements_count: initial_elements_count,
            leaves_count: initial_leaves_count,
            new_elements: test_headers.iter().map(|h| h.block_hash.clone()).collect(),
        };

        let combined_input = CombinedInput {
            headers: test_headers,
            mmr_input,
        };

        // Run the blocking task in a separate thread
        let (_calldata, receipt) =
            tokio::task::spawn_blocking(move || run_blocking_tasks(&combined_input)).await??;

        receipt.verify(METHOD_ID).unwrap();

        // Keep the TempDir alive until the end of the test
        std::mem::drop(test_dir);

        Ok(())
    }


    #[tokio::test]
    async fn test_full_workflow() -> Result<()> {
        // Setup database
        let test_dir = tempfile::tempdir()?;
        let db_dir = test_dir.path().join("db-instances");
        std::fs::create_dir_all(&db_dir)?;
        let store_path = db_dir.join("test.db");

        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&store_path)?;

        let (store_manager, mut mmr, pool) = initialize_mmr(store_path.to_str().unwrap()).await?;

        let test_headers = fetch_test_headers(17034870, 8).await?;

        let initial_peaks = mmr.get_peaks(PeaksOptions::default()).await?;
        let initial_elements_count = mmr.elements_count.get().await?;
        let initial_leaves_count = mmr.leaves_count.get().await?;

        let mmr_input = GuestInput {
            initial_peaks,
            elements_count: initial_elements_count,
            leaves_count: initial_leaves_count,
            new_elements: test_headers.iter().map(|h| h.block_hash.clone()).collect(),
        };

        let combined_input = CombinedInput {
            headers: test_headers.clone(),
            mmr_input,
        };

        let (_calldata, receipt) =
            tokio::task::spawn_blocking(move || run_blocking_tasks(&combined_input)).await??;

        receipt.verify(METHOD_ID).unwrap();

        let guest_output: GuestOutput = receipt.journal.decode()?;

        // Store the append results for verification
        let mut append_results = Vec::new();

        // Update MMR state with the block hashes
        for (_, header) in test_headers.iter().enumerate() {
            // First append to MMR to get the index
            let append_result = mmr.append(header.block_hash.clone()).await?;
            append_results.push(append_result.clone());

            // Then update the store mapping
            store_manager
                .insert_value_index_mapping(&pool, &header.block_hash, append_result.element_index)
                .await?;
        }

        let current_peaks = mmr.get_peaks(PeaksOptions::default()).await?;
        let current_elements_count = mmr.elements_count.get().await?;
        let current_leaves_count = mmr.leaves_count.get().await?;

        // Verify each block was properly added
        for (i, header) in test_headers.iter().enumerate() {
            let append_result = &append_results[i];

            let stored_index = store_manager
                .get_value_for_element_index(&pool, append_result.element_index)
                .await?;

            assert!(
                stored_index.is_some(),
                "Block {} should be stored in MMR at index {}",
                i,
                append_result.element_index
            );

            if let Some(stored_value) = stored_index {
                assert_eq!(
                    stored_value, header.block_hash,
                    "Stored hash doesn't match block hash for block {}",
                    i
                );
            }
        }

        // Verify final peaks match guest output
        assert_eq!(
            current_peaks, guest_output.final_peaks,
            "Final peaks don't match"
        );
        assert_eq!(
            current_elements_count, guest_output.elements_count,
            "Elements count doesn't match"
        );
        assert_eq!(
            current_leaves_count, guest_output.leaves_count,
            "Leaves count doesn't match"
        );

        Ok(())
    }

    async fn fetch_test_headers(start_block: u64, count: usize) -> Result<Vec<BlockHeader>> {
        let end_block = start_block + count as u64 - 1;
        get_block_headers_in_range(start_block, end_block)
            .await
            .map_err(|e| eyre::eyre!("Failed to fetch headers: {}", e))
    }

    #[tokio::test]
    async fn test_block_validation() -> Result<()> {
        let test_headers = fetch_test_headers(17034870, 2).await?;
        assert!(
            are_blocks_and_chain_valid(&test_headers),
            "Test headers should form valid chain"
        );
        Ok(())
    }
}
