use anyhow::{Context, Result};
use chrono::Utc;
use hex::ToHex;
use pod2::{
    backends::plonky2::primitives::ec::schnorr::SecretKey,
    frontend::{MainPod, SerializedMainPod, SerializedSignedPod, SignedPod},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Db;

// --- General API Data Structures ---

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct SpaceInfo {
    pub id: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(tag = "pod_data_variant", content = "pod_data_payload")]
pub enum PodData {
    Signed(Box<SerializedSignedPod>),
    Main(Box<SerializedMainPod>),
}

impl PodData {
    /// Returns a string representation of the pod data variant.
    pub fn type_str(&self) -> &'static str {
        match self {
            PodData::Signed(_) => "signed",
            PodData::Main(_) => "main",
        }
    }

    pub fn id(&self) -> String {
        match self {
            PodData::Signed(pod) => pod.id().0.encode_hex(),
            PodData::Main(pod) => pod.id().0.encode_hex(),
        }
    }
}

impl From<SignedPod> for PodData {
    fn from(pod: SignedPod) -> Self {
        PodData::Signed(Box::new(pod.into()))
    }
}

impl From<MainPod> for PodData {
    fn from(pod: MainPod) -> Self {
        PodData::Main(Box::new(pod.into()))
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct PodInfo {
    pub id: String,
    pub pod_type: String,
    pub data: PodData,
    pub label: Option<String>,
    pub created_at: String,
    pub space: String,
}

pub async fn create_space(db: &Db, id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let id_clone = id.to_string();

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    conn.interact(move |conn| {
        conn.execute(
            "INSERT INTO spaces (id, created_at) VALUES (?1, ?2)",
            rusqlite::params![id_clone, now],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for create_space")??;

    Ok(())
}

pub async fn list_spaces(db: &Db) -> Result<Vec<SpaceInfo>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let spaces = conn
        .interact(|conn| {
            let mut stmt = conn.prepare("SELECT id, created_at FROM spaces")?;
            let space_iter = stmt.query_map([], |row| {
                Ok(SpaceInfo {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                })
            })?;
            space_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for list_spaces")??;

    Ok(spaces)
}

pub async fn space_exists(db: &Db, id: &str) -> Result<bool> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection for space check")?;
    let id_clone = id.to_string();
    let exists = conn
        .interact(move |conn| {
            let mut stmt = conn.prepare("SELECT 1 FROM spaces WHERE id = ?1")?;
            stmt.exists([id_clone])
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for space existence check")??;
    Ok(exists)
}

pub async fn delete_space(db: &Db, id: &str) -> Result<usize> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;
    let id_clone = id.to_string();

    let rows_deleted = conn
        .interact(move |conn| conn.execute("DELETE FROM spaces WHERE id = ?1", [&id_clone]))
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for delete_space")??;

    Ok(rows_deleted)
}

// --- Pod Queries ---

pub async fn import_pod(
    db: &Db,
    data: &PodData,
    label: Option<&str>,
    space_id: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let data_blob =
        serde_json::to_vec(data).context("Failed to serialize PodData enum for storage")?;

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let label_clone = label.map(|s| s.to_string());
    let space_id_clone = space_id.to_string();
    let type_str = data.type_str();
    let id = data.id();

    conn.interact(move |conn| {
        conn.execute(
            "INSERT INTO pods (id, pod_type, data, label, created_at, space) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id,
                type_str,
                data_blob,
                label_clone,
                now,
                space_id_clone
            ],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for import_pod")??;

    Ok(())
}

pub async fn get_pod(db: &Db, space_id: &str, pod_id: &str) -> Result<Option<PodInfo>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;
    let space_id_clone = space_id.to_string();
    let pod_id_clone = pod_id.to_string();

    let pod_info_result = conn
        .interact(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, pod_type, data, label, created_at, space FROM pods WHERE space = ?1 AND id = ?2",
            )?;
            let result = stmt.query_row([&space_id_clone, &pod_id_clone], |row| {
                let data_blob: Vec<u8> = row.get(2)?;
                let pod_data: PodData =
                    serde_json::from_slice(&data_blob).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Blob,
                            Box::new(e),
                        )
                    })?;
                Ok(PodInfo {
                    id: row.get(0)?,
                    pod_type: row.get(1)?,
                    data: pod_data,
                    label: row.get(3)?,
                    created_at: row.get(4)?,
                    space: row.get(5)?,
                })
            });

            match result {
                Ok(pod_info) => Ok(Some(pod_info)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_pod")??;

    Ok(pod_info_result)
}

pub async fn list_pods(db: &Db, space_id: &str) -> Result<Vec<PodInfo>> {
    list_pods_filtered(db, space_id, None).await
}

pub async fn list_pods_by_type(db: &Db, space_id: &str, pod_type: &str) -> Result<Vec<PodInfo>> {
    list_pods_filtered(db, space_id, Some(pod_type)).await
}

async fn list_pods_filtered(
    db: &Db,
    space_id: &str,
    pod_type_filter: Option<&str>,
) -> Result<Vec<PodInfo>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;
    let space_id_clone = space_id.to_string();
    let pod_type_filter_clone = pod_type_filter.map(|s| s.to_string());

    let pods = conn
        .interact(move |conn| {
            match pod_type_filter_clone {
                Some(pod_type) => {
                    let mut stmt = conn.prepare(
                        "SELECT id, pod_type, data, label, created_at, space FROM pods WHERE space = ?1 AND pod_type = ?2"
                    )?;
                    let pod_iter = stmt.query_map([&space_id_clone, &pod_type], |row| {
                        let data_blob: Vec<u8> = row.get(2)?;
                        let pod_data: PodData = serde_json::from_slice(&data_blob).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Blob,
                                Box::new(e),
                            )
                        })?;
                        Ok(PodInfo {
                            id: row.get(0)?,
                            pod_type: row.get(1)?,
                            data: pod_data,
                            label: row.get(3)?,
                            created_at: row.get(4)?,
                            space: row.get(5)?,
                        })
                    })?;
                    pod_iter.collect::<Result<Vec<_>, _>>()
                },
                None => {
                    let mut stmt = conn.prepare(
                        "SELECT id, pod_type, data, label, created_at, space FROM pods WHERE space = ?1"
                    )?;
                    let pod_iter = stmt.query_map([&space_id_clone], |row| {
                        let data_blob: Vec<u8> = row.get(2)?;
                        let pod_data: PodData = serde_json::from_slice(&data_blob).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Blob,
                                Box::new(e),
                            )
                        })?;
                        Ok(PodInfo {
                            id: row.get(0)?,
                            pod_type: row.get(1)?,
                            data: pod_data,
                            label: row.get(3)?,
                            created_at: row.get(4)?,
                            space: row.get(5)?,
                        })
                    })?;
                    pod_iter.collect::<Result<Vec<_>, _>>()
                }
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for list_pods_filtered")??;
    Ok(pods)
}

pub async fn delete_pod(db: &Db, space_id: &str, pod_id: &str) -> Result<usize> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;
    let space_id_clone = space_id.to_string();
    let pod_id_clone = pod_id.to_string();

    let rows_deleted = conn
        .interact(move |conn| {
            // Check if the pod is mandatory before attempting to delete
            let mut check_stmt =
                conn.prepare("SELECT is_mandatory FROM pods WHERE space = ?1 AND id = ?2")?;
            let is_mandatory = check_stmt.query_row([&space_id_clone, &pod_id_clone], |row| {
                Ok(row.get::<_, bool>(0).unwrap_or(false))
            });

            match is_mandatory {
                Ok(true) => {
                    // Pod is mandatory, cannot be deleted
                    Err(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                        Some("Cannot delete mandatory POD".to_string()),
                    ))
                }
                Ok(false) => {
                    // Pod is not mandatory, proceed with deletion
                    conn.execute(
                        "DELETE FROM pods WHERE space = ?1 AND id = ?2",
                        [space_id_clone, pod_id_clone],
                    )
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Pod doesn't exist, return 0 rows deleted
                    Ok(0)
                }
                Err(e) => Err(e),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for delete_pod")??;
    Ok(rows_deleted)
}

pub async fn count_all_pods(db: &Db) -> Result<u32> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    conn.interact(move |conn| {
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM pods", [], |row| row.get(0))?;
        Ok(count as u32)
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for count_all_pods")?
}

pub async fn count_pods_by_type(db: &Db) -> Result<(u32, u32)> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let counts = conn
        .interact(move |conn| {
            let signed_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM pods WHERE pod_type = 'signed'",
                [],
                |row| row.get(0),
            )?;
            let main_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM pods WHERE pod_type = 'main'",
                [],
                |row| row.get(0),
            )?;
            Ok::<_, rusqlite::Error>((signed_count as u32, main_count as u32))
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for count_pods_by_type")??;

    Ok(counts)
}

// --- P2P Messaging Functions ---

/// Add a message to the inbox for user approval
pub async fn add_inbox_message(
    db: &Db,
    from_node_id: &str,
    from_alias: Option<&str>,
    space_id: &str,
    pod_id: &str,
    message_text: Option<&str>,
) -> Result<String> {
    let message_id = uuid::Uuid::new_v4().to_string();
    let received_at = Utc::now().to_rfc3339();

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let from_node_id_clone = from_node_id.to_string();
    let from_alias_clone = from_alias.map(|s| s.to_string());
    let space_id_clone = space_id.to_string();
    let pod_id_clone = pod_id.to_string();
    let message_text_clone = message_text.map(|s| s.to_string());
    let message_id_clone = message_id.clone();

    conn.interact(move |conn| {
        conn.execute(
            "INSERT INTO inbox_messages (id, from_node_id, from_alias, space_id, pod_id, message_text, received_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
            rusqlite::params![
                message_id_clone,
                from_node_id_clone,
                from_alias_clone,
                space_id_clone,
                pod_id_clone,
                message_text_clone,
                received_at
            ],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for add_inbox_message")??;

    Ok(message_id)
}

/// Get pending inbox messages
pub async fn get_inbox_messages(db: &Db) -> Result<Vec<serde_json::Value>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let messages = conn
        .interact(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, from_node_id, from_alias, space_id, pod_id, message_text, received_at, status
                 FROM inbox_messages
                 WHERE status = 'pending'
                 ORDER BY received_at DESC"
            )?;
            let message_iter = stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "from_node_id": row.get::<_, String>(1)?,
                    "from_alias": row.get::<_, Option<String>>(2)?,
                    "space_id": row.get::<_, String>(3)?,
                    "pod_id": row.get::<_, String>(4)?,
                    "message_text": row.get::<_, Option<String>>(5)?,
                    "received_at": row.get::<_, String>(6)?,
                    "status": row.get::<_, String>(7)?
                }))
            })?;
            message_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_inbox_messages")??;

    Ok(messages)
}

/// Accept an inbox message and create/update chat
pub async fn accept_inbox_message(
    db: &Db,
    message_id: &str,
    chat_alias: Option<&str>,
) -> Result<String> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let message_id_clone = message_id.to_string();
    let chat_alias_clone = chat_alias.map(|s| s.to_string());

    let chat_id = conn
        .interact(move |conn| {
            let tx = conn.transaction()?;
            // Get the inbox message
            let (from_node_id, from_alias, space_id, pod_id, message_text, received_at): (String, Option<String>, String, String, Option<String>, String) = {
                let mut stmt = tx.prepare(
                    "SELECT from_node_id, from_alias, space_id, pod_id, message_text, received_at
                     FROM inbox_messages
                     WHERE id = ?1 AND status = 'pending'"
                )?;
                stmt.query_row([&message_id_clone], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?
                    ))
                })?
            };

            // Create or get existing chat
            let chat_id = uuid::Uuid::new_v4().to_string();
            let final_alias = chat_alias_clone.or(from_alias);
            let now = chrono::Utc::now().to_rfc3339();

            // Try to insert new chat, or get existing one
            match tx.execute(
                "INSERT INTO chats (id, peer_node_id, peer_alias, last_activity, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![&chat_id, &from_node_id, &final_alias, &now, &now]
            ) {
                Ok(_) => {
                    // New chat created, use the generated ID
                }
                Err(rusqlite::Error::SqliteFailure(err, _)) if err.code == rusqlite::ErrorCode::ConstraintViolation => {
                    // Chat already exists, get the existing chat_id
                    let existing_chat_id: String = {
                        let mut stmt = tx.prepare("SELECT id FROM chats WHERE peer_node_id = ?1")?;
                        stmt.query_row([&from_node_id], |row| row.get(0))?
                    };
                    return Ok(existing_chat_id);
                }
                Err(e) => return Err(e),
            }

            // Add message to chat_messages
            let chat_message_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO chat_messages (id, chat_id, space_id, pod_id, message_text, timestamp, direction) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'received')",
                rusqlite::params![&chat_message_id, &chat_id, &space_id, &pod_id, &message_text, &received_at]
            )?;

            // Mark inbox message as accepted
            tx.execute(
                "UPDATE inbox_messages SET status = 'accepted' WHERE id = ?1",
                [&message_id_clone]
            )?;

            tx.commit()?;
            Ok(chat_id)
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for accept_inbox_message")??;

    Ok(chat_id)
}

// --- Private Key Management ---

/// Regenerate public keys from private keys to use proper base58 encoding
/// This should be called after migrations to fix any existing hex-based public keys
pub async fn regenerate_public_keys_if_needed(db: &Db) -> Result<()> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let updated_count = conn
        .interact(|conn| {
            let mut stmt = conn.prepare("SELECT private_key, public_key FROM private_keys")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?, // private_key
                    row.get::<_, String>(1)?, // public_key
                ))
            })?;

            let mut count = 0;
            for row in rows {
                let (private_key_hex, current_public_key) = row?;

                // Check if this looks like the old hex format (starts with "pub_")
                if current_public_key.starts_with("pub_") {
                    // Regenerate proper public key from private key
                    let bytes = match hex::decode(&private_key_hex) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            log::error!("Failed to decode private key hex for regeneration: {e}");
                            continue; // Skip this key and continue with others
                        }
                    };
                    let big_uint = num::BigUint::from_bytes_be(&bytes);
                    let secret_key = SecretKey(big_uint);
                    let public_key_base58 = secret_key.public_key().to_string();

                    // Update the public key
                    conn.execute(
                        "UPDATE private_keys SET public_key = ?1 WHERE private_key = ?2",
                        rusqlite::params![public_key_base58, private_key_hex],
                    )?;
                    count += 1;
                }
            }

            Ok::<i32, rusqlite::Error>(count)
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for regenerate_public_keys_if_needed")??;

    if updated_count > 0 {
        log::info!("Regenerated {updated_count} public keys to use proper base58 encoding");
    }

    Ok(())
}

/// Get the default private key, returns error if none exists (no auto-generation)
pub async fn get_default_private_key(db: &Db) -> Result<SecretKey> {
    // Check if setup is completed first
    if !is_setup_completed(db).await? {
        return Err(anyhow::anyhow!(
            "Identity setup not completed. Please complete the mandatory identity setup first."
        ));
    }

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let key_hex = conn
        .interact(|conn| {
            let mut stmt =
                conn.prepare("SELECT private_key FROM private_keys WHERE is_default = TRUE")?;
            let result = stmt.query_row([], |row| row.get::<_, String>(0));

            match result {
                Ok(hex_string) => Ok(hex_string),
                Err(rusqlite::Error::QueryReturnedNoRows) => Err(anyhow::anyhow!(
                    "No default private key found after ensuring one exists"
                )),
                Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_default_private_key")??;

    let bytes = hex::decode(key_hex).context("Failed to decode private key hex")?;
    let big_uint = num::BigUint::from_bytes_be(&bytes);
    Ok(SecretKey(big_uint))
}

/// Get information about the default private key (without exposing the secret key)
pub async fn get_default_private_key_info(db: &Db) -> Result<serde_json::Value> {
    // Check if setup is completed first
    if !is_setup_completed(db).await? {
        return Err(anyhow::anyhow!(
            "Identity setup not completed. Please complete the mandatory identity setup first."
        ));
    }

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let key_info = conn
        .interact(|conn| {
            let mut stmt = conn.prepare(
                "SELECT private_key, public_key, alias, created_at FROM private_keys WHERE is_default = TRUE"
            )?;
            let result = stmt.query_row([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?, // Use private_key as id
                    "public_key": row.get::<_, String>(1)?,
                    "alias": row.get::<_, Option<String>>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                    "is_default": true
                }))
            });

            match result {
                Ok(info) => Ok(info),
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    Err(anyhow::anyhow!("No default private key found after ensuring one exists"))
                }
                Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_default_private_key_info")??;

    Ok(key_info)
}

// --- Chat Management Functions ---

/// Get all chats ordered by last activity
pub async fn get_chats(db: &Db) -> Result<Vec<serde_json::Value>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let chats = conn
        .interact(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, peer_node_id, peer_alias, last_activity, created_at, status
                 FROM chats
                 WHERE status = 'active'
                 ORDER BY last_activity DESC",
            )?;
            let chat_iter = stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "peer_node_id": row.get::<_, String>(1)?,
                    "peer_alias": row.get::<_, Option<String>>(2)?,
                    "last_activity": row.get::<_, String>(3)?,
                    "created_at": row.get::<_, String>(4)?,
                    "status": row.get::<_, String>(5)?
                }))
            })?;
            chat_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_chats")??;

    Ok(chats)
}

/// Get messages for a specific chat
pub async fn get_chat_messages(db: &Db, chat_id: &str) -> Result<Vec<serde_json::Value>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let chat_id_clone = chat_id.to_string();

    let messages = conn
        .interact(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, space_id, pod_id, message_text, timestamp, direction, created_at
                 FROM chat_messages
                 WHERE chat_id = ?1
                 ORDER BY timestamp ASC",
            )?;
            let message_iter = stmt.query_map([&chat_id_clone], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "space_id": row.get::<_, String>(1)?,
                    "pod_id": row.get::<_, String>(2)?,
                    "message_text": row.get::<_, Option<String>>(3)?,
                    "timestamp": row.get::<_, String>(4)?,
                    "direction": row.get::<_, String>(5)?,
                    "created_at": row.get::<_, String>(6)?
                }))
            })?;
            message_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_chat_messages")??;

    Ok(messages)
}

/// Add a sent message to a chat (when sending PODs)
pub async fn add_sent_message_to_chat(
    db: &Db,
    peer_node_id: &str,
    space_id: &str,
    pod_id: &str,
    message_text: Option<&str>,
) -> Result<String> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let peer_node_id_clone = peer_node_id.to_string();
    let space_id_clone = space_id.to_string();
    let pod_id_clone = pod_id.to_string();
    let message_text_clone = message_text.map(|s| s.to_string());

    let message_id = conn
        .interact(move |conn| {
            let tx = conn.transaction()?;

            // Find or create chat for this peer
            let chat_id = {
                let mut stmt = tx.prepare("SELECT id FROM chats WHERE peer_node_id = ?1")?;
                let result = stmt.query_row([&peer_node_id_clone], |row| {
                    row.get::<_, String>(0)
                });

                match result {
                    Ok(existing_chat_id) => existing_chat_id,
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        // Create new chat
                        let new_chat_id = uuid::Uuid::new_v4().to_string();
                        let now = chrono::Utc::now().to_rfc3339();
                        tx.execute(
                            "INSERT INTO chats (id, peer_node_id, last_activity, created_at) VALUES (?1, ?2, ?3, ?4)",
                            rusqlite::params![&new_chat_id, &peer_node_id_clone, &now, &now]
                        )?;
                        new_chat_id
                    }
                    Err(e) => return Err(e),
                }
            };

            // Add the sent message
            let message_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();

            tx.execute(
                "INSERT INTO chat_messages (id, chat_id, space_id, pod_id, message_text, timestamp, direction) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'sent')",
                rusqlite::params![&message_id, &chat_id, &space_id_clone, &pod_id_clone, &message_text_clone, &now]
            )?;

            // Update chat last activity
            tx.execute(
                "UPDATE chats SET last_activity = ?1 WHERE id = ?2",
                rusqlite::params![&now, &chat_id]
            )?;

            tx.commit()?;
            Ok(message_id)
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for add_sent_message_to_chat")??;

    Ok(message_id)
}

/// Import a POD and add it to the inbox in a single transaction to avoid foreign key issues
pub async fn import_pod_and_add_to_inbox(
    db: &Db,
    data: &PodData,
    space_id: &str,
    from_node_id: &str,
    from_alias: Option<&str>,
    message_text: Option<&str>,
) -> Result<String> {
    let now = Utc::now().to_rfc3339();
    let pod_id = data.id();
    let data_blob =
        serde_json::to_vec(data).context("Failed to serialize PodData enum for storage")?;
    let message_id = uuid::Uuid::new_v4().to_string();

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    // Clone data for move closure
    let pod_id_clone = pod_id.clone();
    let data_blob_clone = data_blob;
    let space_id_clone = space_id.to_string();
    let from_node_id_clone = from_node_id.to_string();
    let from_alias_clone = from_alias.map(|s| s.to_string());
    let message_text_clone = message_text.map(|s| s.to_string());
    let message_id_clone = message_id.clone();
    let now_clone = now.clone();
    let pod_type_clone = data.type_str();

    conn.interact(move |conn| -> rusqlite::Result<String> {
        let tx = conn.transaction()?;

        // First, import the POD
        tx.execute(
            "INSERT INTO pods (id, data, created_at, space, pod_type) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![&pod_id_clone, &data_blob_clone, &now_clone, &space_id_clone, &pod_type_clone],
        )?;

        // Then add to inbox (foreign key constraint will be satisfied)
        tx.execute(
            "INSERT INTO inbox_messages (id, from_node_id, from_alias, space_id, pod_id, message_text, received_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
            rusqlite::params![
                &message_id_clone,
                &from_node_id_clone,
                &from_alias_clone,
                &space_id_clone,
                &pod_id_clone,
                &message_text_clone,
                &now_clone
            ],
        )?;

        tx.commit()?;
        Ok(message_id_clone)
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for import_pod_and_add_to_inbox")??;

    Ok(message_id)
}

/// List all pods across all spaces (for solver)
pub async fn list_all_pods(db: &Db) -> Result<Vec<PodInfo>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let pods = conn
        .interact(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, pod_type, data, label, created_at, space FROM pods ORDER BY created_at DESC"
            )?;
            let pod_iter = stmt.query_map([], |row| {
                let data_blob: Vec<u8> = row.get(2)?;
                let pod_data: PodData = serde_json::from_slice(&data_blob).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Blob,
                        Box::new(e),
                    )
                })?;
                Ok(PodInfo {
                    id: row.get(0)?,
                    pod_type: row.get(1)?,
                    data: pod_data,
                    label: row.get(3)?,
                    created_at: row.get(4)?,
                    space: row.get(5)?,
                })
            })?;
            pod_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for list_all_pods")??;

    Ok(pods)
}

// --- Identity Setup Functions ---

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct AppSetupState {
    pub setup_completed: bool,
    pub identity_server_url: Option<String>,
    pub identity_server_id: Option<String>,
    pub identity_server_public_key: Option<String>,
    pub username: Option<String>,
    pub identity_pod_id: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

/// Check if the app setup has been completed
pub async fn is_setup_completed(db: &Db) -> Result<bool> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let setup_completed = conn
        .interact(|conn| {
            let mut stmt =
                conn.prepare("SELECT setup_completed FROM app_setup_state WHERE id = 1")?;
            let result = stmt.query_row([], |row| row.get::<_, bool>(0));

            match result {
                Ok(completed) => Ok(completed),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false), // No setup record means not completed
                Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for is_setup_completed")??;

    Ok(setup_completed)
}

/// Get the current app setup state
pub async fn get_app_setup_state(db: &Db) -> Result<AppSetupState> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let setup_state = conn
        .interact(|conn| {
            let mut stmt = conn.prepare(
                "SELECT setup_completed, identity_server_url, identity_server_id, identity_server_public_key, username, identity_pod_id, completed_at, created_at FROM app_setup_state WHERE id = 1"
            )?;
            let result = stmt.query_row([], |row| {
                Ok(AppSetupState {
                    setup_completed: row.get(0)?,
                    identity_server_url: row.get(1)?,
                    identity_server_id: row.get(2)?,
                    identity_server_public_key: row.get(3)?,
                    username: row.get(4)?,
                    identity_pod_id: row.get(5)?,
                    completed_at: row.get(6)?,
                    created_at: row.get(7)?,
                })
            });

            match result {
                Ok(state) => Ok(state),
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Return default state if no record exists
                    Ok(AppSetupState {
                        setup_completed: false,
                        identity_server_url: None,
                        identity_server_id: None,
                        identity_server_public_key: None,
                        username: None,
                        identity_pod_id: None,
                        completed_at: None,
                        created_at: Utc::now().to_rfc3339(),
                    })
                }
                Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_app_setup_state")??;

    Ok(setup_state)
}

/// Update identity server info in the setup state
pub async fn update_identity_server_info(
    db: &Db,
    server_url: &str,
    server_id: &str,
    server_public_key: &str,
) -> Result<()> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let server_url_clone = server_url.to_string();
    let server_id_clone = server_id.to_string();
    let server_public_key_clone = server_public_key.to_string();

    conn.interact(move |conn| {
        conn.execute(
            "UPDATE app_setup_state SET identity_server_url = ?1, identity_server_id = ?2, identity_server_public_key = ?3 WHERE id = 1",
            rusqlite::params![server_url_clone, server_id_clone, server_public_key_clone],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for update_identity_server_info")??;

    Ok(())
}

/// Update username and identity pod info in the setup state
pub async fn update_identity_info(db: &Db, username: &str, identity_pod_id: &str) -> Result<()> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let username_clone = username.to_string();
    let identity_pod_id_clone = identity_pod_id.to_string();

    conn.interact(move |conn| {
        conn.execute(
            "UPDATE app_setup_state SET username = ?1, identity_pod_id = ?2 WHERE id = 1",
            rusqlite::params![username_clone, identity_pod_id_clone],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for update_identity_info")??;

    Ok(())
}

/// Mark the app setup as completed
pub async fn complete_app_setup(db: &Db) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    conn.interact(move |conn| {
        conn.execute(
            "UPDATE app_setup_state SET setup_completed = TRUE, completed_at = ?1 WHERE id = 1",
            rusqlite::params![now],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for complete_app_setup")??;

    Ok(())
}

/// Store an identity POD with mandatory flag
pub async fn store_identity_pod(
    db: &Db,
    pod_data: &PodData,
    space_id: &str,
    label: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let pod_id = pod_data.id();
    let data_blob =
        serde_json::to_vec(pod_data).context("Failed to serialize PodData enum for storage")?;

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    // Clone data for move closure
    let pod_id_clone = pod_id.clone();
    let data_blob_clone = data_blob;
    let space_id_clone = space_id.to_string();
    let label_clone = label.map(|s| s.to_string());
    let pod_type_clone = pod_data.type_str();

    conn.interact(move |conn| {
        conn.execute(
            "INSERT INTO pods (id, data, created_at, space, pod_type, label, is_mandatory) VALUES (?1, ?2, ?3, ?4, ?5, ?6, TRUE)",
            rusqlite::params![&pod_id_clone, &data_blob_clone, &now, &space_id_clone, &pod_type_clone, &label_clone],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for store_identity_pod")??;

    Ok(())
}

/// Get the default private key without checking setup completion (for internal use)
pub async fn get_default_private_key_raw(db: &Db) -> Result<SecretKey> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let key_hex = conn
        .interact(|conn| {
            let mut stmt =
                conn.prepare("SELECT private_key FROM private_keys WHERE is_default = TRUE")?;
            let result = stmt.query_row([], |row| row.get::<_, String>(0));

            match result {
                Ok(hex_string) => Ok(hex_string),
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    Err(anyhow::anyhow!("No default private key found"))
                }
                Err(e) => Err(anyhow::anyhow!("Database error: {}", e)),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_default_private_key_raw")??;

    let bytes = hex::decode(key_hex).context("Failed to decode private key hex")?;
    let big_uint = num::BigUint::from_bytes_be(&bytes);
    Ok(SecretKey(big_uint))
}

/// Create a default private key during the setup process
pub async fn create_default_private_key(db: &Db) -> Result<SecretKey> {
    let private_key = SecretKey::new_rand();
    let private_key_hex = hex::encode(private_key.0.to_bytes_be());
    let public_key_base58 = private_key.public_key().to_string();
    let now = Utc::now().to_rfc3339();

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let private_key_hex_clone = private_key_hex.clone();
    let public_key_base58_clone = public_key_base58.clone();

    conn.interact(move |conn| {
        // First check if a default key already exists
        let mut check_stmt = conn.prepare("SELECT COUNT(*) FROM private_keys WHERE is_default = TRUE")?;
        let count: i64 = check_stmt.query_row([], |row| row.get(0))?;

        if count > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                Some("Default private key already exists".to_string()),
            ));
        }

        conn.execute(
            "INSERT INTO private_keys (private_key, key_type, public_key, is_default, created_at) VALUES (?1, ?2, ?3, TRUE, ?4)",
            rusqlite::params![private_key_hex_clone, "Plonky2", public_key_base58_clone, now],
        )
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for create_default_private_key")??;

    log::info!("Created default private key during setup");
    Ok(private_key)
}

// --- Draft Management ---

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct DraftInfo {
    pub id: String, // UUID
    pub title: String,
    pub content_type: String, // "message", "file", or "url"
    pub message: Option<String>,
    pub file_name: Option<String>,
    pub file_content: Option<Vec<u8>>,
    pub file_mime_type: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub authors: Vec<String>,
    pub reply_to: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDraftRequest {
    pub title: String,
    pub content_type: String,
    pub message: Option<String>,
    pub file_name: Option<String>,
    pub file_content: Option<Vec<u8>>,
    pub file_mime_type: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub authors: Vec<String>,
    pub reply_to: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateDraftRequest {
    pub title: String,
    pub content_type: String,
    pub message: Option<String>,
    pub file_name: Option<String>,
    pub file_content: Option<Vec<u8>>,
    pub file_mime_type: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub authors: Vec<String>,
    pub reply_to: Option<String>,
}

/// Create a new draft
pub async fn create_draft(db: &Db, request: CreateDraftRequest) -> Result<String> {
    let draft_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let tags_json = serde_json::to_string(&request.tags)?;
    let authors_json = serde_json::to_string(&request.authors)?;

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let draft_id_clone = draft_id.clone();
    conn.interact(move |conn| -> Result<(), rusqlite::Error> {
        let mut stmt = conn.prepare(
            "INSERT INTO drafts (id, title, content_type, message, file_name, file_content, 
             file_mime_type, url, tags, authors, reply_to, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;

        stmt.execute(rusqlite::params![
            draft_id_clone,
            request.title,
            request.content_type,
            request.message,
            request.file_name,
            request.file_content,
            request.file_mime_type,
            request.url,
            tags_json,
            authors_json,
            request.reply_to,
            now,
            now
        ])?;

        Ok(())
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for create_draft")??;

    log::info!("Created new draft with UUID: {draft_id}");
    Ok(draft_id)
}

/// List all drafts ordered by updated_at DESC
pub async fn list_drafts(db: &Db) -> Result<Vec<DraftInfo>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let drafts = conn
        .interact(|conn| -> Result<Vec<DraftInfo>, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT id, title, content_type, message, file_name, file_content, 
                 file_mime_type, url, tags, authors, reply_to, created_at, updated_at 
                 FROM drafts ORDER BY updated_at DESC",
            )?;

            let draft_iter = stmt.query_map([], |row| {
                let tags_json: String = row.get(8)?;
                let authors_json: String = row.get(9)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::InvalidColumnType(
                        8,
                        format!("JSON parse error: {e}"),
                        rusqlite::types::Type::Text,
                    )
                })?;
                let authors: Vec<String> = serde_json::from_str(&authors_json).map_err(|e| {
                    rusqlite::Error::InvalidColumnType(
                        9,
                        format!("JSON parse error: {e}"),
                        rusqlite::types::Type::Text,
                    )
                })?;

                Ok(DraftInfo {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content_type: row.get(2)?,
                    message: row.get(3)?,
                    file_name: row.get(4)?,
                    file_content: row.get(5)?,
                    file_mime_type: row.get(6)?,
                    url: row.get(7)?,
                    tags,
                    authors,
                    reply_to: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?;

            draft_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for list_drafts")??;

    Ok(drafts)
}

/// Get a specific draft by ID
pub async fn get_draft(db: &Db, draft_id: &str) -> Result<Option<DraftInfo>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let draft_id_owned = draft_id.to_string();
    let draft = conn
        .interact(move |conn| -> Result<Option<DraftInfo>, rusqlite::Error> {
            let mut stmt = conn.prepare(
                "SELECT id, title, content_type, message, file_name, file_content, 
                 file_mime_type, url, tags, authors, reply_to, created_at, updated_at 
                 FROM drafts WHERE id = ?1",
            )?;

            let mut rows = stmt.query_map([&draft_id_owned], |row| {
                let tags_json: String = row.get(8)?;
                let authors_json: String = row.get(9)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::InvalidColumnType(
                        8,
                        format!("JSON parse error: {e}"),
                        rusqlite::types::Type::Text,
                    )
                })?;
                let authors: Vec<String> = serde_json::from_str(&authors_json).map_err(|e| {
                    rusqlite::Error::InvalidColumnType(
                        9,
                        format!("JSON parse error: {e}"),
                        rusqlite::types::Type::Text,
                    )
                })?;

                Ok(DraftInfo {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content_type: row.get(2)?,
                    message: row.get(3)?,
                    file_name: row.get(4)?,
                    file_content: row.get(5)?,
                    file_mime_type: row.get(6)?,
                    url: row.get(7)?,
                    tags,
                    authors,
                    reply_to: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?;

            match rows.next() {
                Some(draft) => Ok(Some(draft?)),
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_draft")??;

    Ok(draft)
}

/// Update an existing draft
pub async fn update_draft(db: &Db, draft_id: &str, request: UpdateDraftRequest) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let tags_json = serde_json::to_string(&request.tags)?;
    let authors_json = serde_json::to_string(&request.authors)?;

    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let draft_id_owned = draft_id.to_string();
    let rows_affected = conn
        .interact(move |conn| {
            conn.execute(
                "UPDATE drafts SET title = ?1, content_type = ?2, message = ?3, 
                 file_name = ?4, file_content = ?5, file_mime_type = ?6, url = ?7, 
                 tags = ?8, authors = ?9, reply_to = ?10, updated_at = ?11 
                 WHERE id = ?12",
                rusqlite::params![
                    request.title,
                    request.content_type,
                    request.message,
                    request.file_name,
                    request.file_content,
                    request.file_mime_type,
                    request.url,
                    tags_json,
                    authors_json,
                    request.reply_to,
                    now,
                    draft_id_owned
                ],
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for update_draft")??;

    Ok(rows_affected > 0)
}

/// Delete a draft by ID
pub async fn delete_draft(db: &Db, draft_id: &str) -> Result<bool> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;

    let draft_id_owned = draft_id.to_string();
    let rows_affected = conn
        .interact(move |conn| {
            conn.execute(
                "DELETE FROM drafts WHERE id = ?1",
                rusqlite::params![draft_id_owned],
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for delete_draft")??;

    Ok(rows_affected > 0)
}
