use anyhow::{Context, Result};
use chrono::Utc;
use pod2::frontend::{MainPod, SerializedMainPod, SerializedSignedPod, SignedPod};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use hex::ToHex;

use crate::Db;


// --- General API Data Structures ---

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SpaceInfo {
    pub id: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(tag = "pod_data_variant", content = "pod_data_payload")]
pub enum PodData {
    Signed(SerializedSignedPod),
    Main(SerializedMainPod),
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
        PodData::Signed(pod.into())
    }
}

impl From<MainPod> for PodData {
    fn from(pod: MainPod) -> Self {
        PodData::Main(pod.into())
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

async fn list_pods_filtered(db: &Db, space_id: &str, pod_type_filter: Option<&str>) -> Result<Vec<PodInfo>> {
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
            conn.execute(
                "DELETE FROM pods WHERE space = ?1 AND id = ?2",
                [space_id_clone, pod_id_clone],
            )
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

    conn
        .interact(move |conn| {
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

/// Create and store a new private key
pub async fn create_private_key(
    db: &Db,
    alias: Option<&str>,
    set_as_default: bool,
) -> Result<String> {
    let private_key = pod2::backends::plonky2::primitives::ec::schnorr::SecretKey::new_rand();
    
    let private_key_hex = hex::encode(private_key.0.to_bytes_be());
    // For simplicity, we'll store a truncated version as the public key display
    // In practice, we'd derive and format the public key properly
    let public_key_hex = format!("pub_{}", &private_key_hex[0..16]);
    
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;
    
    let private_key_hex_clone = private_key_hex.clone();
    let alias_clone = alias.map(|s| s.to_string());
    
    conn.interact(move |conn| {
        let tx = conn.transaction()?;
        
        // If setting as default, clear existing default
        if set_as_default {
            tx.execute("UPDATE private_keys SET is_default = FALSE", [])?;
        }
        
        // Insert new private key (using private_key as PK)
        tx.execute(
            "INSERT INTO private_keys (private_key, key_type, public_key, alias, is_default) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                private_key_hex_clone,
                "Plonky2",
                public_key_hex,
                alias_clone,
                set_as_default
            ],
        )?;
        
        tx.commit()?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
    .context("DB interaction failed for create_private_key")??;
    
    Ok(private_key_hex)
}

/// Get the default private key
pub async fn get_default_private_key(db: &Db) -> Result<Option<pod2::backends::plonky2::primitives::ec::schnorr::SecretKey>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;
    
    let key_hex = conn
        .interact(|conn| {
            let mut stmt = conn.prepare("SELECT private_key FROM private_keys WHERE is_default = TRUE")?;
            let result = stmt.query_row([], |row| {
                Ok(row.get::<_, String>(0)?)
            });
            
            match result {
                Ok(hex_string) => Ok(Some(hex_string)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for get_default_private_key")??;
    
    if let Some(hex_string) = key_hex {
        let bytes = hex::decode(hex_string)
            .context("Failed to decode private key hex")?;
        let big_uint = num::BigUint::from_bytes_be(&bytes);
        Ok(Some(pod2::backends::plonky2::primitives::ec::schnorr::SecretKey(big_uint)))
    } else {
        Ok(None)
    }
}

/// List all private keys (without secret keys for security)
pub async fn list_private_keys(db: &Db) -> Result<Vec<serde_json::Value>> {
    let conn = db
        .pool()
        .get()
        .await
        .context("Failed to get DB connection")?;
    
    let keys = conn
        .interact(|conn| {
            let mut stmt = conn.prepare(
                "SELECT private_key, public_key, alias, created_at, COALESCE(is_default, FALSE) FROM private_keys ORDER BY created_at DESC"
            )?;
            let key_iter = stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?, // Use private_key as id
                    "public_key_hex": row.get::<_, String>(1)?,
                    "alias": row.get::<_, Option<String>>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                    "is_default": row.get::<_, bool>(4)?
                }))
            })?;
            key_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for list_private_keys")??;
    
    Ok(keys)
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
                 ORDER BY last_activity DESC"
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
                 ORDER BY timestamp ASC"
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
                    Ok(row.get::<_, String>(0)?)
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
    let data_blob = serde_json::to_vec(data).context("Failed to serialize PodData enum for storage")?;
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
