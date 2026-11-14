use chrono::Utc;
use launcher_types::ServerPingStatus;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

pub const MAX_PING_PACKET_LEN: i32 = 256 * 1024;
pub const MAX_PING_JSON_LEN: i32 = 256 * 1024;

pub fn parse_host_port(address: &str) -> (String, u16) {
    let trimmed = address.trim();
    if trimmed.starts_with('[') {
        if let Some(close_idx) = trimmed.find(']') {
            let host = &trimmed[1..close_idx];
            let rest = &trimmed[close_idx + 1..];
            if let Some(port_str) = rest.strip_prefix(':') {
                if let Ok(port) = port_str.parse::<u16>() {
                    return (host.to_string(), port);
                }
            }
            return (host.to_string(), 25565);
        }
    }

    if let Some((host, port_str)) = trimmed.split_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            return (host.to_string(), port);
        }
    }

    (trimmed.to_string(), 25565)
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
                i = i.saturating_add(2); // skip § and color code
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
    let (host, port) = parse_host_port(address);

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
        let started_at = Instant::now();
        let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await?;
        let latency_ms = started_at.elapsed().as_millis().max(1) as u32;

        // 1. Handshake Packet (ID 0x00, Protocol 767, Host, Port, NextState 1)
        let mut handshake_body = encode_varint(0x00);
        handshake_body.extend(encode_varint(767));
        handshake_body.extend(encode_mc_string(&host));
        handshake_body.extend_from_slice(&port.to_be_bytes());
        handshake_body.extend(encode_varint(1));

        let mut handshake_packet = encode_varint(handshake_body.len() as i32);
        handshake_packet.extend(handshake_body);

        // 2. Status Request Packet (ID 0x00)
        let status_req_body = encode_varint(0x00);
        let mut status_req_packet = encode_varint(status_req_body.len() as i32);
        status_req_packet.extend(status_req_body);

        // Send both packets
        stream.write_all(&handshake_packet).await?;
        stream.write_all(&status_req_packet).await?;

        // 3. Read Response Packet
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
                // Safety limit
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
