use zenocore::{Engine, SlotMeta, Value};
use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

unsafe extern "C" {
    fn getuid() -> u32;
}

fn get_distro() -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let name = line.trim_start_matches("PRETTY_NAME=").trim_matches('"');
                return name.to_string();
            } else if line.starts_with("NAME=") {
                let name = line.trim_start_matches("NAME=").trim_matches('"');
                return name.to_string();
            }
        }
    }
    "Linux".to_string()
}

fn get_init_system() -> String {
    if Path::new("/run/systemd/system").exists() {
        "systemd".to_string()
    } else if Path::new("/run/openrc").exists() || Path::new("/sbin/openrc-run").exists() {
        "openrc".to_string()
    } else if Path::new("/etc/init.d").exists() {
        "sysvinit".to_string()
    } else {
        "unknown".to_string()
    }
}

fn get_service_status(init_sys: &str) -> String {
    if init_sys == "systemd" {
        let path = Path::new("/etc/systemd/system/zenopanel.service");
        if !path.exists() {
            return "not_installed".to_string();
        }
        let output = Command::new("systemctl")
            .args(&["is-active", "zenopanel.service"])
            .output();
        if let Ok(out) = output {
            let status_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if status_str == "active" {
                return "active".to_string();
            }
        }
        "inactive".to_string()
    } else {
        let path = Path::new("/etc/init.d/zenopanel");
        if !path.exists() {
            return "not_installed".to_string();
        }
        let output = Command::new("/etc/init.d/zenopanel")
            .arg("status")
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                return "active".to_string();
            }
        }
        "inactive".to_string()
    }
}

fn generate_service_content(init_sys: &str) -> String {
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/usr/local/bin/zeno".to_string());
    let working_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/home/max/Documents/PROJ/github/zenopanel".to_string());

    if init_sys == "systemd" {
        let app_user = std::env::var("APP_USER").unwrap_or_else(|_| "root".to_string());
        let cap_lines = if app_user != "root" {
            "AmbientCapabilities=CAP_NET_BIND_SERVICE\nCapabilityBoundingSet=CAP_NET_BIND_SERVICE\n"
        } else {
            ""
        };
        format!(
            "[Unit]\n\
             Description=ZenoPanel Control Panel Service\n\
             After=network.target\n\n\
             [Service]\n\
             Type=simple\n\
             User={}\n\
             {}WorkingDirectory={}\n\
             ExecStart={}\n\
             Restart=always\n\
             RestartSec=5\n\
             Environment=PATH=/usr/bin:/usr/local/bin\n\n\
             [Install]\n\
             WantedBy=multi-user.target\n",
            app_user, cap_lines, working_dir, exe_path
        )
    } else {
        format!(
            "#!/bin/sh\n\
             ### BEGIN INIT INFO\n\
             # Provides:          zenopanel\n\
             # Required-Start:    $local_fs $network\n\
             # Required-Stop:     $local_fs $network\n\
             # Default-Start:     2 3 4 5\n\
             # Default-Stop:      0 1 6\n\
             # Short-Description: ZenoPanel Service\n\
             ### END INIT INFO\n\n\
             DIR=\"{}\"\n\
             DAEMON=\"{}\"\n\
             DAEMON_NAME=\"zenopanel\"\n\n\
             case \"$1\" in\n\
                 start)\n\
                     echo \"Starting $DAEMON_NAME...\"\n\
                     cd \"$DIR\" && \"$DAEMON\" > /var/log/zenopanel.log 2>&1 &\n\
                     ;;\n\
                 stop)\n\
                     echo \"Stopping $DAEMON_NAME...\"\n\
                     pkill -f \"$DAEMON\"\n\
                     ;;\n\
                 restart)\n\
                     $0 stop\n\
                     sleep 1\n\
                     $0 start\n\
                     ;;\n\
                 status)\n\
                     if pgrep -f \"$DAEMON\" > /dev/null; then\n\
                         echo \"$DAEMON_NAME is running.\"\n\
                         exit 0\n\
                     else\n\
                         echo \"$DAEMON_NAME is not running.\"\n\
                         exit 3\n\
                     fi\n\
                     ;;\n\
                 *)\n\
                     echo \"Usage: $0 {{start|stop|restart|status}}\"\n\
                     exit 1\n\
                     ;;\n\
             esac\n\
             exit 0\n",
            working_dir, exe_path
        )
    }
}

pub fn register(engine: &mut Engine) {
    engine.register(
        "system.service_info",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "service_info".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let distro = get_distro();
            let init_sys = get_init_system();
            let is_root = unsafe { getuid() } == 0;
            let status = get_service_status(&init_sys);
            let service_content = generate_service_content(&init_sys);

            let install_cmd = if init_sys == "systemd" {
                "sudo systemctl daemon-reload && sudo systemctl enable zenopanel && sudo systemctl start zenopanel"
            } else {
                "sudo chmod +x /etc/init.d/zenopanel && sudo update-rc.d zenopanel defaults && sudo service zenopanel start"
            };

            let uninstall_cmd = if init_sys == "systemd" {
                "sudo systemctl stop zenopanel && sudo systemctl disable zenopanel && sudo rm /etc/systemd/system/zenopanel.service && sudo systemctl daemon-reload"
            } else {
                "sudo service zenopanel stop && sudo update-rc.d -f zenopanel remove && sudo rm /etc/init.d/zenopanel"
            };

            let mut info = HashMap::new();
            info.insert("distro".to_string(), Value::String(distro));
            info.insert("init_system".to_string(), Value::String(init_sys));
            info.insert("is_root".to_string(), Value::Bool(is_root));
            info.insert("status".to_string(), Value::String(status));
            info.insert("service_content".to_string(), Value::String(service_content));
            info.insert("install_command".to_string(), Value::String(install_cmd.to_string()));
            info.insert("uninstall_command".to_string(), Value::String(uninstall_cmd.to_string()));

            scope.set(&target, Value::Map(info));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.service_install",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "success".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let init_sys = get_init_system();
            let is_root = unsafe { getuid() } == 0;

            if !is_root {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            // Copy zeno-container if found in local candidate paths
            let candidates = [
                "modul/zeno-container/zeno-container",
                "modul/zeno-container",
                "zeno-container",
            ];
            for path in &candidates {
                if Path::new(path).exists() {
                    let dest = "/usr/local/bin/zeno-container";
                    if std::fs::copy(path, dest).is_ok() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755));
                        }
                        let _ = std::fs::create_dir_all("/var/lib/zeno-container");
                        break;
                    }
                }
            }

            let service_content = generate_service_content(&init_sys);
            let success = if init_sys == "systemd" {
                let path = "/etc/systemd/system/zenopanel.service";
                if std::fs::write(path, &service_content).is_ok() {
                    let reload = Command::new("systemctl").arg("daemon-reload").status();
                    let enable = Command::new("systemctl").args(&["enable", "zenopanel.service"]).status();
                    let start = Command::new("systemctl").args(&["start", "zenopanel.service"]).status();
                    reload.is_ok() && enable.is_ok() && start.is_ok()
                } else {
                    false
                }
            } else if init_sys == "openrc" || init_sys == "sysvinit" {
                let path = "/etc/init.d/zenopanel";
                if std::fs::write(path, &service_content).is_ok() {
                    let chmod = Command::new("chmod").args(&["+x", path]).status();
                    let update = Command::new("update-rc.d").args(&["zenopanel", "defaults"]).status();
                    let start = Command::new("service").args(&["zenopanel", "start"]).status();
                    chmod.is_ok() && update.is_ok() && start.is_ok()
                } else {
                    false
                }
            } else {
                false
            };

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.service_uninstall",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "success".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let init_sys = get_init_system();
            let is_root = unsafe { getuid() } == 0;

            if !is_root {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let success = if init_sys == "systemd" {
                let stop = Command::new("systemctl").args(&["stop", "zenopanel.service"]).status();
                let disable = Command::new("systemctl").args(&["disable", "zenopanel.service"]).status();
                let rm = std::fs::remove_file("/etc/systemd/system/zenopanel.service");
                let reload = Command::new("systemctl").arg("daemon-reload").status();
                stop.is_ok() && disable.is_ok() && rm.is_ok() && reload.is_ok()
            } else if init_sys == "openrc" || init_sys == "sysvinit" {
                let stop = Command::new("service").args(&["zenopanel", "stop"]).status();
                let remove = Command::new("update-rc.d").args(&["-f", "zenopanel", "remove"]).status();
                let rm = std::fs::remove_file("/etc/init.d/zenopanel");
                stop.is_ok() && remove.is_ok() && rm.is_ok()
            } else {
                false
            };

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}
