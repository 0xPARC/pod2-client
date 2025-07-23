use std::{fs::File, io::prelude::*};

use pod2::backends::plonky2::primitives::ec::{curve::Point, schnorr::SecretKey};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct KeypairData {
    pub secret_key: String,
    pub public_key: Point,
    pub created_at: String,
    pub key_type: String,
}

pub fn generate_keypair(output_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Generate a new Schnorr keypair
    let secret_key = SecretKey::new_rand();
    let public_key = secret_key.public_key();

    // Create a JSON structure with both keys and metadata
    let keypair_data = KeypairData {
        secret_key: hex::encode(secret_key.0.to_bytes_le()),
        public_key,
        created_at: chrono::Utc::now().to_rfc3339(),
        key_type: "schnorr".to_string(),
    };

    // Write to file
    let mut file = File::create(output_file)?;
    file.write_all(serde_json::to_string_pretty(&keypair_data)?.as_bytes())?;

    println!("Generated keypair:");
    println!("Public Key: {public_key}");
    println!("Saved to: {output_file}");

    Ok(())
}
