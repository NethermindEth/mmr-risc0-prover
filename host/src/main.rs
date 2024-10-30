use alloy::primitives::hex::encode;
use anyhow::Result;
use garaga_rs::{
    calldata::full_proof_with_hints::groth16::{
        get_groth16_calldata, risc0_utils::get_risc0_vk, Groth16Proof,
    },
    definitions::CurveID,
};
use methods::{METHOD_ELF, METHOD_ID};
// use num_bigint::BigUint;
use risc0_ethereum_contracts::encode_seal;
// use risc0_groth16::Seal;
use risc0_zkvm::{
    compute_image_id, default_prover, sha::Digestible, ExecutorEnv, ProverOpts, VerifierContext,
};
use tracing_subscriber;

fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    let input: u32 = 15 * u32::pow(2, 27) + 1;
    let env = ExecutorEnv::builder()
        .write(&input)
        .unwrap()
        .build()
        .unwrap();

    let receipt = default_prover()
        .prove_with_ctx(
            env,
            &VerifierContext::default(),
            METHOD_ELF,
            &ProverOpts::groth16(),
        )?
        .receipt;

    let encoded_seal = encode_seal(&receipt)?;
    println!("Solidity Encoded Seal: 0x{}", encode(encoded_seal.clone()));

    let journal = receipt.journal.digest();
    println!("Journal: 0x{}", encode(journal.clone()));

    let image_id = compute_image_id(&METHOD_ELF).unwrap();
    println!("Image ID: 0x{}", encode(image_id));

    let proof = Groth16Proof::from_risc0(
        encoded_seal,
        image_id.as_bytes().to_vec(),
        journal.as_bytes().to_vec(),
    );

    let calldata = get_groth16_calldata(&proof, &get_risc0_vk(), CurveID::BN254).unwrap();
    println!("Calldata: {:?}", calldata);

    receipt.verify(METHOD_ID).unwrap();

    Ok(())
}
