use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::SqlitePool;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    pub id: i64,
    pub name: String,
    pub os_type: String, // "linux", "windows"
    pub vcpus: u32,
    pub memory_mb: u64,
    pub disk_path: String,
    pub disk_size_gb: u64,
    pub tap_device: String,
    pub ip_address: String,
    pub status: String, // "running", "stopped", "paused", "error"
    pub socket_path: String,
    pub ssh_key: String,
    pub root_password: String,
    pub iso_path: String,
    pub created_at: String,
}

pub struct MachineState {
    pub info: MachineInfo,
    pub pid: Option<u32>,
    pub stop_requested: bool,
}

#[derive(Clone)]
pub struct MachineManager {
    pool: SqlitePool,
    machines: Arc<RwLock<HashMap<String, Arc<RwLock<MachineState>>>>>,
    binary_path: String,
}

impl MachineManager {
    pub async fn new(pool: SqlitePool) -> Self {
        let create_table_query = "
            CREATE TABLE IF NOT EXISTS db_machines (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                os_type TEXT NOT NULL DEFAULT 'linux',
                vcpus INTEGER NOT NULL DEFAULT 2,
                memory_mb INTEGER NOT NULL DEFAULT 1024,
                disk_path TEXT NOT NULL,
                disk_size_gb INTEGER NOT NULL DEFAULT 10,
                tap_device TEXT NOT NULL DEFAULT '',
                ip_address TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'stopped',
                socket_path TEXT NOT NULL DEFAULT '',
                ssh_key TEXT DEFAULT '',
                root_password TEXT DEFAULT '',
                iso_path TEXT DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS db_isos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                path TEXT NOT NULL,
                source_url TEXT DEFAULT '',
                status TEXT NOT NULL DEFAULT 'ready',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS db_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                machine_name TEXT NOT NULL,
                snapshot_name TEXT NOT NULL,
                description TEXT DEFAULT '',
                file_path TEXT NOT NULL,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS db_migration_requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_host TEXT NOT NULL,
                machine_name TEXT NOT NULL,
                os_type TEXT NOT NULL DEFAULT 'linux',
                vcpus INTEGER NOT NULL DEFAULT 2,
                memory_mb INTEGER NOT NULL DEFAULT 1024,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
        ";
        if let Err(e) = sqlx::query(create_table_query).execute(&pool).await {
            eprintln!("Failed to create db_machines, db_isos, db_snapshots & migration tables: {}", e);
        }

        let _ = sqlx::query("ALTER TABLE db_machines ADD COLUMN ssh_key TEXT DEFAULT ''").execute(&pool).await;
        let _ = sqlx::query("ALTER TABLE db_machines ADD COLUMN root_password TEXT DEFAULT ''").execute(&pool).await;
        let _ = sqlx::query("ALTER TABLE db_machines ADD COLUMN iso_path TEXT DEFAULT ''").execute(&pool).await;


        // Tentukan lokasi biner cloud-hypervisor terisolasi di folder ZenoPanel
        let binary_path = match std::env::current_exe() {
            Ok(exe) => {
                if let Some(parent) = exe.parent() {
                    parent.join("bin/cloud-hypervisor").to_string_lossy().to_string()
                } else {
                    "/opt/zenopanel/bin/cloud-hypervisor".to_string()
                }
            }
            Err(_) => "/opt/zenopanel/bin/cloud-hypervisor".to_string(),
        };

        let manager = Self {
            pool,
            machines: Arc::new(RwLock::new(HashMap::new())),
            binary_path,
        };

        if let Err(e) = manager.load_from_db().await {
            eprintln!("Failed to load machines from database: {}", e);
        }

        manager
    }

    pub fn binary_path(&self) -> &str {
        &self.binary_path
    }

    pub async fn load_from_db(&self) -> Result<(), String> {

        let rows = sqlx::query(
            "SELECT id, name, os_type, vcpus, memory_mb, disk_path, disk_size_gb, tap_device, ip_address, status, socket_path, ssh_key, root_password, iso_path, created_at FROM db_machines"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut map = self.machines.write().await;
        for row in rows {
            use sqlx::Row;
            let id: i64 = row.get("id");
            let name: String = row.get("name");
            let os_type: String = row.get("os_type");
            let vcpus: i32 = row.get("vcpus");
            let memory_mb: i64 = row.get("memory_mb");
            let disk_path: String = row.get("disk_path");
            let disk_size_gb: i64 = row.get("disk_size_gb");
            let tap_device: String = row.get("tap_device");
            let ip_address: String = row.get("ip_address");
            let status: String = row.get("status");
            let socket_path: String = row.get("socket_path");
            let ssh_key: String = row.try_get("ssh_key").unwrap_or_default();
            let root_password: String = row.try_get("root_password").unwrap_or_default();
            let iso_path: String = row.try_get("iso_path").unwrap_or_default();
            let created_at: String = row.try_get("created_at").unwrap_or_else(|_| "".to_string());

            let info = MachineInfo {
                id,
                name: name.clone(),
                os_type,
                vcpus: vcpus as u32,
                memory_mb: memory_mb as u64,
                disk_path,
                disk_size_gb: disk_size_gb as u64,
                tap_device,
                ip_address,
                status,
                socket_path,
                ssh_key,
                root_password,
                iso_path,
                created_at,
            };

            map.insert(
                name.clone(),
                Arc::new(RwLock::new(MachineState {
                    info,
                    pid: None,
                    stop_requested: false,
                })),
            );
        }

        Ok(())
    }

    pub async fn list_machines(&self) -> Vec<MachineInfo> {
        let machines = self.machines.read().await;
        let mut list = Vec::new();
        for state_arc in machines.values() {
            let state = state_arc.read().await;
            list.push(state.info.clone());
        }
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    pub async fn get_machine(&self, name: &str) -> Option<MachineInfo> {
        let machines = self.machines.read().await;
        if let Some(state_arc) = machines.get(name) {
            let state = state_arc.read().await;
            Some(state.info.clone())
        } else {
            None
        }
    }

    pub async fn create_machine(
        &self,
        name: String,
        os_type: String,
        vcpus: u32,
        memory_mb: u64,
        disk_size_gb: u64,
        tap_device: String,
        ip_address: String,
        ssh_key: String,
        root_password: String,
        iso_path: String,
    ) -> Result<MachineInfo, String> {
        let clean_name = name.trim().to_lowercase().replace(' ', "-");
        if clean_name.is_empty() {
            return Err("Nama Zeno Machine tidak boleh kosong".to_string());
        }

        let disk_path = format!("/var/lib/zeno-container/machines/{}.img", clean_name);
        let socket_path = format!("/run/zeno-machine/{}.sock", clean_name);

        // Buat folder penyimpan disk, socket & cloud-init jika belum ada
        let _ = std::fs::create_dir_all("/var/lib/zeno-container/machines");
        let _ = std::fs::create_dir_all("/run/zeno-machine");
        let cloud_init_dir = format!("/var/lib/zeno-container/machines/cloud-init/{}", clean_name);
        let _ = std::fs::create_dir_all(&cloud_init_dir);

        // Tulis konfigurasi Cloud-Init user-data jika ssh_key / password diberikan
        if !ssh_key.is_empty() || !root_password.is_empty() {
            let mut user_data = String::from("#cloud-config\n");
            if !root_password.is_empty() {
                user_data.push_str(&format!("password: {}\nchpasswd: {{ expire: False }}\nssh_pwauth: True\n", root_password));
            }
            if !ssh_key.is_empty() {
                user_data.push_str(&format!("ssh_authorized_keys:\n  - {}\n", ssh_key.trim()));
            }
            let _ = std::fs::write(format!("{}/user-data", cloud_init_dir), user_data);
            let _ = std::fs::write(format!("{}/meta-data", cloud_init_dir), format!("instance-id: {}\nlocal-hostname: {}\n", clean_name, clean_name));
        }

        let res = sqlx::query(
            "INSERT INTO db_machines (name, os_type, vcpus, memory_mb, disk_path, disk_size_gb, tap_device, ip_address, status, socket_path, ssh_key, root_password, iso_path) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'stopped', ?, ?, ?, ?)"
        )
        .bind(&clean_name)
        .bind(&os_type)
        .bind(vcpus as i32)
        .bind(memory_mb as i64)
        .bind(&disk_path)
        .bind(disk_size_gb as i64)
        .bind(&tap_device)
        .bind(&ip_address)
        .bind(&socket_path)
        .bind(&ssh_key)
        .bind(&root_password)
        .bind(&iso_path)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Gagal menyimpan Zeno Machine ke database: {}", e))?;

        let id = res.last_insert_rowid();
        let now_str = chrono::Utc::now().to_rfc3339();

        let info = MachineInfo {
            id,
            name: clean_name.clone(),
            os_type,
            vcpus,
            memory_mb,
            disk_path,
            disk_size_gb,
            tap_device,
            ip_address,
            status: "stopped".to_string(),
            socket_path,
            ssh_key,
            root_password,
            iso_path,
            created_at: now_str,
        };

        let state = Arc::new(RwLock::new(MachineState {
            info: info.clone(),
            pid: None,
            stop_requested: false,
        }));

        self.machines.write().await.insert(clean_name, state);
        Ok(info)
    }

    pub async fn start_machine(&self, name: &str) -> Result<(), String> {
        let state_arc = {
            let machines = self.machines.read().await;
            machines.get(name).cloned().ok_or_else(|| format!("Zeno Machine '{}' tidak ditemukan", name))?
        };

        let mut state = state_arc.write().await;
        if state.info.status == "running" {
            return Ok(());
        }

        // Perbarui status ke database & memori
        state.info.status = "running".to_string();
        let _ = sqlx::query("UPDATE db_machines SET status = 'running' WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(())
    }

    pub async fn stop_machine(&self, name: &str) -> Result<(), String> {
        let state_arc = {
            let machines = self.machines.read().await;
            machines.get(name).cloned().ok_or_else(|| format!("Zeno Machine '{}' tidak ditemukan", name))?
        };

        let mut state = state_arc.write().await;
        state.info.status = "stopped".to_string();
        state.pid = None;

        let _ = sqlx::query("UPDATE db_machines SET status = 'stopped' WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(())
    }

    pub async fn pause_machine(&self, name: &str) -> Result<(), String> {
        let state_arc = {
            let machines = self.machines.read().await;
            machines.get(name).cloned().ok_or_else(|| format!("Zeno Machine '{}' tidak ditemukan", name))?
        };

        let mut state = state_arc.write().await;
        state.info.status = "paused".to_string();

        let _ = sqlx::query("UPDATE db_machines SET status = 'paused' WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(())
    }

    pub async fn resume_machine(&self, name: &str) -> Result<(), String> {
        let state_arc = {
            let machines = self.machines.read().await;
            machines.get(name).cloned().ok_or_else(|| format!("Zeno Machine '{}' tidak ditemukan", name))?
        };

        let mut state = state_arc.write().await;
        state.info.status = "running".to_string();

        let _ = sqlx::query("UPDATE db_machines SET status = 'running' WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(())
    }

    pub async fn resize_machine(&self, name: &str, vcpus: Option<u32>, memory_mb: Option<u64>) -> Result<(), String> {
        let state_arc = {
            let machines = self.machines.read().await;
            machines.get(name).cloned().ok_or_else(|| format!("Zeno Machine '{}' tidak ditemukan", name))?
        };

        let mut state = state_arc.write().await;
        if let Some(c) = vcpus {
            state.info.vcpus = c;
        }
        if let Some(m) = memory_mb {
            state.info.memory_mb = m;
        }

        let _ = sqlx::query("UPDATE db_machines SET vcpus = ?, memory_mb = ? WHERE name = ?")
            .bind(state.info.vcpus as i32)
            .bind(state.info.memory_mb as i64)
            .bind(name)
            .execute(&self.pool)
            .await;

        Ok(())
    }

    pub async fn migrate_machine(&self, name: &str, target_host: &str, target_port: u16) -> Result<(), String> {
        let state_arc = {
            let machines = self.machines.read().await;
            machines.get(name).cloned().ok_or_else(|| format!("Zeno Machine '{}' tidak ditemukan", name))?
        };

        let state = state_arc.read().await;
        if state.info.status != "running" {
            return Err(format!("Zeno Machine '{}' harus dalam keadaan 'running' untuk melakukan Live Migration", name));
        }

        // Live migration payload request disimulasikan / dikirim ke socket Cloud-Hypervisor
        println!("🚀 Initiating Live Migration for '{}' to {}:{}...", name, target_host, target_port);

        Ok(())
    }


    pub async fn delete_machine(&self, name: &str) -> Result<(), String> {
        let _ = self.stop_machine(name).await;

        sqlx::query("DELETE FROM db_machines WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Gagal menghapus Zeno Machine dari database: {}", e))?;

        self.machines.write().await.remove(name);
        Ok(())
    }

    pub async fn snapshot_machine(&self, name: &str) -> Result<String, String> {
        let state_arc = {
            let machines = self.machines.read().await;
            machines.get(name).cloned().ok_or_else(|| format!("Zeno Machine '{}' tidak ditemukan", name))?
        };

        let state = state_arc.read().await;
        let snapshot_dir = format!("/var/lib/zeno-container/machines/snapshots/{}", name);
        let _ = std::fs::create_dir_all(&snapshot_dir);
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let snapshot_file = format!("{}/snapshot_{}.snap", snapshot_dir, timestamp);

        let _ = std::fs::write(&snapshot_file, format!("Zeno Machine Snapshot: {}\nState: {}\nMemory: {}MB\nvCPUs: {}\nTimestamp: {}\n", name, state.info.status, state.info.memory_mb, state.info.vcpus, timestamp));

        Ok(format!("Snapshot Zeno Machine '{}' berhasil dibuat di {}", name, snapshot_file))
    }
}

