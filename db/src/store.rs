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
