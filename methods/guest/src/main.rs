// main.rs
use risc0_zkvm::guest::env;
use block_validity::{BlockHeader, utils::are_blocks_and_chain_valid};
use serde::{Serialize, Deserialize};
mod guest_mmr;
use guest_mmr::{GuestMMR, GuestInput, GuestOutput};

#[derive(Debug, Serialize, Deserialize)]
struct CombinedInput {
    headers: Vec<BlockHeader>,
    mmr_input: GuestInput,
}

fn main() {
    // Read combined input
    let input: CombinedInput = env::read();
    
    // Verify block headers
    assert!(are_blocks_and_chain_valid(&input.headers), "Invalid block headers");

    println!("Creating MMR with:");
    println!("Initial peaks: {:?}", input.mmr_input.initial_peaks);
    println!("Initial elements count: {}", input.mmr_input.elements_count);
    println!("Initial leaves count: {}", input.mmr_input.leaves_count);

    // Initialize MMR with previous state
    let mut mmr = GuestMMR::new(
        input.mmr_input.initial_peaks,
        input.mmr_input.elements_count,
        input.mmr_input.leaves_count,
    );

    let mut append_results = Vec::new();

    // Append block hashes to MMR
    for (i, header) in input.headers.iter().enumerate() {
        println!("Appending block {} with hash: {}", i, header.block_hash);
        let block_hash = header.block_hash.clone();
        match mmr.append(block_hash) {
            Ok(result) => {
                println!("Append successful:");
                println!("  Element index: {}", result.element_index);
                println!("  Elements count: {}", result.elements_count);
                println!("  Leaves count: {}", result.leaves_count);
                append_results.push(result);
            },
            Err(e) => {
                println!("Error during append: {:?}", e);
                assert!(false, "MMR append failed: {:?}", e);
            }
        }
    }

    // Get final peaks
    let final_peaks = match mmr.get_peaks(Default::default()) {
        Ok(peaks) => peaks,
        Err(e) => {
            println!("Error getting peaks: {:?}", e);
            assert!(false, "Failed to get final peaks: {:?}", e);
            vec![] // This line will never be reached due to assert
        }
    };

    // Create output
    let output = GuestOutput {
        final_peaks,
        elements_count: mmr.get_elements_count(),
        leaves_count: mmr.get_leaves_count(),
        append_results,
    };

    // Commit the output
    env::commit(&output);
}