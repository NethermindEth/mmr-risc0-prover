use clap::Parser;
use dotenv::dotenv;
use eyre::Result;
use host::{AccumulatorBuilder, ProofGenerator, ProofType};
use methods::{MMR_GUEST_ELF, MMR_GUEST_ID};
use mmr_accumulator::processor_utils::{create_database_file, ensure_directory_exists};
use starknet_handler::verify_groth16_proof_onchain;
use std::env;
use tracing::info;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Batch size for processing blocks
    #[arg(short, long, default_value_t = 1024)]
    batch_size: u64,

    /// Path to the SQLite database file. If not specified, a new one will be created.
    #[arg(short, long)]
    db_file: Option<String>,

    /// Number of batches to process. If not specified, processes until block #0.
    #[arg(short, long)]
    num_batches: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    info!("Starting host...");
    // Load environment variables from .env file
    dotenv().ok();

    let rpc_url = env::var("STARKNET_RPC_URL").expect("STARKNET_RPC_URL must be set");
    let verifier_address = env::var("VERIFIER_ADDRESS").expect("VERIFIER_ADDRESS must be set");

    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    // Parse CLI arguments
    let args = Args::parse();

    // Load the database file path from the environment or use the provided argument
    let store_path = if let Some(db_file) = &args.db_file {
        db_file.clone()
    } else {
        // Check for DB_FILE environment variable
        if let Ok(env_db_file) = env::var("DB_FILE") {
            env_db_file
        } else {
            // Otherwise, create a new database file
            let current_dir = ensure_directory_exists("db-instances")?;
            create_database_file(&current_dir, 0)?
        }
    };

    info!("Initializing proof generator...");
    // Initialize proof generator
    let proof_generator = ProofGenerator::new(MMR_GUEST_ELF, MMR_GUEST_ID);

    info!("Initializing accumulator builder...");
    // Initialize accumulator builder with the batch size
    let mut builder =
        AccumulatorBuilder::new(&store_path, proof_generator, args.batch_size).await?;

    info!("Building MMR...");
    // Build MMR from finalized block to block #0 or up to the specified number of batches
    let results = if let Some(num_batches) = args.num_batches {
        builder.build_with_num_batches(num_batches).await?
    } else {
        builder.build_from_finalized().await?
    };

    info!("Processing results...");
    // Print results
    for result in &results {
        info!(
            "Processed blocks {} to {}",
            result.start_block, result.end_block
        );
        match &result.proof {
            Some(ProofType::Stark { .. }) => info!("Generated STARK proof"),
            Some(ProofType::Groth16 { calldata, .. }) => {
                info!("Generated Groth16 proof");
                let result = verify_groth16_proof_onchain(&rpc_url, &verifier_address, &calldata);
                info!(
                    "Proof verification result: {:?}",
                    result.await.expect("Failed to verify final Groth16 proof")
                );
            }
            None => info!("No proof generated"),
        }
    }
    info!("Host finished");

    Ok(())
}
