use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::Command;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use sqlx::SqlitePool;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub id: String,
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub auto_restart: bool,
    pub status: String, // "stopped", "running", "starting", "failed"
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
}

pub struct ProcessState {
    pub id: String,
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub auto_restart: bool,
    pub status: String,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub logs: VecDeque<String>,
    pub stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub stop_requested: bool,
}

#[derive(Clone)]
pub struct ProcessManager {
    pool: SqlitePool,
    processes: Arc<RwLock<HashMap<String, Arc<RwLock<ProcessState>>>>>,
}

impl ProcessManager {
    pub async fn new(pool: SqlitePool) -> Self {
        // Create table if it doesn't exist
        let create_table_query = "
            CREATE TABLE IF NOT EXISTS managed_procs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                command TEXT NOT NULL,
                cwd TEXT NOT NULL DEFAULT '.',
                env TEXT NOT NULL DEFAULT '{}',
                auto_restart INTEGER NOT NULL DEFAULT 1
            );
        ";
        if let Err(e) = sqlx::query(create_table_query).execute(&pool).await {
            eprintln!("Failed to create managed_procs table: {}", e);
        }

        Self {
            pool,
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn load_from_db(&self) -> Result<(), String> {
        let rows = sqlx::query("SELECT id, name, command, cwd, env, auto_restart FROM managed_procs")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut to_start = Vec::new();

        {
            let mut procs = self.processes.write().await;
            for row in rows {
                use sqlx::Row;
                let id: String = row.get("id");
                let name: String = row.get("name");
                let command: String = row.get("command");
                let cwd: String = row.get("cwd");
                let env_str: String = row.get("env");
                let auto_restart_int: i32 = row.get("auto_restart");

                let env: HashMap<String, String> = serde_json::from_str(&env_str).unwrap_or_default();
                let auto_restart = auto_restart_int != 0;

                if auto_restart {
                    to_start.push(id.clone());
                }

                procs.insert(
                    id.clone(),
                    Arc::new(RwLock::new(ProcessState {
                        id,
                        name,
                        command,
                        cwd,
                        env,
                        auto_restart,
                        status: "stopped".to_string(),
                        pid: None,
                        exit_code: None,
                        logs: VecDeque::with_capacity(1000),
                        stop_tx: None,
                        stop_requested: false,
                    })),
                );
            }
        }

        // Spawn starting tasks for auto_restart processes
        for id in to_start {
            let this = self.clone();
            tokio::spawn(async move {
                if let Err(e) = this.start_process(&id).await {
                    eprintln!("Failed to auto-start process {}: {}", id, e);
                }
            });
        }

        Ok(())
    }

    pub async fn add_process(
        &self,
        name: String,
        command: String,
        cwd: String,
        env: HashMap<String, String>,
        auto_restart: bool,
    ) -> Result<String, String> {
        // Generate simple ID
        let id = format!("{:x}", rand::random::<u32>());
        let env_str = serde_json::to_string(&env).unwrap_or_else(|_| "{}".to_string());
        let auto_restart_int = if auto_restart { 1 } else { 0 };

        sqlx::query("INSERT INTO managed_procs (id, name, command, cwd, env, auto_restart) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&name)
            .bind(&command)
            .bind(&cwd)
            .bind(&env_str)
            .bind(auto_restart_int)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let state = Arc::new(RwLock::new(ProcessState {
            id: id.clone(),
            name,
            command,
            cwd,
            env,
            auto_restart,
            status: "stopped".to_string(),
            pid: None,
            exit_code: None,
            logs: VecDeque::with_capacity(1000),
            stop_tx: None,
            stop_requested: false,
        }));

        self.processes.write().await.insert(id.clone(), state);
        Ok(id)
    }

    pub async fn update_process(
        &self,
        id: &str,
        name: String,
        command: String,
        cwd: String,
        env: HashMap<String, String>,
        auto_restart: bool,
    ) -> Result<(), String> {
        let env_str = serde_json::to_string(&env).unwrap_or_else(|_| "{}".to_string());
        let auto_restart_int = if auto_restart { 1 } else { 0 };

        sqlx::query("UPDATE managed_procs SET name = ?, command = ?, cwd = ?, env = ?, auto_restart = ? WHERE id = ?")
            .bind(&name)
            .bind(&command)
            .bind(&cwd)
            .bind(&env_str)
            .bind(auto_restart_int)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let state_arc = {
            let procs = self.processes.read().await;
            procs.get(id).cloned().ok_or_else(|| "Process not found".to_string())?
        };

        {
            let mut state = state_arc.write().await;
            state.name = name;
            state.command = command;
            state.cwd = cwd;
            state.env = env;
            state.auto_restart = auto_restart;
        }

        Ok(())
    }

    pub async fn remove_process(&self, id: &str) -> Result<(), String> {
        // Stop if running
        let _ = self.stop_process(id).await;

        sqlx::query("DELETE FROM managed_procs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        self.processes.write().await.remove(id);
        Ok(())
    }

    pub async fn start_process(&self, id: &str) -> Result<(), String> {
        let state_arc = {
            let procs = self.processes.read().await;
            procs.get(id).cloned().ok_or_else(|| "Process not found".to_string())?
        };

        let mut state = state_arc.write().await;
        if state.status == "running" || state.status == "starting" {
            return Err("Process is already running or starting".to_string());
        }

        state.status = "starting".to_string();
        state.stop_requested = false;
        state.exit_code = None;

        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        state.stop_tx = Some(stop_tx);

        let command = state.command.clone();
        let cwd = state.cwd.clone();
        let env = state.env.clone();
        let state_arc_clone = state_arc.clone();

        // Spawn background supervisor task
        tokio::spawn(async move {
            let mut backoff = 0;
            loop {
                // Check if manually stopped
                {
                    let lock = state_arc_clone.read().await;
                    if lock.stop_requested {
                        break;
                    }
                }

                // If backing off, wait
                if backoff > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(backoff)).await;
                }

                // Build Command
                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = Command::new("cmd");
                    c.args(&["/C", &command]);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.args(&["-c", &command]);
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::CommandExt;
                        c.as_std_mut().process_group(0);
                    }
                    c
                };

                cmd.current_dir(&cwd);

                // Clean ZenoPanel-specific environment variables that might pollute child processes
                let zeno_keys = [
                    "DB_DRIVER",
                    "DB_HOST",
                    "DB_USER",
                    "DB_PASS",
                    "DB_NAME",
                    "DB_MAX_OPEN_CONNS",
                    "DB_MAX_IDLE_CONNS",
                    "APP_PORT",
                    "APP_ENV",
                    "JWT_SECRET",
                    "CSRF_TOKEN",
                ];
                for key in &zeno_keys {
                    if !env.contains_key(*key) {
                        cmd.env_remove(key);
                    }
                }

                cmd.envs(&env);
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                let start_time = std::time::Instant::now();
                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        let err_msg = format!("Failed to spawn process: {}", e);
                        {
                            let mut lock = state_arc_clone.write().await;
                            lock.status = "failed".to_string();
                            lock.logs.push_back(err_msg);
                            if lock.logs.len() > 1000 {
                                lock.logs.pop_front();
                            }
                        }
                        backoff = std::cmp::min(backoff + 2, 10); // exponential backoff on spawns
                        let lock = state_arc_clone.read().await;
                        if !lock.auto_restart || lock.stop_requested {
                            break;
                        }
                        continue;
                    }
                };

                let pid = child.id();
                {
                    let mut lock = state_arc_clone.write().await;
                    lock.status = "running".to_string();
                    lock.pid = pid;
                    lock.exit_code = None;
                    lock.logs.push_back(format!("[ZenoPanel] Process started with PID {:?}", pid));
                    if lock.logs.len() > 1000 {
                        lock.logs.pop_front();
                    }
                }

                // Capture stdout & stderr
                let stdout = child.stdout.take().unwrap();
                let stderr = child.stderr.take().unwrap();

                let state_ref_for_stdout = state_arc_clone.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let mut lock = state_ref_for_stdout.write().await;
                        lock.logs.push_back(line);
                        if lock.logs.len() > 1000 {
                            lock.logs.pop_front();
                        }
                    }
                });

                let state_ref_for_stderr = state_arc_clone.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let mut lock = state_ref_for_stderr.write().await;
                        lock.logs.push_back(line);
                        if lock.logs.len() > 1000 {
                            lock.logs.pop_front();
                        }
                    }
                });

                // Wait for exit or stop request
                tokio::select! {
                    status_res = child.wait() => {
                        let elapsed = start_time.elapsed().as_secs();
                        let exit_code = match status_res {
                            Ok(st) => st.code(),
                            Err(_) => None,
                        };

                        let mut lock = state_arc_clone.write().await;
                        lock.status = if exit_code == Some(0) { "stopped".to_string() } else { "failed".to_string() };
                        lock.pid = None;
                        lock.exit_code = exit_code;
                        lock.logs.push_back(format!("[ZenoPanel] Process exited with code {:?}", exit_code));
                        if lock.logs.len() > 1000 {
                            lock.logs.pop_front();
                        }

                        // Determine if we should auto-restart
                        if !lock.auto_restart || lock.stop_requested {
                            break;
                        }

                        // If it exited too quickly (less than 3 seconds), increase backoff
                        if elapsed < 3 {
                            backoff = std::cmp::min(backoff + 2, 10);
                        } else {
                            backoff = 1;
                        }
                    }
                    _ = &mut stop_rx => {
                        // Kill the process group / process
                        let mut lock = state_arc_clone.write().await;
                        let pid = lock.pid;
                        lock.status = "stopped".to_string();
                        lock.pid = None;
                        lock.exit_code = None;
                        lock.logs.push_back("[ZenoPanel] Process stopped by user".to_string());
                        if lock.logs.len() > 1000 {
                            lock.logs.pop_front();
                        }

                        #[cfg(unix)]
                        {
                            if let Some(p) = pid {
                                // Kill process group
                                let _ = std::process::Command::new("kill")
                                    .args(&["-9", &format!("-{}", p)])
                                    .status();
                            }
                        }
                        #[cfg(not(unix))]
                        let _ = child.kill().await;

                        break;
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn stop_process(&self, id: &str) -> Result<(), String> {
        let state_arc = {
            let procs = self.processes.read().await;
            procs.get(id).cloned().ok_or_else(|| "Process not found".to_string())?
        };

        let mut state = state_arc.write().await;
        state.stop_requested = true;
        if let Some(tx) = state.stop_tx.take() {
            let _ = tx.send(());
        }

        // Just in case it was in starting or crashed status
        if state.status == "running" || state.status == "starting" {
            state.status = "stopped".to_string();
            state.pid = None;
        }

        Ok(())
    }

    pub async fn restart_process(&self, id: &str) -> Result<(), String> {
        let _ = self.stop_process(id).await;
        // Wait briefly for stop to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start_process(id).await
    }

    pub async fn get_logs(&self, id: &str, lines_count: usize) -> Result<Vec<String>, String> {
        let procs = self.processes.read().await;
        let state_arc = procs.get(id).ok_or_else(|| "Process not found".to_string())?;
        let state = state_arc.read().await;

        let total_logs = state.logs.len();
        let skip = if total_logs > lines_count { total_logs - lines_count } else { 0 };

        Ok(state.logs.iter().skip(skip).cloned().collect())
    }

    pub async fn list_processes(&self) -> Vec<ProcessInfo> {
        let procs = self.processes.read().await;
        let mut list = Vec::new();
        for state_arc in procs.values() {
            let state = state_arc.read().await;
            list.push(ProcessInfo {
                id: state.id.clone(),
                name: state.name.clone(),
                command: state.command.clone(),
                cwd: state.cwd.clone(),
                env: state.env.clone(),
                auto_restart: state.auto_restart,
                status: state.status.clone(),
                pid: state.pid,
                exit_code: state.exit_code,
            });
        }
        list
    }
}
