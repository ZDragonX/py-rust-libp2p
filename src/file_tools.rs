use libp2p::{identity, PeerId};
use std::{fs::File, io::{Read, Write}, path::Path, path::PathBuf};
use std::error::Error;

// 生成Ed25519密钥对
pub fn generate_ed25519_keypair() -> identity::Keypair {
    identity::Keypair::generate_ed25519()
}

pub fn save_keypair_to_file(keypair: &identity::Keypair, file_path_str: String) -> std::io::Result<()> {

    // 将String转换为PathBuf
    let mut file_path = PathBuf::from(file_path_str);

    // 检查路径是否存在，如果存在且是文件，则返回错误
    if file_path.exists() && file_path.is_file() {
        return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Path points to an existing file"));
    }

    // 如果路径存在但不是文件（可能是目录），则在路径后面加上"node.key"
    if file_path.exists() && file_path.is_dir() {
        file_path = file_path.join("node.key");
    }

    // let file_path = PathBuf::from(file_path_str);
    let keypair_bytes = keypair.to_protobuf_encoding()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;;
    let mut file = File::create(file_path)?;
    file.write_all(&keypair_bytes)?;
    Ok(())
}

pub fn load_keypair_from_file(file_path_str: String) -> std::io::Result<identity::Keypair> {
    let file_path = PathBuf::from(file_path_str);
    let mut file = File::open(file_path)?;
    let mut keypair_bytes = Vec::new();
    file.read_to_end(&mut keypair_bytes)?;
    let keypair = identity::Keypair::from_protobuf_encoding(&keypair_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(keypair)
}

