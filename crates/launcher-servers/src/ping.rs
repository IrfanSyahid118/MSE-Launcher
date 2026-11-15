use chrono::Utc;
use launcher_types::ServerPingStatus;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

pub const MAX_PING_PACKET_LEN: i32 = 256 * 1024;
pub const MAX_PING_JSON_LEN: i32 = 256 * 1024;

pub fn parse_host_port(address: &str) -> (String, u16) {
    let (host, port, _) = parse_host_port_with_explicit(address);
    (host, port)
}

fn parse_host_port_with_explicit(address: &str) -> (String, u16, bool) {
    let trimmed = address.trim();
    if trimmed.starts_with('[') {
        if let Some(close_idx) = trimmed.find(']') {
            let host = &trimmed[1..close_idx];
            let rest = &trimmed[close_idx + 1..];
            if let Some(port_str) = rest.strip_prefix(':') {
                if let Ok(port) = port_str.parse::<u16>() {
                    return (host.to_string(), port, true);
                }
            }
            return (host.to_string(), 25565, false);
        }
    }

    // Do not split bare IPv6 addresses (which contain multiple colons) on ':'.
    if trimmed.parse::<std::net::IpAddr>().is_ok() {
        return (trimmed.to_string(), 25565, false);
    }

    if let Some((host, port_str)) = trimmed.rsplit_once(':') {
        if !host.contains(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                return (host.to_string(), port, true);
            }
        }
    }

    (trimmed.to_string(), 25565, false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SrvCandidate {
    priority: u16,
    weight: u16,
    port: u16,
    target: String,
}

fn select_srv_target(mut records: Vec<SrvCandidate>) -> Option<(String, u16)> {
    records.retain(|r| !r.target.trim().is_empty() && r.target != ".");
    records.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.weight.cmp(&a.weight))
    });
    records
        .into_iter()
        .next()
        .map(|r| (r.target.trim_end_matches('.').to_string(), r.port))
}

#[cfg(windows)]
async fn resolve_minecraft_srv(host: &str) -> Option<(String, u16)> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::NetworkManagement::Dns::{
        DnsFree, DnsFreeRecordList, DnsQuery_W, DNS_QUERY_STANDARD, DNS_RECORDW, DNS_TYPE_SRV,
    };

    let qname = format!("_minecraft._tcp.{}", host.trim_end_matches('.'));
    tokio::task::spawn_blocking(move || {
        let wide: Vec<u16> = OsStr::new(&qname).encode_wide().chain(Some(0)).collect();
        let mut results_ptr: *mut DNS_RECORDW = std::ptr::null_mut();
        let status = unsafe {
            DnsQuery_W(
                PCWSTR(wide.as_ptr()),
                DNS_TYPE_SRV,
                DNS_QUERY_STANDARD,
                None,
                (&mut results_ptr as *mut *mut DNS_RECORDW).cast(),
                None,
            )
        };
        if status != WIN32_ERROR(0) || results_ptr.is_null() {
            if !results_ptr.is_null() {
                unsafe { DnsFree(Some(results_ptr.cast()), DnsFreeRecordList) };
            }
            return None;
        }

        let mut candidates = Vec::new();
        let mut cur = results_ptr;
        while !cur.is_null() {
            unsafe {
                let rec = &*cur;
                if rec.wType == DNS_TYPE_SRV.0 {
                    let srv = &rec.Data.Srv;
                    if !srv.pNameTarget.0.is_null() {
                        let mut len = 0usize;
                        while *srv.pNameTarget.0.add(len) != 0 {
                            len += 1;
                        }
                        let target = String::from_utf16_lossy(std::slice::from_raw_parts(
                            srv.pNameTarget.0,
                            len,
                        ));
                        candidates.push(SrvCandidate {
                            priority: srv.wPriority,
                            weight: srv.wWeight,
                            port: srv.wPort,
                            target,
                        });
                    }
                }
                cur = rec.pNext;
            }
        }

        unsafe { DnsFree(Some(results_ptr.cast()), DnsFreeRecordList) };
        select_srv_target(candidates)
    })
    .await
    .ok()
    .flatten()
}

#[cfg(not(windows))]
async fn resolve_minecraft_srv(host: &str) -> Option<(String, u16)> {
    let qname = format!("_minecraft._tcp.{}", host.trim_end_matches('.'));
    let mut query = Vec::with_capacity(64);
    let tx_id: u16 = 0x4D43;
    query.extend_from_slice(&tx_id.to_be_bytes());
    query.extend_from_slice(&0x0100u16.to_be_bytes()); // standard recursive query
    query.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT = 1
    query.extend_from_slice(&[0u8; 6]); // ANCOUNT, NSCOUNT, ARCOUNT
    for label in qname.split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.push(0);
    query.extend_from_slice(&33u16.to_be_bytes()); // QTYPE = SRV
    query.extend_from_slice(&1u16.to_be_bytes()); // QCLASS = IN

    let server_ip = std::fs::read_to_string("/etc/resolv.conf")
        .ok()
        .and_then(|c| {
            c.lines().find_map(|l| {
                let parts: Vec<&str> = l.split_whitespace().collect();
                (parts.first() == Some(&"nameserver"))
                    .then(|| parts.get(1).copied())
                    .flatten()
                    .and_then(|ip| ip.parse::<std::net::IpAddr>().ok())
            })
        })
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::new(1, 1, 1, 1)));

    let bind_addr = if server_ip.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let sock = tokio::net::UdpSocket::bind(bind_addr).await.ok()?;
    sock.send_to(&query, (server_ip, 53)).await.ok()?;
    let mut resp = [0u8; 1024];
    let (n, _) = sock.recv_from(&mut resp).await.ok()?;
    parse_srv_dns_response(&resp[..n], tx_id)
}

#[cfg_attr(windows, allow(dead_code))]
fn read_dns_name(packet: &[u8], mut offset: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut next_offset = offset;
    for _ in 0..64 {
        let len = *packet.get(offset)?;
        if len == 0 {
            if !jumped {
                next_offset = offset + 1;
            }
            return Some((labels.join("."), next_offset));
        }
        if (len & 0xC0) == 0xC0 {
            let b2 = *packet.get(offset + 1)?;
            let ptr = (((len & 0x3F) as usize) << 8) | (b2 as usize);
            if !jumped {
                next_offset = offset + 2;
                jumped = true;
            }
            offset = ptr;
            continue;
        }
        let end = offset.checked_add(1 + len as usize)?;
        let label = std::str::from_utf8(packet.get(offset + 1..end)?).ok()?;
        labels.push(label.to_string());
        offset = end;
        if !jumped {
            next_offset = offset;
        }
    }
    None
}

#[cfg_attr(windows, allow(dead_code))]
fn parse_srv_dns_response(packet: &[u8], expected_tx_id: u16) -> Option<(String, u16)> {
    if packet.len() < 12 || u16::from_be_bytes([packet[0], packet[1]]) != expected_tx_id {
        return None;
    }
    let qdcount = u16::from_be_bytes([packet[4], packet[5]]);
    let ancount = u16::from_be_bytes([packet[6], packet[7]]);
    let mut off = 12;
    for _ in 0..qdcount {
        let (_, next) = read_dns_name(packet, off)?;
        off = next.checked_add(4)?;
    }
    let mut candidates = Vec::new();
    for _ in 0..ancount {
        let (_, after_name) = read_dns_name(packet, off)?;
        let header = packet.get(after_name..after_name + 10)?;
        let rtype = u16::from_be_bytes([header[0], header[1]]);
        let rdlen = u16::from_be_bytes([header[8], header[9]]) as usize;
        let rdata_start = after_name + 10;
        let rdata_end = rdata_start.checked_add(rdlen)?;
        if rtype == 33 && rdlen >= 7 && rdata_end <= packet.len() {
            let priority = u16::from_be_bytes([packet[rdata_start], packet[rdata_start + 1]]);
            let weight = u16::from_be_bytes([packet[rdata_start + 2], packet[rdata_start + 3]]);
            let port = u16::from_be_bytes([packet[rdata_start + 4], packet[rdata_start + 5]]);
            if let Some((target, _)) = read_dns_name(packet, rdata_start + 6) {
                candidates.push(SrvCandidate {
                    priority,
                    weight,
                    port,
                    target,
                });
            }
        }
        off = rdata_end;
    }
    select_srv_target(candidates)
}

pub fn encode_varint(value: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut val = value as u32;
    loop {
        if (val & !0x7f) == 0 {
            bytes.push(val as u8);
            return bytes;
        }
        bytes.push(((val & 0x7f) | 0x80) as u8);
        val >>= 7;
    }
}

pub fn decode_varint(buf: &[u8], offset: usize) -> Option<(i32, usize)> {
    let mut num_read = 0;
    let mut result: i32 = 0;

    loop {
        if num_read >= 5 {
            return None;
        }
        let pos = offset.checked_add(num_read)?;
        let read = *buf.get(pos)?;
        let val = (read & 0x7f) as i32;
        result |= val << (7 * num_read);
        num_read += 1;
        if (read & 0x80) == 0 {
            break;
        }
    }

    Some((result, num_read))
}

pub fn encode_mc_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = encode_varint(bytes.len() as i32);
    out.extend_from_slice(bytes);
    out
}

pub fn strip_formatting_codes(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if let Some(&ch) = chars.get(i) {
            if ch == '§' {
                // Skip the '§' prefix and its trailing formatting code character.
                i = i.saturating_add(2);
            } else {
                result.push(ch);
                i = i.saturating_add(1);
            }
        } else {
            break;
        }
    }

    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn flatten_motd_component(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(flatten_motd_component)
            .collect::<Vec<_>>()
            .join(""),
        serde_json::Value::Object(map) => {
            let mut out = String::new();
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            }
            if let Some(extra) = map.get("extra").and_then(|v| v.as_array()) {
                for item in extra {
                    out.push_str(&flatten_motd_component(item));
                }
            }
            out
        }
        _ => String::new(),
    }
}

pub fn normalize_favicon(raw: Option<&str>) -> Option<String> {
    let clean = raw?.replace(['\r', '\n'], "").trim().to_string();
    if clean.is_empty() {
        return None;
    }
    if clean.starts_with("data:image/") {
        Some(clean)
    } else {
        Some(format!("data:image/png;base64,{}", clean))
    }
}

pub async fn query_minecraft_server_status(
    address: &str,
    timeout_duration: Duration,
) -> ServerPingStatus {
    let checked_at = Utc::now().to_rfc3339();
    let (host, port, explicit_port) = parse_host_port_with_explicit(address);

    if host.is_empty() {
        return ServerPingStatus {
            ip: address.to_string(),
            online: false,
            latency_ms: None,
            players_online: None,
            players_max: None,
            motd: None,
            version_name: None,
            favicon: None,
            checked_at,
        };
    }

    let ping_fut = async {
        let (connect_host, connect_port) = if !explicit_port
            && port == 25565
            && !host.eq_ignore_ascii_case("localhost")
            && host.parse::<std::net::IpAddr>().is_err()
        {
            resolve_minecraft_srv(&host)
                .await
                .unwrap_or_else(|| (host.clone(), port))
        } else {
            (host.clone(), port)
        };

        let started_at = Instant::now();
        let mut stream = TcpStream::connect((connect_host.as_str(), connect_port)).await?;
        let latency_ms = started_at.elapsed().as_millis().max(1) as u32;

        let mut handshake_body = encode_varint(0x00);
        handshake_body.extend(encode_varint(767));
        handshake_body.extend(encode_mc_string(&connect_host));
        handshake_body.extend_from_slice(&connect_port.to_be_bytes());
        handshake_body.extend(encode_varint(1));

        let mut handshake_packet = encode_varint(handshake_body.len() as i32);
        handshake_packet.extend(handshake_body);

        let status_req_body = encode_varint(0x00);
        let mut status_req_packet = encode_varint(status_req_body.len() as i32);
        status_req_packet.extend(status_req_body);

        stream.write_all(&handshake_packet).await?;
        stream.write_all(&status_req_packet).await?;

        let mut buffer = Vec::new();
        let mut temp_buf = [0u8; 4096];

        loop {
            let n = stream.read(&mut temp_buf).await?;
            if n == 0 {
                break;
            }
            if let Some(chunk) = temp_buf.get(..n) {
                buffer.extend_from_slice(chunk);
            }
            if buffer.len() > 512 * 1024 {
                break;
            }

            if let Some((packet_len, len_bytes)) = decode_varint(&buffer, 0) {
                if packet_len <= 0 {
                    break;
                }
                let total_needed = len_bytes.saturating_add(packet_len as usize);
                if buffer.len() >= total_needed {
                    break;
                }
            }
        }

        let (packet_len, len_bytes) = decode_varint(&buffer, 0).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Header terpotong")
        })?;
        if packet_len <= 0 || packet_len > MAX_PING_PACKET_LEN {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Panjang paket tidak valid atau melebihi batas keamanan",
            ));
        }
        let (_packet_id, id_bytes) = decode_varint(&buffer, len_bytes).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Packet ID terpotong")
        })?;

        let str_len_offset = len_bytes.saturating_add(id_bytes);
        let (json_len, json_len_bytes) =
            decode_varint(&buffer, str_len_offset).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "JSON len terpotong")
            })?;
        if json_len <= 0 || json_len > MAX_PING_JSON_LEN {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Panjang JSON tidak valid atau melebihi batas keamanan",
            ));
        }

        let json_start = str_len_offset.saturating_add(json_len_bytes);
        let json_end = json_start.saturating_add(json_len as usize);
        let slice = buffer.get(json_start..json_end).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "JSON payload terpotong")
        })?;

        let json_str = std::str::from_utf8(slice)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let players_online = parsed
            .get("players")
            .and_then(|p| p.get("online"))
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let players_max = parsed
            .get("players")
            .and_then(|p| p.get("max"))
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);

        let motd = parsed
            .get("description")
            .map(flatten_motd_component)
            .map(|s| strip_formatting_codes(&s))
            .filter(|s| !s.is_empty());

        let version_name = parsed
            .get("version")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .map(strip_formatting_codes)
            .filter(|s| !s.is_empty());

        let favicon = parsed
            .get("favicon")
            .and_then(|v| v.as_str())
            .and_then(|f| normalize_favicon(Some(f)));

        Ok::<ServerPingStatus, std::io::Error>(ServerPingStatus {
            ip: address.to_string(),
            online: true,
            latency_ms: Some(latency_ms),
            players_online,
            players_max,
            motd,
            version_name,
            favicon,
            checked_at: Utc::now().to_rfc3339(),
        })
    };

    match timeout(timeout_duration, ping_fut).await {
        Ok(Ok(status)) => status,
        _ => ServerPingStatus {
            ip: address.to_string(),
            online: false,
            latency_ms: None,
            players_online: None,
            players_max: None,
            motd: None,
            version_name: None,
            favicon: None,
            checked_at,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h14_ipv6_host_port_and_srv_selection() {
        assert_eq!(parse_host_port("::1"), ("::1".to_string(), 25565));
        assert_eq!(parse_host_port("[::1]:25577"), ("::1".to_string(), 25577));
        assert_eq!(
            parse_host_port("2001:db8::8080"),
            ("2001:db8::8080".to_string(), 25565)
        );
        let (_, _, explicit_bare) = parse_host_port_with_explicit("play.example.net");
        let (_, _, explicit_port) = parse_host_port_with_explicit("play.example.net:25566");
        assert!(!explicit_bare);
        assert!(explicit_port);

        let selected = select_srv_target(vec![
            SrvCandidate {
                priority: 10,
                weight: 5,
                port: 25566,
                target: "backup.example.net.".to_string(),
            },
            SrvCandidate {
                priority: 5,
                weight: 20,
                port: 25570,
                target: "mc1.example.net.".to_string(),
            },
        ]);
        assert_eq!(selected, Some(("mc1.example.net".to_string(), 25570)));
    }
}
