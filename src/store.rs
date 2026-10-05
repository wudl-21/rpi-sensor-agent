use anyhow::Result;
use rusqlite::Connection;

const DB_PATH: &str = "/var/lib/sensor-agent/samples.db";

pub fn open() -> Result<Connection> {
    if let Some(p) = std::path::Path::new(DB_PATH).parent() {
        std::fs::create_dir_all(p).map_err(|e| anyhow::anyhow!(e).context("mkdir state dir"))?;
    }

    let conn = Connection::open(DB_PATH)
        .map_err(|e| anyhow::anyhow!(e).context("open sqlite"))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            device_id TEXT NOT NULL,
            temperature REAL NOT NULL,
            humidity REAL,
            sent INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_samples_ts ON samples(ts);",
    )
    .map_err(|e| anyhow::anyhow!(e).context("create table"))?;

    Ok(conn)
}

pub fn insert(
    conn: &Connection,
    device_id: &str,
    temperature: f64,
    humidity: Option<f64>,
) -> Result<i64> {
    let ts = now_unix();
    conn.execute(
        "INSERT INTO samples (ts, device_id, temperature, humidity, sent)
         VALUES (?1, ?2, ?3, ?4, 0)",
        (ts, device_id, temperature, humidity),
    )
    .map_err(|e| anyhow::anyhow!(e).context("insert sample"))?;
    Ok(conn.last_insert_rowid())
}

pub fn mark_sent(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("UPDATE samples SET sent=1 WHERE id=?1", [id])
        .map_err(|e| anyhow::anyhow!(e).context("mark sent"))?;
    Ok(())
}

pub fn unsent(conn: &Connection) -> Result<Vec<(i64, f64, Option<f64>)>> {
    let mut stmt = conn.prepare(
        "SELECT id, temperature, humidity FROM samples WHERE sent=0 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
