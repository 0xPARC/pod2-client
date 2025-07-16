use super::generator::BlockiesGenerator;
use lazy_static::lazy_static;

lazy_static! {
    static ref BLOCKIES_GENERATOR: BlockiesGenerator = BlockiesGenerator::new();
}

/// Generate a blockies image for a given public key
#[tauri::command]
pub async fn generate_blockies(public_key: String) -> Result<String, String> {
    log::debug!("Generating blockies for public key: {}", public_key);
    
    // Generate the blockies image
    let image_data = BLOCKIES_GENERATOR
        .generate_png(&public_key)
        .map_err(|e| format!("Failed to generate blockies: {}", e))?;
    
    // Convert to base64 for transmission to frontend
    use base64::{Engine as _, engine::general_purpose};
    let base64_data = general_purpose::STANDARD.encode(&image_data);
    
    log::debug!("Generated blockies of size {} bytes", image_data.len());
    
    Ok(base64_data)
}

/// Get blockies data as raw RGB values (for debugging or other uses)
#[tauri::command]
pub async fn get_blockies_data(public_key: String) -> Result<Vec<Vec<[u8; 3]>>, String> {
    log::debug!("Getting blockies data for public key: {}", public_key);
    
    use eth_blockies::{Blockies, BlockiesGenerator};
    
    // Canonicalize the public key for consistent blockies generation
    let seed = public_key.to_lowercase();
    
    // Generate 16x16 blockies data using the new API
    type Icon16<T> = Blockies<16, T>;
    let blockies_data = Icon16::data(&seed);
    
    // Convert to Vec<Vec<[u8; 3]>> format for JSON serialization
    let result: Vec<Vec<[u8; 3]>> = blockies_data
        .iter()
        .map(|row| row.iter().map(|&(r, g, b)| [r, g, b]).collect())
        .collect();
    
    Ok(result)
}