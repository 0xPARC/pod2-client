use anyhow::{Context, Result};
use chrono::Utc;

use crate::{
    api_types::{PodData, PodInfo, SpaceInfo},
    db::Db,
};

pub async fn create_space(db: &Db, id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let id_clone = id.to_string();

    let conn = db.pool().get().await.context("Failed to get DB connection")?;

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
    let conn = db.pool().get().await.context("Failed to get DB connection")?;

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
    let conn = db.pool().get().await.context("Failed to get DB connection")?;
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
    id: &str,
    pod_type: &str,
    data: &PodData,
    label: Option<&str>,
    space_id: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let data_blob =
        serde_json::to_vec(data).context("Failed to serialize PodData enum for storage")?;

    let conn = db.pool().get().await.context("Failed to get DB connection")?;

    let id_clone = id.to_string();
    let pod_type_clone = pod_type.to_string();
    let label_clone = label.map(|s| s.to_string());
    let space_id_clone = space_id.to_string();

    conn.interact(move |conn| {
        conn.execute(
            "INSERT INTO pods (id, pod_type, data, label, created_at, space) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id_clone,
                pod_type_clone,
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
    let conn = db.pool().get().await.context("Failed to get DB connection")?;
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
    let conn = db.pool().get().await.context("Failed to get DB connection")?;
    let space_id_clone = space_id.to_string();

    let pods = conn
        .interact(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, pod_type, data, label, created_at, space FROM pods WHERE space = ?1",
            )?;
            let pod_iter = stmt.query_map([&space_id_clone], |row| {
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
            })?;
            pod_iter.collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| anyhow::anyhow!("InteractError: {}", e))
        .context("DB interaction failed for list_pods")??;
    Ok(pods)
}

pub async fn delete_pod(db: &Db, space_id: &str, pod_id: &str) -> Result<usize> {
    let conn = db.pool().get().await.context("Failed to get DB connection")?;
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