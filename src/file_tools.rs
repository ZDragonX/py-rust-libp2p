use libp2p::{identity, PeerId};
use std::{fs::File, io::{Read, Write}, path::Path, path::PathBuf};
use std::error::Error;
use base64::{encode, decode};

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

/// 使用私钥进行签名
pub fn sign_data(file_path_str: String, data: String) -> std::io::Result<String> {
    let keypair = load_keypair_from_file(file_path_str)?;
    let signature = keypair.sign(data.as_bytes()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    Ok(encode(signature))
}

/// 将公钥序列化为字符串
pub fn get_serialize_public_key(file_path_str: String) -> std::io::Result<String> {
    let keypair = load_keypair_from_file(file_path_str)?;
    let public_key_bytes = keypair.public().encode_protobuf();
    Ok(encode(public_key_bytes))
}

/// 从字符串转换为公钥
pub fn deserialize_public_key(encoded: String) -> std::io::Result<identity::PublicKey> {
    let bytes = decode(encoded).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    identity::PublicKey::try_decode_protobuf(&bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// 使用公钥验证签名
pub fn verify_signature(public_key: &identity::PublicKey, data: String, signature: String) -> std::io::Result<()> {
    let signature_bytes = decode(signature).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    if public_key.verify(data.as_bytes(), &signature_bytes) {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Verification failed"))
    }
}