use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use sqlx::SqlitePool;

pub struct DnsServer {
    pool: SqlitePool,
    bind_addr: String,
    upstream_dns: String,
}

impl DnsServer {
    pub fn new(pool: SqlitePool, bind_addr: String, upstream_dns: String) -> Self {
        Self {
            pool,
            bind_addr,
            upstream_dns,
        }
    }

    pub async fn start(self) {
        let socket = match UdpSocket::bind(&self.bind_addr).await {
            Ok(s) => {
                println!("[DNS Server] Listening on UDP {}", self.bind_addr);
                Arc::new(s)
            }
            Err(e) => {
                eprintln!("[DNS Server] Failed to bind UDP {}: {}", self.bind_addr, e);
                return;
            }
        };

        let pool = self.pool;
        let upstream = self.upstream_dns;

        loop {
            let mut buf = [0u8; 1024];
            let socket = socket.clone();
            let pool = pool.clone();
            let upstream = upstream.clone();

            match socket.recv_from(&mut buf).await {
                Ok((len, src)) => {
                    tokio::spawn(async move {
                        if let Err(e) = handle_dns_packet(&socket, &buf[..len], src, &pool, &upstream).await {
                            eprintln!("[DNS Server] Error handling packet: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[DNS Server] Error receiving UDP packet: {}", e);
                    break;
                }
            }
        }
    }
}

async fn handle_dns_packet(
    socket: &UdpSocket,
    data: &[u8],
    src: SocketAddr,
    pool: &SqlitePool,
    upstream: &str,
) -> Result<(), String> {
    if data.len() < 12 {
        return Err("Packet too short".to_string());
    }

    // Parse DNS Header
    let id_high = data[0];
    let id_low = data[1];
    let qdcount = ((data[4] as u16) << 8) | (data[5] as u16);

    if qdcount == 0 {
        return Err("No questions in DNS packet".to_string());
    }

    // Extract query name
    let mut pos = 12;
    let mut name = String::new();
    loop {
        if pos >= data.len() {
            return Err("Malformed QNAME".to_string());
        }
        let len = data[pos] as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        if pos + 1 + len > data.len() {
            return Err("Malformed QNAME label".to_string());
        }
        if !name.is_empty() {
            name.push('.');
        }
        let label = std::str::from_utf8(&data[pos + 1..pos + 1 + len])
            .map_err(|e| format!("Invalid UTF-8 in label: {}", e))?;
        name.push_str(label);
        pos += 1 + len;
    }

    // Read QTYPE and QCLASS
    if pos + 4 > data.len() {
        return Err("Malformed QTYPE/QCLASS".to_string());
    }
    let qtype = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
    let _qclass = ((data[pos + 2] as u16) << 8) | (data[pos + 3] as u16);

    let name_lower = name.to_lowercase();
    if name_lower.ends_with(".zeno") && qtype == 1 { // A record query
        let clean_vm_name = name_lower.trim_end_matches(".zeno");
        
        // Search VM IP in SQLite
        let row_res: Result<Option<(String,)>, _> = sqlx::query_as("SELECT ip_address FROM db_machines WHERE name = ?")
            .bind(clean_vm_name)
            .fetch_optional(pool)
            .await;

        if let Ok(Some((ip_str,))) = row_res {
            // Strip subnet mask if present (e.g. "192.168.100.10/24" -> "192.168.100.10")
            let clean_ip = ip_str.split('/').next().unwrap_or("").trim();
            if let Ok(ip) = clean_ip.parse::<std::net::Ipv4Addr>() {
                let octets = ip.octets();
                
                // Build DNS Answer
                let mut response = Vec::new();
                // 1. Transaction ID
                response.push(id_high);
                response.push(id_low);
                // 2. Flags (Standard query response, No error, Authoritative)
                response.push(0x84);
                response.push(0x00);
                // 3. QDCOUNT (1)
                response.push(0x00);
                response.push(0x01);
                // 4. ANCOUNT (1)
                response.push(0x00);
                response.push(0x01);
                // 5. NSCOUNT (0)
                response.push(0x00);
                response.push(0x00);
                // 6. ARCOUNT (0)
                response.push(0x00);
                response.push(0x00);

                // Copy original question section
                response.extend_from_slice(&data[12..pos + 4]);

                // Answer Section
                // Name pointer to QNAME (offset 12)
                response.push(0xc0);
                response.push(0x0c);
                // Type (A: 0x0001)
                response.push(0x00);
                response.push(0x01);
                // Class (IN: 0x0001)
                response.push(0x00);
                response.push(0x01);
                // TTL (60 seconds)
                response.push(0x00);
                response.push(0x00);
                response.push(0x00);
                response.push(0x3c);
                // RDLENGTH (4)
                response.push(0x00);
                response.push(0x04);
                // RDATA (IPv4 address octets)
                response.extend_from_slice(&octets);

                let _ = socket.send_to(&response, src).await;
                return Ok(());
            }
        }

        // If not found in DB, return NXDOMAIN
        let mut response = Vec::new();
        response.push(id_high);
        response.push(id_low);
        response.push(0x81);
        response.push(0x03); // NXDOMAIN
        response.push(0x00);
        response.push(0x01);
        response.push(0x00);
        response.push(0x00);
        response.push(0x00);
        response.push(0x00);
        response.push(0x00);
        response.push(0x00);
        response.extend_from_slice(&data[12..pos + 4]);

        let _ = socket.send_to(&response, src).await;
        return Ok(());
    }

    // Forward to Upstream DNS
    let upstream_socket = UdpSocket::bind("0.0.0.0:0").await
        .map_err(|e| format!("Failed to bind upstream socket: {}", e))?;
    
    upstream_socket.connect(upstream).await
        .map_err(|e| format!("Failed to connect to upstream DNS {}: {}", upstream, e))?;

    upstream_socket.send(data).await
        .map_err(|e| format!("Failed to send to upstream: {}", e))?;

    let mut upstream_buf = [0u8; 1024];
    let upstream_len = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        upstream_socket.recv(&mut upstream_buf)
    ).await
    .map_err(|_| "Upstream DNS timeout".to_string())?
    .map_err(|e| format!("Failed to receive from upstream: {}", e))?;

    let _ = socket.send_to(&upstream_buf[..upstream_len], src).await;
    Ok(())
}
