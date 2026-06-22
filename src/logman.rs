use std::sync::Arc;
use tokio::time::{sleep, Duration};
use sqlx::SqlitePool;

pub struct LogManager {
    pool: SqlitePool,
}

impl LogManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            println!("[LogManager] Starting background log rotation loop...");
            // Initial delay
            sleep(Duration::from_secs(30)).await;
            loop {
                if let Err(e) = self.check_and_run().await {
                    eprintln!("[LogManager] Error: {:?}", e);
                }
                sleep(Duration::from_secs(60)).await;
            }
        });
    }

    async fn get_setting(&self, key: &str) -> String {
        let val: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .unwrap_or(None);
        val.map(|v| v.0).unwrap_or_default()
    }

    async fn save_setting(&self, key: &str, value: &str) {
        let _ = sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await;
    }

    async fn check_and_run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let interval_hours = self.get_setting("log_rotation_interval_hours").await
            .parse::<i64>().unwrap_or(24);
        let last_run_str = self.get_setting("log_last_rotation").await;

        let now = chrono::Utc::now();
        let should_run = if last_run_str.is_empty() {
            true
        } else if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&last_run_str) {
            now.signed_duration_since(last.with_timezone(&chrono::Utc)).num_hours() >= interval_hours
        } else {
            true
        };

        if should_run {
            println!("[LogManager] Log rotation is due. Starting...");
            let max_size_mb = self.get_setting("log_max_size_mb").await
                .parse::<u64>().unwrap_or(10);
            let waf_retention_days = self.get_setting("waf_log_retention_days").await
                .parse::<i64>().unwrap_or(30);

            let summary = self.run_rotation(max_size_mb, waf_retention_days).await;
            let ts = chrono::Utc::now().to_rfc3339();
            self.save_setting("log_last_rotation", &ts).await;
            self.save_setting("log_last_status", &summary).await;
            println!("[LogManager] Rotation done: {}", summary);
        }

        Ok(())
    }

    /// Run rotation synchronously. Returns a human-readable summary.
    pub async fn run_rotation(&self, max_size_mb: u64, waf_retention_days: i64) -> String {
        let base = std::env::var("ZENO_CONTAINER_DATA_DIR")
            .unwrap_or_else(|_| "/var/lib/zeno-container".to_string());
        let containers_dir = std::path::PathBuf::from(&base).join("containers");

        let mut rotated_count = 0usize;
        let mut errors: Vec<String> = Vec::new();

        // ── 1. Rotate container console.log files ────────────────────────────
        if containers_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&containers_dir) {
                for entry in entries.flatten() {
                    let log_path = entry.path().join("console.log");
                    if !log_path.exists() { continue; }
                    if let Ok(meta) = std::fs::metadata(&log_path) {
                        let size_mb = meta.len() / (1024 * 1024);
                        if size_mb >= max_size_mb {
                            match Self::rotate_log_file(&log_path) {
                                Ok(_) => rotated_count += 1,
                                Err(e) => errors.push(format!("{}: {}", log_path.display(), e)),
                            }
                        }
                    }
                }
            }
        }

        // ── 2. Clean up old WAF log entries from SQLite ───────────────────────
        let mut waf_deleted = 0i64;
        let table_exists: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='waf_logs'"
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);

        if table_exists.is_some() {
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(waf_retention_days)).to_rfc3339();
            match sqlx::query("DELETE FROM waf_logs WHERE created_at < ?")
                .bind(&cutoff)
                .execute(&self.pool)
                .await
            {
                Ok(res) => waf_deleted = res.rows_affected() as i64,
                Err(e) => errors.push(format!("waf_logs cleanup: {}", e)),
            }
        }

        format!(
            "Rotated {} log file(s), deleted {} WAF log entries{}",
            rotated_count,
            waf_deleted,
            if errors.is_empty() { String::new() } else { format!(". Errors: {}", errors.join("; ")) }
        )
    }

    /// Rotate a single log file: .log → .1 → .2 → .3 (max 3 old copies).
    fn rotate_log_file(log_path: &std::path::Path) -> std::io::Result<()> {
        let parent = log_path.parent().unwrap_or(std::path::Path::new("."));
        let stem = "console";

        let gen3 = parent.join(format!("{}.log.3", stem));
        let gen2 = parent.join(format!("{}.log.2", stem));
        let gen1 = parent.join(format!("{}.log.1", stem));

        if gen3.exists() { std::fs::remove_file(&gen3)?; }
        if gen2.exists() { std::fs::rename(&gen2, &gen3)?; }
        if gen1.exists() { std::fs::rename(&gen1, &gen2)?; }
        std::fs::rename(log_path, &gen1)?;

        // Create fresh empty log
        std::fs::File::create(log_path)?;
        Ok(())
    }
}
