use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;
use std::fs;
use std::process::Command;
use zenocore::{Engine, SlotMeta, Value};
use crate::slots::resolve_node_value;

use super::common::{
    get_data_dir, run_cmd_status_silent, run_privileged_status,
    get_networks, save_networks, rootfs_dir, parse_port_rule,
    NetworkConfig
};
use super::container::container_list_internal;

pub fn register(engine: &mut Engine) {
    register_network_list(engine);
    register_network_create(engine);
    register_network_delete(engine);
}

pub(crate) fn setup_bridge() -> Result<(), String> {
    let _ = std::fs::write("/proc/sys/net/ipv4/ip_forward", "1");

    let bridge_exists = Command::new("ip").args(&["link", "show", "zenobr0"]).output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !bridge_exists {
        let _ = run_cmd_status_silent("ip", &["link", "add", "name", "zenobr0", "type", "bridge"]);
        let _ = run_cmd_status_silent("ip", &["addr", "add", "172.20.0.1/16", "dev", "zenobr0"]);
        let _ = run_cmd_status_silent("ip", &["link", "set", "zenobr0", "up"]);
    }

    let _ = std::fs::write("/proc/sys/net/ipv4/conf/zenobr0/route_localnet", "1");

    let masq_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !masq_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE"]);
    }

    let fwd_in_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-i", "zenobr0", "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_in_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-i", "zenobr0", "-j", "ACCEPT"]);
    }

    let fwd_out_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-o", "zenobr0", "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_out_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-o", "zenobr0", "-j", "ACCEPT"]);
    }

    let local_masq_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-o", "zenobr0", "-s", "127.0.0.1", "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !local_masq_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-o", "zenobr0", "-s", "127.0.0.1", "-j", "MASQUERADE"]);
    }

    let chk_exists = run_cmd_status_silent("iptables", &["-t", "mangle", "-C", "POSTROUTING", "-o", "zenobr0", "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !chk_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "mangle", "-A", "POSTROUTING", "-o", "zenobr0", "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"]);
    }

    Ok(())
}

fn find_available_ip(data_dir: &str, subnet: &str, gateway: &str) -> Result<String, String> {
    let parts: Vec<&str> = subnet.split('.').collect();
    if parts.len() < 2 {
        return Err(format!("Invalid subnet: {}", subnet));
    }
    let x: i32 = parts[1].parse().map_err(|e| format!("Invalid subnet number: {}", e))?;

    let mut taken_ips = std::collections::HashSet::new();
    taken_ips.insert(gateway.to_string());

    if let Ok(containers) = container_list_internal(data_dir, false) {
        for c in containers {
            if c.status == "running" {
                if let Some(env) = c.env {
                    if let Some(ip) = env.get("ZENO_IP") {
                        taken_ips.insert(ip.clone());
                    }
                }
            }
        }
    }

    for i in 2..255 {
        let ip = format!("172.{}.0.{}", x, i);
        if !taken_ips.contains(&ip) {
            return Ok(ip);
        }
    }

    Err("No available IP addresses".to_string())
}

unsafe extern "C" {
    fn ioctl(
        fd: std::os::raw::c_int,
        request: std::os::raw::c_ulong,
        ...
    ) -> std::os::raw::c_int;
}

fn disable_checksum_offloading(iface: &str) -> Result<(), String> {
    use std::os::unix::io::AsRawFd;

    #[repr(C)]
    struct EthtoolValue {
        cmd: u32,
        data: u32,
    }

    #[repr(C)]
    struct IfreqEthtool {
        ifr_name: [u8; 16],
        ifr_data: *mut EthtoolValue,
    }

    const SIOCETHTOOL: std::os::raw::c_ulong = 0x8946;
    const ETHTOOL_SRXCSUM: u32 = 0x00000015;
    const ETHTOOL_STXCSUM: u32 = 0x00000017;

    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to create socket: {}", e))?;
    let fd = socket.as_raw_fd();

    let mut ifr_name = [0u8; 16];
    let bytes = iface.as_bytes();
    if bytes.len() >= 16 {
        return Err("Interface name too long".to_string());
    }
    ifr_name[..bytes.len()].copy_from_slice(bytes);

    let mut rx_val = EthtoolValue {
        cmd: ETHTOOL_SRXCSUM,
        data: 0,
    };
    let mut ifr_rx = IfreqEthtool {
        ifr_name,
        ifr_data: &mut rx_val,
    };
    unsafe {
        let _ = ioctl(fd, SIOCETHTOOL, &mut ifr_rx);
    }

    let mut tx_val = EthtoolValue {
        cmd: ETHTOOL_STXCSUM,
        data: 0,
    };
    let mut ifr_tx = IfreqEthtool {
        ifr_name,
        ifr_data: &mut tx_val,
    };
    unsafe {
        let _ = ioctl(fd, SIOCETHTOOL, &mut ifr_tx);
    }

    Ok(())
}

pub(crate) fn configure_container_network(
    data_dir: &str,
    container_id: &str,
    pid: i32,
    ports: Vec<String>,
    network_name: &str,
) -> Result<String, String> {
    let mut bridge_id = "zenobr0".to_string();
    let mut subnet_str = "172.20.0.0/16".to_string();
    let mut gateway_ip = "172.20.0.1".to_string();

    if !network_name.is_empty() && network_name != "bridge" && network_name != "default" {
        let networks = get_networks(data_dir);
        for n in networks {
            if n.name == network_name || n.id == network_name {
                bridge_id = n.id;
                subnet_str = n.subnet;
                gateway_ip = n.gateway;
                break;
            }
        }
    }

    if bridge_id == "zenobr0" {
        setup_bridge()?;
    } else {
        let output = Command::new("ip").args(&["link", "show", &bridge_id]).output();
        if output.is_err() || !output.unwrap().status.success() {
            return Err(format!("Custom bridge interface {} does not exist", bridge_id));
        }
    }

    let ip = find_available_ip(data_dir, &subnet_str, &gateway_ip)?;

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    container_id.hash(&mut hasher);
    let hash_val = hasher.finish();
    let hash_str = format!("{:08x}", hash_val);
    let short_hash = if hash_str.len() > 8 { &hash_str[0..8] } else { &hash_str };

    let veth_host = format!("veth-h-{}", short_hash);
    let veth_guest = format!("veth-g-{}", short_hash);

    let _ = run_cmd_status_silent("ip", &["link", "delete", &veth_host]);

    let status = run_cmd_status_silent("ip", &["link", "add", &veth_host, "type", "veth", "peer", "name", &veth_guest])
        .map_err(|e| format!("Failed to create veth pair: {}", e))?;
    if !status.success() {
        return Err("Failed to create veth pair".to_string());
    }

    let _ = disable_checksum_offloading(&veth_host);
    let _ = disable_checksum_offloading(&veth_guest);

    let status = run_cmd_status_silent("ip", &["link", "set", &veth_host, "master", &bridge_id])
        .map_err(|e| format!("Failed to bind host interface to bridge: {}", e))?;
    if !status.success() {
        return Err(format!("Failed to bind host interface {} to bridge {}", veth_host, bridge_id));
    }

    let status = run_cmd_status_silent("ip", &["link", "set", &veth_host, "up"])
        .map_err(|e| format!("Failed to bring up host interface: {}", e))?;
    if !status.success() {
        return Err(format!("Failed to bring up host interface {}", veth_host));
    }

    let pid_str = pid.to_string();
    let status = run_cmd_status_silent("ip", &["link", "set", &veth_guest, "netns", &pid_str])
        .map_err(|e| format!("Failed to move guest veth: {}", e))?;
    if !status.success() {
        return Err("Failed to move guest interface to container netns".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "link", "set", &veth_guest, "name", "eth0"])
        .map_err(|e| format!("Failed to rename veth inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to rename guest interface to eth0 inside container".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "addr", "add", &format!("{}/16", ip), "dev", "eth0"])
        .map_err(|e| format!("Failed to configure IP inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to assign IP address to eth0 inside container".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "link", "set", "eth0", "up"])
        .map_err(|e| format!("Failed to bring up link inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to bring up eth0 inside container".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "route", "add", "default", "via", &gateway_ip])
        .map_err(|e| format!("Failed to add gateway route inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to configure default gateway route inside container".to_string());
    }

    let resolv_path = rootfs_dir(data_dir, container_id).join("etc/resolv.conf");
    let _ = fs::write(resolv_path, "nameserver 8.8.8.8\nnameserver 1.1.1.1\n");

    for p in ports {
        if let Some(rule) = parse_port_rule(&p) {
            let host_port_formatted = rule.host_port.replace('-', ":");
            let dest_str = format!("{}:{}", ip, rule.container_port);
            let mut preroute_args = vec!["-t", "nat", "-A", "PREROUTING", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                preroute_args.push("-d");
                preroute_args.push(hip);
            }
            preroute_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &preroute_args);

            let mut output_args = vec!["-t", "nat", "-A", "OUTPUT", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                output_args.push("-d");
                output_args.push(hip);
            }
            output_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &output_args);
        }
    }

    Ok(ip)
}

pub(crate) fn clean_container_network(container_id: &str, ip: &str, ports: &[String]) {
    for p in ports {
        if let Some(rule) = parse_port_rule(&p) {
            let host_port_formatted = rule.host_port.replace('-', ":");
            let dest_str = format!("{}:{}", ip, rule.container_port);
            let mut preroute_args = vec!["-t", "nat", "-D", "PREROUTING", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                preroute_args.push("-d");
                preroute_args.push(hip);
            }
            preroute_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &preroute_args);

            let mut output_args = vec!["-t", "nat", "-D", "OUTPUT", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                output_args.push("-d");
                output_args.push(hip);
            }
            output_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &output_args);
        }
    }
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    container_id.hash(&mut hasher);
    let hash_val = hasher.finish();
    let hash_str = format!("{:08x}", hash_val);
    let short_hash = if hash_str.len() > 8 { &hash_str[0..8] } else { &hash_str };

    let veth_host = format!("veth-h-{}", short_hash);
    let _ = run_cmd_status_silent("ip", &["link", "delete", &veth_host]);
}

pub(crate) fn sync_hosts_entries(data_dir: &str) -> Result<(), String> {
    let containers = container_list_internal(data_dir, false)?;
    
    let mut running_ips = HashMap::new();
    let mut running_nets = HashMap::new();
    for c in &containers {
        if c.status == "running" {
            if let Some(ref env) = c.env {
                if let Some(ip) = env.get("ZENO_IP") {
                    running_ips.insert(c.id.clone(), ip.clone());
                    if let Some(ref net) = c.network {
                        running_nets.insert(c.id.clone(), net.clone());
                    }
                }
            }
        }
    }

    for c in &containers {
        if c.status != "running" {
            continue;
        }

        let hosts_path = rootfs_dir(data_dir, &c.id).join("etc/hosts");
        let mut sb = String::new();
        sb.push_str("127.0.0.1\tlocalhost\n");
        sb.push_str("::1\tlocalhost ip6-localhost ip6-loopback\n\n");
        sb.push_str("# Zeno Container Service Discovery\n");

        if let Some(my_ip) = running_ips.get(&c.id) {
            sb.push_str(&format!("{}\t{}\n", my_ip, c.id));
        }

        let my_net = c.network.as_ref().map(|s| s.as_str()).unwrap_or("");
        for (other_id, other_ip) in &running_ips {
            if other_id != &c.id {
                let other_net = running_nets.get(other_id).map(|s| s.as_str()).unwrap_or("");
                if other_net == my_net {
                    sb.push_str(&format!("{}\t{}\n", other_ip, other_id));
                }
            }
        }

        let _ = fs::write(hosts_path, sb);
    }

    Ok(())
}

fn create_bridge_network(data_dir: &str, name: &str) -> Result<String, String> {
    let mut networks = get_networks(data_dir);
    for n in &networks {
        if n.name == name {
            return Err(format!("Network {} already exists", name));
        }
    }

    if name == "bridge" || name == "default" {
        return Err(format!("Network name {} is reserved", name));
    }

    let mut used_subnets = std::collections::HashSet::new();
    for n in &networks {
        let parts: Vec<&str> = n.subnet.split('.').collect();
        if parts.len() > 1 {
            if let Ok(x) = parts[1].parse::<i32>() {
                used_subnets.insert(x);
            }
        }
    }

    let mut selected_x = -1;
    for x in 21..=31 {
        if !used_subnets.contains(&x) {
            selected_x = x;
            break;
        }
    }

    if selected_x == -1 {
        return Err("No subnets available in 172.21.0.0/16 - 172.31.0.0/16".to_string());
    }

    let bridge_id = format!("zenobr{}", selected_x);
    let subnet = format!("172.{}.0.0/16", selected_x);
    let gateway = format!("172.{}.0.1", selected_x);

    let _ = run_cmd_status_silent("ip", &["link", "add", "name", &bridge_id, "type", "bridge"]);
    let _ = run_cmd_status_silent("ip", &["addr", "add", &format!("{}/16", gateway), "dev", &bridge_id]);
    let _ = run_cmd_status_silent("ip", &["link", "set", &bridge_id, "up"]);

    let route_localnet_path = format!("/proc/sys/net/ipv4/conf/{}/route_localnet", bridge_id);
    let _ = std::fs::write(&route_localnet_path, "1");

    let rule_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-s", &subnet, "!", "-o", &bridge_id, "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);

    if !rule_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-s", &subnet, "!", "-o", &bridge_id, "-j", "MASQUERADE"]);
    }

    let fwd_in_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-i", &bridge_id, "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_in_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-i", &bridge_id, "-j", "ACCEPT"]);
    }

    let fwd_out_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-o", &bridge_id, "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_out_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-o", &bridge_id, "-j", "ACCEPT"]);
    }

    let local_masq_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-o", &bridge_id, "-s", "127.0.0.1", "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !local_masq_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-o", &bridge_id, "-s", "127.0.0.1", "-j", "MASQUERADE"]);
    }

    let chk_exists = run_cmd_status_silent("iptables", &["-t", "mangle", "-C", "POSTROUTING", "-o", &bridge_id, "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !chk_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "mangle", "-A", "POSTROUTING", "-o", &bridge_id, "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"]);
    }

    let new_net = NetworkConfig {
        id: bridge_id.clone(),
        name: name.to_string(),
        driver: "bridge".to_string(),
        subnet,
        gateway,
    };
    networks.push(new_net);
    save_networks(data_dir, &networks)?;

    Ok(bridge_id)
}

fn delete_bridge_network(data_dir: &str, name: &str) -> Result<(), String> {
    let mut networks = get_networks(data_dir);
    let mut found_idx = None;
    for (i, n) in networks.iter().enumerate() {
        if n.name == name || n.id == name {
            found_idx = Some(i);
            break;
        }
    }

    let idx = found_idx.ok_or_else(|| format!("Network {} not found", name))?;
    let net = &networks[idx];

    let containers = container_list_internal(data_dir, false)?;
    for c in containers {
        if c.network.as_ref().map(|s| s == name || s == &net.id).unwrap_or(false) && c.status == "running" {
            return Err(format!("Network is in use by running container {}", c.id));
        }
    }

    let _ = run_cmd_status_silent("ip", &["link", "set", &net.id, "down"]);
    let _ = run_cmd_status_silent("ip", &["link", "delete", &net.id]);
    let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-D", "POSTROUTING", "-s", &net.subnet, "!", "-o", &net.id, "-j", "MASQUERADE"]);
    let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-D", "POSTROUTING", "-o", &net.id, "-s", "127.0.0.1", "-j", "MASQUERADE"]);
    let _ = run_cmd_status_silent("iptables", &["-t", "mangle", "-D", "POSTROUTING", "-o", &net.id, "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"]);
    let _ = run_cmd_status_silent("iptables", &["-D", "FORWARD", "-i", &net.id, "-j", "ACCEPT"]);
    let _ = run_cmd_status_silent("iptables", &["-D", "FORWARD", "-o", &net.id, "-j", "ACCEPT"]);

    networks.remove(idx);
    save_networks(data_dir, &networks)?;

    Ok(())
}

fn register_network_list(engine: &mut Engine) {
    engine.register(
        "box.network_list",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "networks".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let mut list = Vec::new();

            let mut default_net = HashMap::new();
            default_net.insert("Name".to_string(), Value::String("bridge".to_string()));
            default_net.insert("Id".to_string(), Value::String("zenobr0".to_string()));
            default_net.insert("Driver".to_string(), Value::String("bridge".to_string()));
            default_net.insert("Subnet".to_string(), Value::String("172.20.0.0/16".to_string()));
            default_net.insert("Gateway".to_string(), Value::String("172.20.0.1".to_string()));
            list.push(Value::Map(default_net));

            let custom_nets = get_networks(&data_dir);
            for n in custom_nets {
                let mut m = HashMap::new();
                m.insert("Name".to_string(), Value::String(n.name));
                m.insert("Id".to_string(), Value::String(n.id));
                m.insert("Driver".to_string(), Value::String(n.driver));
                m.insert("Subnet".to_string(), Value::String(n.subnet));
                m.insert("Gateway".to_string(), Value::String(n.gateway));
                list.push(Value::Map(m));
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List bridge networks".to_string(),
            example: "box.network_list { as: $networks }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_network_create(engine: &mut Engine) {
    engine.register(
        "box.network_create",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut target = "network_create_result".to_string();

            for child in &node.children {
                if child.name == "name" {
                    name = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let res = create_bridge_network(&data_dir, &name);

            let mut result = HashMap::new();
            match res {
                Ok(id) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("id".to_string(), Value::String(id));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Create bridge network".to_string(),
            example: "box.network_create { name: 'my-net', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_network_delete(engine: &mut Engine) {
    engine.register(
        "box.network_delete",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut target = "network_delete_result".to_string();

            for child in &node.children {
                if child.name == "name" {
                    name = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let res = delete_bridge_network(&data_dir, &name);

            let mut result = HashMap::new();
            match res {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Delete bridge network".to_string(),
            example: "box.network_delete { name: 'my-net', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}
