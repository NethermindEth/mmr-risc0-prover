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
use store::SubKey;
pub use mmr_accumulator::{
    ethereum::get_finalized_block_hash,
    BlockHeader,
    processor_utils::*,
};
// use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
// use risc0_zkvm::compute_image_id;
use serde::{Serialize, Deserialize};
use starknet_crypto::Felt;
// use starknet_handler::verify_groth16_proof_onchain;
use tokio::task;
use tracing_subscriber;

#[derive(Debug, Serialize, Deserialize)]
struct CombinedInput {
    headers: Vec<BlockHeader>,
    mmr_input: GuestInput,
}

#[derive(Debug, Serialize, Deserialize)]
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
    println!("Headers Length: {:?}", headers.len());

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

    let (_calldata, receipt) = task::spawn_blocking(move || {
        run_blocking_tasks(&combined_input)
    })
    .await??;

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
            return Err(eyre::eyre!("Invalid MMR state transition: elements count decreased"));
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
    println!("Initial peaks: {:?}", input.mmr_input.initial_peaks);
    println!("Initial elements count: {}", input.mmr_input.elements_count);
    println!("Initial leaves count: {}", input.mmr_input.leaves_count);

    let env = ExecutorEnv::builder()
        .write(&input)
        .unwrap()
        .build()
        .unwrap();
    println!("Env Set");

    let receipt = default_prover()
        .prove_with_ctx(
            env,
            &VerifierContext::default(),
            METHOD_ELF,
            &ProverOpts::groth16(),
        )
        .map_err(|e| eyre::eyre!(e.to_string()))?
        .receipt;

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
    use super::*;
    use block_validity::utils::are_blocks_and_chain_valid;

    #[tokio::test]
    async fn test_mmr_state_transition() -> Result<()> {
        // Setup test environment
        let test_dir = tempfile::tempdir()?;
        let store_path = test_dir.path().join("test.db");
        let (_store_manager, mmr, _pool) = initialize_mmr(store_path.to_str().unwrap()).await?;

        // Create test data
        let test_headers = vec![
            create_test_block_header("0x1234", "0x0000"),
            create_test_block_header("0x5678", "0x1234"),
        ];

        // Get initial MMR state
        let initial_peaks = mmr.get_peaks(PeaksOptions::default()).await?;
        let initial_elements_count = mmr.elements_count.get().await?;
        let initial_leaves_count = mmr.leaves_count.get().await?;

        // Create guest input
        let mmr_input = GuestInput {
            initial_peaks: initial_peaks.clone(),
            elements_count: initial_elements_count,
            leaves_count: initial_leaves_count,
            new_elements: test_headers.iter().map(|h| h.block_hash.clone()).collect(),
        };

        let combined_input = CombinedInput {
            headers: test_headers.clone(),
            mmr_input,
        };

        // Run the proof generation
        let (_calldata, receipt) = run_blocking_tasks(&combined_input)?;

        // Verify the receipt
        receipt.verify(METHOD_ID).unwrap();

        // Decode and verify the output
        let guest_output: GuestOutput = receipt.journal.decode()?;

        // Verify state transition
        assert!(guest_output.elements_count > initial_elements_count, 
            "Elements count should increase");
        assert!(guest_output.leaves_count > initial_leaves_count,
            "Leaves count should increase");
        assert!(!guest_output.final_peaks.is_empty(), 
            "Should have peaks after insertion");

        // Verify each append result
        for (i, result) in guest_output.append_results.iter().enumerate() {
            assert_eq!(result.element_index, initial_elements_count + i + 1,
                "Element indices should be sequential");
        }

        // Clean up
        test_dir.close()?;
        Ok(())
    }

    #[tokio::test]
    async fn test_block_validation() -> Result<()> {
        // Test data
        let test_headers = vec![
            create_test_block_header("0x1234", "0x0000"),
            create_test_block_header("0x5678", "0x1234"),
        ];

        // Verify block chain validity
        assert!(are_blocks_and_chain_valid(&test_headers),
            "Test headers should form valid chain");

        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_state_transition() -> Result<()> {
        // Setup test environment with invalid state transition
        let test_dir = tempfile::tempdir()?;
        // let store_path = test_dir.path().join("test.db");
        // let (_store_manager, mmr, _pool) = initialize_mmr(store_path.to_str().unwrap()).await?;

        // Create test data with invalid state transition
        let mmr_input = GuestInput {
            initial_peaks: vec!["0x1234".to_string()],
            elements_count: 10,  // Set higher than actual to test invalid transition
            leaves_count: 5,
            new_elements: vec!["0x5678".to_string()],
        };

        let test_headers = vec![
            create_test_block_header("0x5678", "0x1234"),
        ];

        let combined_input = CombinedInput {
            headers: test_headers,
            mmr_input,
        };

        // Run the proof generation - should fail
        let result = run_blocking_tasks(&combined_input);
        assert!(result.is_err(), "Should fail with invalid state transition");

        // Clean up
        test_dir.close()?;
        Ok(())
    }

    // #[tokio::test]
    // async fn test_mmr_peaks_calculation() -> Result<()> {
    //     // Setup test environment
    //     let test_dir = tempfile::tempdir()?;
    //     let store_path = test_dir.path().join("test.db");
    //     let (store_manager, mut mmr, pool) = initialize_mmr(store_path.to_str().unwrap()).await?;

    //     // Add a sequence of elements and verify peaks
    //     let test_elements = vec![
    //         "0x1234".to_string(),
    //         "0x5678".to_string(),
    //         "0x9abc".to_string(),
    //     ];

    //     // Initial state
    //     let initial_peaks = mmr.get_peaks(PeaksOptions::default()).await?;

    //     // Create and process guest input
    //     let mmr_input = GuestInput {
    //         initial_peaks: initial_peaks.clone(),
    //         elements_count: mmr.elements_count.get().await?,
    //         leaves_count: mmr.leaves_count.get().await?,
    //         new_elements: test_elements.clone(),
    //     };

    //     let mut block_header = create_test_block_header("0x1234", "0x0000");

    //     let test_headers: Vec<BlockHeader> = test_elements
    //         .iter()
    //         .enumerate()
    //         .map(|(i, hash)| BlockHeader {
    //             block_hash: hash.clone(),
    //             parent_hash: if i == 0 {
    //                 Some("0x0000".to_string())
    //             } else {
    //                 Some(test_elements[i - 1].clone())
    //             },
    //             // ... fill other required fields
    //         })
    //         .collect();

    //     let combined_input = CombinedInput {
    //         headers: test_headers,
    //         mmr_input,
    //     };

    //     // Run proof generation
    //     let (_, receipt) = run_blocking_tasks(&combined_input)?;
    //     let guest_output: GuestOutput = receipt.journal.decode()?;

    //     // Verify peaks properties
    //     assert!(!guest_output.final_peaks.is_empty(), "Should have peaks");
    //     assert!(guest_output.final_peaks.len() <= guest_output.leaves_count,
    //         "Number of peaks should not exceed number of leaves");

    //     // Verify peaks are properly ordered
    //     for i in 1..guest_output.final_peaks.len() {
    //         assert!(guest_output.final_peaks[i] > guest_output.final_peaks[i-1],
    //             "Peaks should be ordered");
    //     }

    //     // Clean up
    //     test_dir.close()?;
    //     Ok(())
    // }

    // Helper function to create test BlockHeader
    // Helper function to create test BlockHeader
    fn create_test_block_header(block_hash: &str, parent_hash: &str) -> BlockHeader {
        BlockHeader {
            block_hash: block_hash.to_string(),
            parent_hash: Some(parent_hash.to_string()),
            // Fill in other required fields with test data
            number: 0,
            timestamp: Some("0x0".to_string()),
            difficulty: Some("0x0".to_string()),
            gas_limit: 0,
            gas_used: 0,
            nonce: "0x0".to_string(),
            extra_data: Some("0x0".to_string()),
            base_fee_per_gas: Some("0x0".to_string()),
            // transactions_root: Some("0x0".to_string()),
            state_root: Some("0x0".to_string()),
            receipts_root: Some("0x0".to_string()),
            miner: Some("0x0".to_string()),
            mix_hash: Some("0x0".to_string()),
            logs_bloom: Some("0x0".to_string()),
            withdrawals_root: Some("0x0".to_string()),
            transaction_root: Some("0x0".to_string()),
            ommers_hash: Some("0x0".to_string()),
            totaldifficulty: Some("0x0".to_string()),
            sha3_uncles: Some("0x0".to_string()),
            blob_gas_used: Some("0x0".to_string()),
            excess_blob_gas: Some("0x0".to_string()),
            parent_beacon_block_root: Some("0x0".to_string()),
        }
    }

    #[tokio::test]
    async fn test_full_workflow() -> Result<()> {
        // Setup test environment
        let test_dir = tempfile::tempdir()?;
        let store_path = test_dir.path().join("test.db");
        let (store_manager, mmr, pool) = initialize_mmr(store_path.to_str().unwrap()).await?;

        // Create a sequence of test blocks
        let test_blocks = vec![
            create_test_block_header("0x1234", "0x0000"),
            create_test_block_header("0x5678", "0x1234"),
            create_test_block_header("0x9abc", "0x5678"),
        ];

        // Get initial MMR state
        let initial_peaks = mmr.get_peaks(PeaksOptions::default()).await?;
        let initial_elements_count = mmr.elements_count.get().await?;
        let initial_leaves_count = mmr.leaves_count.get().await?;

        // Create guest input
        let mmr_input = GuestInput {
            initial_peaks,
            elements_count: initial_elements_count,
            leaves_count: initial_leaves_count,
            new_elements: test_blocks.iter().map(|b| b.block_hash.clone()).collect(),
        };

        let combined_input = CombinedInput {
            headers: test_blocks.clone(),
            mmr_input,
        };

        // Run the full workflow
        let (_calldata, receipt) = run_blocking_tasks(&combined_input)?;
        
        // Verify the receipt
        receipt.verify(METHOD_ID).unwrap();
        
        // Decode the output
        let guest_output: GuestOutput = receipt.journal.decode()?;

        // Verify the state transition
        assert!(guest_output.elements_count > initial_elements_count);
        assert!(guest_output.leaves_count > initial_leaves_count);
        
        // Verify each block was properly added
        for (i, _block) in test_blocks.iter().enumerate() {
            let result = &guest_output.append_results[i];
            assert_eq!(result.element_index, initial_elements_count + i + 1);
            
            // Verify the block can be found in the MMR
            let stored_index = store_manager
                .get_value_for_element_index(&pool, result.element_index)
                .await?;
            assert!(stored_index.is_some(), "Block should be stored in MMR");
        }

        // Verify final MMR state
        let final_peaks = mmr.get_peaks(PeaksOptions::default()).await?;
        assert_eq!(final_peaks, guest_output.final_peaks);

        // Clean up
        test_dir.close()?;
        Ok(())
    }
}