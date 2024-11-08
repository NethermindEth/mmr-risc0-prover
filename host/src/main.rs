mod accumulator;
mod proof_generator;
mod types;
use accumulator::AccumulatorBuilder;
use eyre::Result;
use methods::{MMR_GUEST_ELF, MMR_GUEST_ID};
use mmr_accumulator::processor_utils::{create_database_file, ensure_directory_exists};
use proof_generator::{ProofGenerator, ProofType};
use tracing::info;
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    // Initialize proof generator
    let proof_generator = ProofGenerator::new(MMR_GUEST_ELF, MMR_GUEST_ID);

    // Initialize accumulator builder
    let current_dir = ensure_directory_exists("db-instances")?;
    let store_path = create_database_file(&current_dir, 0)?;
    let mut builder = AccumulatorBuilder::new(&store_path, proof_generator, 4).await?;

    // Build accumulator from finalized block to genesis
    let results = builder.build_from_finalized().await?;

    // Print results
    for result in &results {
        info!(
            "Processed blocks {} to {}",
            result.start_block, result.end_block
        );
        match &result.proof {
            Some(ProofType::Stark { .. }) => info!("Generated STARK proof"),
            Some(ProofType::Groth16 { .. }) => info!("Generated Groth16 proof"),
            None => info!("No proof generated"),
        }
    }

    Ok(())
}
