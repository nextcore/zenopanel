use std::sync::Arc;
use tokio::time::{sleep, Duration};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::fs::File;

pub struct BackupManager {
    pool: SqlitePool,
}

impl BackupManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            println!("[BackupManager] Starting background loop...");
            // Initial delay before first check
            sleep(Duration::from_secs(15)).await;
            loop {
                if let Err(e) = self.check_and_run_backup().await {
                    eprintln!("[BackupManager] Error checking/running backup: {:?}", e);
                }
                // Sleep for 1 minute before checking again
                sleep(Duration::from_secs(60)).await;
            }
        });
    }

    async fn get_setting(&self, key: &str) -> Result<String, sqlx::Error> {
        let val: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(val.map(|v| v.0).unwrap_or_default())
    }

    async fn save_setting(&self, key: &str, value: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn check_and_run_backup(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check and execute scheduled database backups first
        let _ = self.check_and_run_db_backups().await;

        let enabled = self.get_setting("backup_enabled").await? == "true";
        if !enabled {
            return Ok(());
        }

        let interval_hours = self.get_setting("backup_interval_hours").await?.parse::<i64>().unwrap_or(24);
        let last_run_str = self.get_setting("backup_last_run").await?;

        let now = chrono::Utc::now();
        let mut run_now = false;

        if last_run_str.is_empty() {
            run_now = true;
        } else if let Ok(last_run) = chrono::DateTime::parse_from_rfc3339(&last_run_str) {
            let duration = now.signed_duration_since(last_run.with_timezone(&chrono::Utc));
            if duration.num_hours() >= interval_hours {
                run_now = true;
            }
        } else {
            run_now = true;
        }

        if run_now {
            println!("[BackupManager] Automated backup is due. Starting backup run...");
            match self.run_backup().await {
                Ok(filename) => {
                    let run_time = chrono::Utc::now().to_rfc3339();
                    let _ = self.save_setting("backup_last_run", &run_time).await;
                    let _ = self.save_setting("backup_last_status", &format!("Success (Filename: {})", filename)).await;
                    println!("[BackupManager] Automated backup completed successfully: {}", filename);
                }
                Err(e) => {
                    let run_time = chrono::Utc::now().to_rfc3339();
                    let _ = self.save_setting("backup_last_run", &run_time).await;
                    let _ = self.save_setting("backup_last_status", &format!("Failed: {}", e)).await;
                    eprintln!("[BackupManager] Automated backup failed: {:?}", e);
                }
            }
        }

        Ok(())
    }

    pub async fn run_backup(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let dest_dir_str = self.get_setting("backup_dest_dir").await.unwrap_or_else(|_| "/var/lib/zenopanel/backups".to_string());
        let dest_dir = Path::new(&dest_dir_str);
        if !dest_dir.exists() {
            std::fs::create_dir_all(dest_dir)?;
        }

        let retention = self.get_setting("backup_retention").await.unwrap_or_default().parse::<usize>().unwrap_or(7);
        let post_script = self.get_setting("backup_post_script").await.unwrap_or_default();

        let now = chrono::Utc::now();
        let timestamp = now.format("%Y%m%d_%H%M%S").to_string();
        let backup_filename = format!("zeno_backup_{}.tar.gz", timestamp);
        let backup_path = dest_dir.join(&backup_filename);

        println!("[BackupManager] Backing up to {}", backup_path.display());

        // 1. Vacuum SQLite database to a temp file
        let temp_dir = std::env::temp_dir();
        let temp_db_path = temp_dir.join(format!("zeno_backup_vacuum_{}.db", timestamp));

        let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "./zeno.db".to_string());
        let mut db_path = PathBuf::from(&db_name);
        if db_path.is_relative() {
            if let Ok(cwd) = std::env::current_dir() {
                db_path = cwd.join(db_path);
            }
        }

        // Run SQLite VACUUM INTO
        sqlx::query(&format!("VACUUM INTO '{}'", temp_db_path.to_string_lossy().replace('\'', "''")))
            .execute(&self.pool)
            .await?;

        // 2. Collect container volume directories
        let data_dir = std::env::var("ZENO_CONTAINER_DATA_DIR")
            .unwrap_or_else(|_| "/var/lib/zeno-container".to_string());
        let volume_dirs = get_all_container_mount_dirs(&data_dir);

        // 3. Create .tar.gz archive
        {
            let tar_file = File::create(&backup_path)?;
            let enc = flate2::write::GzEncoder::new(tar_file, flate2::Compression::default());
            let mut archive = tar::Builder::new(enc);

            // Add the database copy to the archive using its original path structure (without leading /)
            if temp_db_path.exists() {
                let clean_db_path = db_path.to_string_lossy();
                let clean_db_path_stripped = clean_db_path.strip_prefix('/').unwrap_or(&clean_db_path);
                archive.append_path_with_name(&temp_db_path, clean_db_path_stripped)?;
            }

            // Add each volume directory recursively
            for v_dir in volume_dirs {
                let path = Path::new(&v_dir);
                if path.exists() {
                    let clean_v_dir = v_dir.strip_prefix('/').unwrap_or(&v_dir);
                    if path.is_file() {
                        if let Err(e) = archive.append_path_with_name(path, clean_v_dir) {
                            eprintln!("[BackupManager] Warning: failed to back up volume file {}: {:?}", v_dir, e);
                        }
                    } else if path.is_dir() {
                        if let Err(e) = archive.append_dir_all(clean_v_dir, path) {
                            eprintln!("[BackupManager] Warning: failed to back up volume dir {}: {:?}", v_dir, e);
                        }
                    }
                }
            }

            archive.finish()?;
        }

        // Clean up temp database
        if temp_db_path.exists() {
            let _ = std::fs::remove_file(temp_db_path);
        }

        // 4. Run post-exec script if configured
        if !post_script.is_empty() {
            let script_path = Path::new(&post_script);
            if script_path.exists() {
                println!("[BackupManager] Running post-backup script: {}", post_script);
                let output = tokio::process::Command::new(script_path)
                    .arg(&backup_path)
                    .output()
                    .await;
                match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        println!("[BackupManager] Post-backup script stdout: {}", stdout);
                        if !out.status.success() {
                            eprintln!("[BackupManager] Post-backup script failed (exit status: {:?}). Stderr: {}", out.status.code(), stderr);
                        }
                    }
                    Err(e) => {
                        eprintln!("[BackupManager] Failed to run post-backup script: {:?}", e);
                    }
                }
            } else {
                println!("[BackupManager] Post-backup script configured but does not exist at: {}", post_script);
            }
        }

        // 5. Clean up old backups based on retention policy
        if let Ok(entries) = std::fs::read_dir(dest_dir) {
            let mut backup_files = Vec::new();
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.starts_with("zeno_backup_") && filename.ends_with(".tar.gz") {
                    backup_files.push(entry.path());
                }
            }

            backup_files.sort();
            if backup_files.len() > retention {
                let delete_count = backup_files.len() - retention;
                for file_to_delete in backup_files.iter().take(delete_count) {
                    println!("[BackupManager] Retention cleanup: deleting old backup {}", file_to_delete.display());
                    let _ = std::fs::remove_file(file_to_delete);
                }
            }
        }

        Ok(backup_filename)
    }
}

fn get_all_container_mount_dirs(data_dir: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    let containers_dir = format!("{}/containers", data_dir);
    if let Ok(entries) = std::fs::read_dir(containers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let state_file = path.join("state.json");
                if state_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(state_file) {
                        if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(mounts) = state.get("mounts").and_then(|m| m.as_array()) {
                                for m in mounts {
                                    if let Some(mount_str) = m.as_str() {
                                        let parts: Vec<&str> = mount_str.split(':').collect();
                                        if !parts.is_empty() {
                                            let host_path = parts[0].to_string();
                                            if !host_path.is_empty() && Path::new(&host_path).exists() {
                                                dirs.push(host_path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct DbBackupSchedule {
    id: i64,
    database_name: String,
    schedule: String,
    retention: i64,
    is_active: i64,
    last_run: Option<String>,
}

impl BackupManager {
    async fn check_and_run_db_backups(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let active_schedules: Vec<DbBackupSchedule> = sqlx::query_as(
            "SELECT id, database_name, schedule, retention, is_active, last_run FROM db_backup_schedules WHERE is_active = 1"
        )
        .fetch_all(&self.pool)
        .await?;

        let now = chrono::Utc::now();
        for sched in active_schedules {
            let mut run_now = false;
            if sched.last_run.is_none() || sched.last_run.as_ref().unwrap().is_empty() {
                run_now = true;
            } else if let Some(ref last_run_str) = sched.last_run {
                if let Ok(last_run) = chrono::DateTime::parse_from_rfc3339(last_run_str) {
                    let duration = now.signed_duration_since(last_run.with_timezone(&chrono::Utc));
                    let due = match sched.schedule.as_str() {
                        "hourly" => duration.num_hours() >= 1,
                        "daily" => duration.num_hours() >= 24,
                        "weekly" => duration.num_hours() >= 168,
                        _ => false,
                    };
                    if due {
                        run_now = true;
                    }
                } else {
                    run_now = true;
                }
            }

            if run_now {
                println!("[BackupManager] DB Auto Backup due for database: {}", sched.database_name);
                if let Err(err) = self.execute_db_backup(&sched).await {
                    eprintln!("[BackupManager] DB Auto Backup failed for {}: {:?}", sched.database_name, err);
                }
            }
        }
        Ok(())
    }

    async fn execute_db_backup(&self, sched: &DbBackupSchedule) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let dest_setting: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'backup_dest_dir'")
            .fetch_optional(&self.pool)
            .await?;
        let dest_dir_str = dest_setting.map(|v| v.0).unwrap_or_else(|| "/var/lib/zenopanel/backups".to_string());
        let db_backup_dir = Path::new(&dest_dir_str).join("db");
        if !db_backup_dir.exists() {
            std::fs::create_dir_all(&db_backup_dir)?;
        }

        let now = chrono::Utc::now();
        let timestamp = now.format("%Y%m%d_%H%M%S").to_string();

        let (driver, host, port, user, password, server_name) = if sched.database_name == "default" {
            ("sqlite".to_string(), "".to_string(), 0i32, "".to_string(), "".to_string(), "Default Panel DB (SQLite)".to_string())
        } else {
            // Find DB info and Server credentials
            let info: Option<(String, String, i32, String, String, String)> = sqlx::query_as(
                "SELECT s.driver, s.host, s.port, s.admin_user, s.admin_password, s.name FROM db_databases d JOIN db_servers s ON d.server_id = s.id WHERE d.db_name = ?"
            )
            .bind(&sched.database_name)
            .fetch_optional(&self.pool)
            .await?;
            
            match info {
                Some((drv, hst, prt, usr, pwd, srv)) => (drv, hst, prt, usr, pwd, srv),
                None => return Err(format!("Database info not found for database: {}", sched.database_name).into()),
            }
        };

        let filename = if driver == "sqlite" {
            format!("backup_default_{}.db", timestamp)
        } else {
            format!("backup_{}_{}.sql", sched.database_name, timestamp)
        };
        let backup_path = db_backup_dir.join(&filename);

        println!("[BackupManager] Auto backing up DB {} ({}) to {}", sched.database_name, driver, backup_path.display());

        let res = if driver == "sqlite" {
            // vacuum sqlite
            sqlx::query(&format!("VACUUM INTO '{}'", backup_path.to_string_lossy().replace('\'', "''")))
                .execute(&self.pool)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        } else if driver == "mysql" {
            let file = std::fs::File::create(&backup_path)
                .map_err(|e| format!("Failed to create backup file: {}", e))?;
            let mut cmd = std::process::Command::new("mysqldump");
            cmd.arg("-h").arg(&host)
               .arg("-P").arg(&port.to_string())
               .arg("-u").arg(&user);
            if !password.is_empty() {
                cmd.arg(format!("-p{}", password));
            }
            let output = cmd.arg(&sched.database_name)
               .stdout(std::process::Stdio::from(file))
               .output();
            match output {
                Ok(out) => {
                    if out.status.success() {
                        Ok(())
                    } else {
                        let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                        let _ = std::fs::remove_file(&backup_path);
                        Err(format!("mysqldump failed: {}", err_msg))
                    }
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&backup_path);
                    Err(format!("Failed to start mysqldump: {}", e))
                }
            }
        } else if driver == "postgres" {
            let file = std::fs::File::create(&backup_path)
                .map_err(|e| format!("Failed to create backup file: {}", e))?;
            let mut cmd = std::process::Command::new("pg_dump");
            if !password.is_empty() {
                cmd.env("PGPASSWORD", &password);
            }
            cmd.arg("-h").arg(&host)
               .arg("-p").arg(&port.to_string())
               .arg("-U").arg(&user)
               .arg(&sched.database_name)
               .stdout(std::process::Stdio::from(file));
            let output = cmd.output();
            match output {
                Ok(out) => {
                    if out.status.success() {
                        Ok(())
                    } else {
                        let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                        let _ = std::fs::remove_file(&backup_path);
                        Err(format!("pg_dump failed: {}", err_msg))
                    }
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&backup_path);
                    Err(format!("Failed to start pg_dump: {}", e))
                }
            }
        } else {
            Err(format!("Unsupported database driver: {}", driver))
        };

        match res {
            Ok(()) => {
                let size = std::fs::metadata(&backup_path).map(|m| m.len()).unwrap_or(0);
                sqlx::query("INSERT INTO db_backups (database_name, server_name, filename, size, status) VALUES (?, ?, ?, ?, 'success')")
                    .bind(&sched.database_name)
                    .bind(&server_name)
                    .bind(&filename)
                    .bind(size as i64)
                    .execute(&self.pool)
                    .await?;

                // Clean up old backups based on retention policy
                if let Ok(entries) = std::fs::read_dir(&db_backup_dir) {
                    let mut db_backup_files = Vec::new();
                    let prefix = if driver == "sqlite" {
                        "backup_default_".to_string()
                    } else {
                        format!("backup_{}_", sched.database_name)
                    };
                    let suffix = if driver == "sqlite" {
                        ".db".to_string()
                    } else {
                        ".sql".to_string()
                    };

                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.starts_with(&prefix) && fname.ends_with(&suffix) {
                            db_backup_files.push(entry.path());
                        }
                    }

                    db_backup_files.sort();
                    let retention = sched.retention as usize;
                    if db_backup_files.len() > retention {
                        let delete_count = db_backup_files.len() - retention;
                        for file_to_delete in db_backup_files.iter().take(delete_count) {
                            println!("[BackupManager] Retention cleanup: deleting old db backup {}", file_to_delete.display());
                            let _ = std::fs::remove_file(file_to_delete);
                            
                            // Delete record from SQLite too so it is in sync
                            if let Some(f_name) = file_to_delete.file_name().and_then(|f| f.to_str()) {
                                let _ = sqlx::query("DELETE FROM db_backups WHERE filename = ?")
                                    .bind(f_name)
                                    .execute(&self.pool)
                                    .await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                sqlx::query("INSERT INTO db_backups (database_name, server_name, filename, size, status) VALUES (?, ?, ?, 0, ?)")
                    .bind(&sched.database_name)
                    .bind(&server_name)
                    .bind(&filename)
                    .bind(e)
                    .execute(&self.pool)
                    .await?;
            }
        }

        // Update last_run time
        let run_time = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE db_backup_schedules SET last_run = ? WHERE id = ?")
            .bind(&run_time)
            .bind(sched.id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
