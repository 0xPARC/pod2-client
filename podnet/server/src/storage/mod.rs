use std::{fs, path::PathBuf};

use anyhow::Result;
use hex::ToHex;
use pod2::middleware::{Hash, Value, hash_values};
use podnet_models::DocumentContent;

pub struct ContentAddressedStorage {
    base_path: PathBuf,
}

impl ContentAddressedStorage {
    pub fn new(base_path: &str) -> Result<Self> {
        let path = PathBuf::from(base_path);
        fs::create_dir_all(&path)?;
        Ok(Self { base_path: path })
    }

    pub fn hash_content(content: &str) -> Hash {
        hash_values(&[Value::from(content)])
    }

    pub fn hash_document_content(content: &DocumentContent) -> Result<Hash> {
        let json_string = serde_json::to_string(content)?;
        Ok(hash_values(&[Value::from(json_string)]))
    }

    pub fn get_file_path(&self, hash: &str) -> PathBuf {
        let prefix = &hash[0..2];
        let suffix = &hash[2..];
        self.base_path.join(prefix).join(suffix)
    }

    pub fn store(&self, content: &str) -> Result<Hash> {
        let hash = Self::hash_content(content);
        let hash_string: String = hash.encode_hex();
        let file_path = self.get_file_path(&hash_string);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if !file_path.exists() {
            fs::write(&file_path, content)?;
        }

        Ok(hash)
    }

    pub fn store_document_content(&self, content: &DocumentContent) -> Result<Hash> {
        let json_string = serde_json::to_string(content)?;
        let hash = Self::hash_document_content(content)?;
        let hash_string: String = hash.encode_hex();
        let file_path = self.get_file_path(&hash_string);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if !file_path.exists() {
            fs::write(&file_path, json_string)?;
        }

        Ok(hash)
    }

    pub fn retrieve(&self, hash: &Hash) -> Result<Option<String>> {
        let hash_string: String = hash.encode_hex();
        let file_path = self.get_file_path(&hash_string);

        if file_path.exists() {
            let content = fs::read_to_string(file_path)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    pub fn retrieve_document_content(&self, hash: &Hash) -> Result<Option<DocumentContent>> {
        let hash_string: String = hash.encode_hex();
        let file_path = self.get_file_path(&hash_string);

        if file_path.exists() {
            let json_string = fs::read_to_string(file_path)?;
            let content: DocumentContent = serde_json::from_str(&json_string)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    pub fn exists(&self, hash: &str) -> bool {
        self.get_file_path(hash).exists()
    }
}
