pub mod accumulator;
pub mod proof_generator;
pub mod types;

pub use accumulator::AccumulatorBuilder;
use eyre::Result;
pub use proof_generator::{ProofGenerator, ProofType};
use tracing::info;
use mmr_accumulator::BlockHeader;
use methods::{MMR_GUEST_ELF, MMR_GUEST_ID};

pub async fn update_mmr(
    db_file: &str,                  // Path to the existing SQLite database file
    new_headers: Vec<BlockHeader>,  // New block headers to update the MMR
) -> Result<()> {
    // Initialize proof generator
    let proof_generator = ProofGenerator::new(MMR_GUEST_ELF, MMR_GUEST_ID);

    // Initialize accumulator builder
    let mut builder = AccumulatorBuilder::new(db_file, proof_generator, 0).await?;

    // **Call the method here**
    builder.update_mmr_with_new_headers(new_headers).await?;

    info!("MMR successfully updated with new block headers");

    Ok(())
}

