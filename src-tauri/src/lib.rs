// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::Emitter;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Device {
    id: String,
    name: String,
    ip: String,
    port: u16,
}

#[derive(Clone, Serialize)]
struct TransferProgress {
    filename: String,
    progress: f64,
    speed: String,
}

struct AppState {
    device_id: String,
    port: u16,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn start_server(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let app_state = state.lock().unwrap();
    let port = app_state.port;

    thread::spawn(move || {
        let listener =
            TcpListener::bind(format!("0.0.0.0:{}", port)).expect("Failed to bind server");

        println!("Server listening on port {}", port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let handle = app_handle.clone();
                    thread::spawn(move || handle_client(stream, handle));
                }
                Err(e) => eprintln!("Connection failed: {}", e),
            }
        }
    });

    Ok(format!("Server started on port {}", port))
}

fn handle_client(mut stream: TcpStream, app_handle: tauri::AppHandle) {
    let mut buffer = [0; 8192];

    if let Ok(n) = stream.read(&mut buffer) {
        let (filename, filesize) = {
            let metadata = String::from_utf8_lossy(&buffer[..n]);
            let parts: Vec<&str> = metadata.trim().split('|').collect();

            if parts.len() < 2 {
                return;
            }

            let filename = parts[0].to_string();
            let filesize: u64 = parts[1].parse().unwrap_or(0);
            (filename, filesize)
        };

        println!("Receiving: {} ({} bytes)", filename, filesize);

        let _ = stream.write_all(b"ACK");

        let mut file_data = Vec::new();
        let mut total_received = 0u64;
        let start_time = std::time::Instant::now();

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    file_data.extend_from_slice(&buffer[..n]);
                    total_received += n as u64;

                    let progress = (total_received as f64 / filesize as f64) * 100.0;
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (total_received as f64 / elapsed) / (1024.0 * 1024.0)
                    } else {
                        0.0
                    };

                    let _ = app_handle.emit(
                        "transfer-progress",
                        TransferProgress {
                            filename: filename.clone(),
                            progress,
                            speed: format!("{:.2} MB/s", speed),
                        },
                    );

                    if total_received >= filesize {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading: {}", e);
                    break;
                }
            }
        }

        let downloads_dir =
            dirs::download_dir().unwrap_or_else(|| std::env::current_dir().unwrap());
        let filepath = downloads_dir.join(&filename);

        if let Err(e) = std::fs::write(&filepath, file_data) {
            eprintln!("Failed to save file: {}", e);
        } else {
            println!("File saved successfully: {:?}", filepath);
            let _ = app_handle.emit("file-received", filename);
        }
    }
}

#[tauri::command]
async fn broadcast_presence(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();
    let device_id = app_state.device_id.clone();
    let port = app_state.port;

    thread::spawn(move || {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind UDP socket");
        socket.set_broadcast(true).expect("Failed to set broadcast");

        let broadcast_addr = "255.255.255.255:37020";
        let hostname = whoami::hostname().unwrap_or_else(|_| "Unknown".to_string());
        let message = format!("{}|{}|{}", device_id, hostname, port);

        loop {
            let _ = socket.send_to(message.as_bytes(), broadcast_addr);
            thread::sleep(std::time::Duration::from_secs(3));
        }
    });

    Ok(())
}

#[tauri::command]
async fn discover_devices(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<Device>, String> {
    let socket = UdpSocket::bind("0.0.0.0:37020").map_err(|e| e.to_string())?;

    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .map_err(|e| e.to_string())?;

    let mut devices = Vec::new();
    let mut buffer = [0; 1024];
    let mut seen_ids = std::collections::HashSet::new();

    let start_time = std::time::Instant::now();

    while start_time.elapsed().as_secs() < 5 {
        if let Ok((amt, src)) = socket.recv_from(&mut buffer) {
            let message = String::from_utf8_lossy(&buffer[..amt]);
            let parts: Vec<&str> = message.split('|').collect();

            if parts.len() >= 3 {
                let device_id = parts[0].to_string();

                let app_state = state.lock().unwrap();
                if device_id != app_state.device_id && !seen_ids.contains(&device_id) {
                    seen_ids.insert(device_id.clone());

                    let device = Device {
                        id: device_id,
                        name: parts[1].to_string(),
                        ip: src.ip().to_string(),
                        port: parts[2].parse().unwrap_or(37021),
                    };

                    devices.push(device);
                }
            }
        }
    }

    Ok(devices)
}

#[tauri::command]
async fn send_file(
    filepath: String,
    target_ip: String,
    target_port: u16,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    thread::spawn(
        move || match TcpStream::connect(format!("{}:{}", target_ip, target_port)) {
            Ok(mut stream) => {
                let file_data = match std::fs::read(&filepath) {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("Failed to read file: {}", e);
                        return;
                    }
                };

                let filename = std::path::Path::new(&filepath)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();

                let metadata = format!("{}|{}\n", filename, file_data.len());
                let _ = stream.write_all(metadata.as_bytes());

                let mut ack = [0; 3];
                let _ = stream.read_exact(&mut ack);

                let chunk_size = 65536;
                let total_size = file_data.len() as f64;
                let start_time = std::time::Instant::now();

                for (i, chunk) in file_data.chunks(chunk_size).enumerate() {
                    if let Err(e) = stream.write_all(chunk) {
                        eprintln!("Error sending chunk: {}", e);
                        break;
                    }

                    let sent = ((i + 1) * chunk_size).min(file_data.len());
                    let progress = (sent as f64 / total_size) * 100.0;
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (sent as f64 / elapsed) / (1024.0 * 1024.0)
                    } else {
                        0.0
                    };

                    let _ = app_handle.emit(
                        "transfer-progress",
                        TransferProgress {
                            filename: filename.to_string(),
                            progress,
                            speed: format!("{:.2} MB/s", speed),
                        },
                    );
                }

                println!("File sent successfully!");
                let _ = app_handle.emit("file-sent", filename);
            }
            Err(e) => eprintln!("Failed to connect: {}", e),
        },
    );

    Ok("Transfer started".to_string())
}

#[tauri::command]
async fn get_local_ip() -> Result<String, String> {
    use local_ip_address::local_ip;

    match local_ip() {
        Ok(ip) => Ok(ip.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let device_id = uuid::Uuid::new_v4().to_string()[..8].to_uppercase();
    let port = 37021;

    let app_state = Arc::new(Mutex::new(AppState {
        device_id: device_id.clone(),
        port,
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            start_server,
            broadcast_presence,
            discover_devices,
            send_file,
            get_local_ip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
