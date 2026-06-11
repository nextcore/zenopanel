fn get_ports_for_pid(pid: u32) -> Vec<u16> {
    let mut ports = Vec::new();
    let mut inodes = std::collections::HashSet::new();
    
    let fd_dir = format!("/proc/{}/fd", pid);
    println!("Reading fd_dir: {}", fd_dir);
    if let Ok(entries) = std::fs::read_dir(fd_dir) {
        for entry in entries.flatten() {
            if let Ok(target) = std::fs::read_link(entry.path()) {
                let target_str = target.to_string_lossy();
                if target_str.starts_with("socket:[") && target_str.ends_with(']') {
                    if let Ok(inode) = target_str[8..target_str.len() - 1].parse::<u64>() {
                        inodes.insert(inode);
                    }
                }
            }
        }
    } else {
        println!("Failed to read fd dir!");
    }
    
    println!("Found inodes: {:?}", inodes);
    if inodes.is_empty() {
        return ports;
    }
    
    for proc_file in &["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(content) = std::fs::read_to_string(proc_file) {
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 10 {
                    if let Ok(inode) = parts[9].parse::<u64>() {
                        if inodes.contains(&inode) {
                            let local_addr = parts[1];
                            if let Some(pos) = local_addr.find(':') {
                                if let Ok(port) = u16::from_str_radix(&local_addr[pos + 1..], 16) {
                                    ports.push(port);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    ports.sort();
    ports.dedup();
    ports
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: ./test_ports <PID>");
        return;
    }
    let pid: u32 = args[1].parse().unwrap();
    let ports = get_ports_for_pid(pid);
    println!("Ports for PID {}: {:?}", pid, ports);
}
