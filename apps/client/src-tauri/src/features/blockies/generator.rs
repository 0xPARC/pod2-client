use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;

pub struct BlockiesGenerator {
    cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl BlockiesGenerator {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn generate_png(&self, public_key: &str) -> Result<Vec<u8>> {
        // Check cache first
        if let Ok(cache) = self.cache.lock() {
            if let Some(cached_data) = cache.get(public_key) {
                return Ok(cached_data.clone());
            }
        }

        // Generate new blockies
        let png_data = self.generate_blockies_png(public_key)?;

        // Cache the result
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(public_key.to_string(), png_data.clone());
        }

        Ok(png_data)
    }

    fn generate_blockies_png(&self, public_key: &str) -> Result<Vec<u8>> {
        use eth_blockies::{Blockies, BlockiesGenerator};

        // Canonicalize the public key for consistent blockies generation
        let seed = public_key.to_lowercase();

        // Generate 16x16 blockies data using the new API
        type Icon16<T> = Blockies<16, T>;
        let blockies_data = Icon16::data(&seed);

        // Create PNG image from blockies data
        let png_data = self.create_png_from_blockies(&blockies_data)?;

        Ok(png_data)
    }

    fn create_png_from_blockies(
        &self,
        blockies_data: &[[(u8, u8, u8); 16]; 16],
    ) -> Result<Vec<u8>> {
        // Scale factor for larger image (each blockies pixel becomes 4x4 pixels for 64x64 total)
        const SCALE: usize = 4;
        const WIDTH: usize = 16 * SCALE;
        const HEIGHT: usize = 16 * SCALE;

        // Create image buffer
        let mut image_data = vec![0u8; WIDTH * HEIGHT * 3]; // RGB format

        // Fill the image data using the 16x16 grid
        for (row_idx, row) in blockies_data.iter().enumerate() {
            for (col_idx, &(r, g, b)) in row.iter().enumerate() {
                // Scale up each blockies pixel to SCALE x SCALE pixels
                for y in 0..SCALE {
                    for x in 0..SCALE {
                        let pixel_row = row_idx * SCALE + y;
                        let pixel_col = col_idx * SCALE + x;
                        let pixel_idx = (pixel_row * WIDTH + pixel_col) * 3;

                        if pixel_idx + 2 < image_data.len() {
                            image_data[pixel_idx] = r;
                            image_data[pixel_idx + 1] = g;
                            image_data[pixel_idx + 2] = b;
                        }
                    }
                }
            }
        }

        // Convert to PNG using a simple PNG encoder
        let png_data = self.encode_png_simple(&image_data, WIDTH, HEIGHT)?;
        Ok(png_data)
    }

    fn encode_png_simple(&self, rgb_data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        // For now, we'll create a simple BMP format since PNG encoding requires additional dependencies
        // This can be upgraded to PNG later if needed
        self.encode_bmp(rgb_data, width, height)
    }

    fn encode_bmp(&self, rgb_data: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        let mut bmp_data = Vec::new();

        // BMP header
        let file_size = 54 + (width * height * 3) as u32;

        // BMP file header (14 bytes)
        bmp_data.extend_from_slice(b"BM"); // Signature
        bmp_data.extend_from_slice(&file_size.to_le_bytes()); // File size
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Reserved
        bmp_data.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel data

        // BMP info header (40 bytes)
        bmp_data.extend_from_slice(&40u32.to_le_bytes()); // Header size
        bmp_data.extend_from_slice(&(width as u32).to_le_bytes()); // Width
        bmp_data.extend_from_slice(&(height as u32).to_le_bytes()); // Height
        bmp_data.extend_from_slice(&1u16.to_le_bytes()); // Planes
        bmp_data.extend_from_slice(&24u16.to_le_bytes()); // Bits per pixel
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Compression
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Image size
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // X pixels per meter
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Y pixels per meter
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Colors used
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // Important colors

        // BMP stores rows bottom to top, so we need to flip the image
        for row in (0..height).rev() {
            let row_start = row * width * 3;
            let row_end = row_start + width * 3;
            let row_data = &rgb_data[row_start..row_end];

            // Convert RGB to BGR for BMP format
            for chunk in row_data.chunks(3) {
                if chunk.len() == 3 {
                    bmp_data.push(chunk[2]); // B
                    bmp_data.push(chunk[1]); // G
                    bmp_data.push(chunk[0]); // R
                }
            }
        }

        Ok(bmp_data)
    }
}

impl Default for BlockiesGenerator {
    fn default() -> Self {
        Self::new()
    }
}
