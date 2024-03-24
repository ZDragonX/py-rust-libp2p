use k256::{
    ecdsa::{self, Signature as EcdsaSignature, signature::{ Signer, Verifier}, SigningKey, VerifyingKey},
    elliptic_curve::sec1::ToEncodedPoint,
};
use k256::elliptic_curve::generic_array::GenericArray;
use k256::elliptic_curve::generic_array::typenum::U32;
use sha3::{Digest, Keccak256};
use std::{fs::File, io::{self, Read, Write}, path::PathBuf};
use hex::{encode, decode};
use rand::thread_rng;

// 生成ECDSA（secp256k1）密钥对
pub fn generate_ecdsa_keypair() -> SigningKey {
    SigningKey::random(&mut rand::thread_rng())
}

// 保存密钥对到文件
pub fn save_keypair_to_file(signing_key: &SigningKey, file_path_str: String) -> io::Result<()> {
    let mut file_path = PathBuf::from(&file_path_str);

    if file_path.exists() && file_path.is_dir() {
        file_path = file_path.join("node.key");
    }

    let mut file = File::create(file_path)?;
    let signing_key_bytes = signing_key.to_bytes();
    file.write_all(&signing_key_bytes)?;
    Ok(())
}

// 修改load_keypair_from_file以适配GenericArray
pub fn load_keypair_from_file(file_path_str: String) -> io::Result<SigningKey> {
    let mut file = File::open(PathBuf::from(file_path_str))?;
    let mut signing_key_bytes = Vec::new();
    file.read_to_end(&mut signing_key_bytes)?;

    let signing_key_bytes = GenericArray::clone_from_slice(&signing_key_bytes);
    SigningKey::from_bytes(&signing_key_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

// 使用私钥进行签名
pub fn sign_data(file_path_str: String, data: Vec<u8>) -> io::Result<String> {
    let signing_key = load_keypair_from_file(file_path_str)?;
    let signature: EcdsaSignature = signing_key.sign(&data);
    Ok(encode(signature.to_der()))
}

// 将公钥序列化为字符串
pub fn get_serialize_public_key(file_path_str: String) -> io::Result<String> {
    let signing_key = load_keypair_from_file(file_path_str)?;
    let verifying_key = VerifyingKey::from(&signing_key);
    let public_key_bytes = verifying_key.to_encoded_point(false).as_bytes().to_vec();
    Ok(encode(public_key_bytes))
}

// 从字符串转换为公钥
pub fn deserialize_public_key(encoded: String) -> io::Result<VerifyingKey> {
    let bytes = decode(encoded).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // 检查bytes数组的长度是否正确，对于压缩公钥应为33字节
    if bytes.len() != 65 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Incorrect public key length"));
    }

    // 直接使用bytes创建VerifyingKey，无需转换为GenericArray
    VerifyingKey::from_sec1_bytes(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

// 使用公钥验证签名
pub fn verify_signature(public_key_str: String, data: Vec<u8>, signature_str: String) -> io::Result<()> {
    let public_key = deserialize_public_key(public_key_str)?;
    let signature_bytes = decode(signature_str).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let signature = EcdsaSignature::from_der(&signature_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    if public_key.verify(&data, &signature).is_ok() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Verification failed"))
    }
}

// 新增：生成EVM地址
pub fn generate_evm_address(signing_key: &SigningKey) -> String {
    let verifying_key = VerifyingKey::from(signing_key);
    let encoded_point = verifying_key.to_encoded_point(false);
    let public_key_bytes = &encoded_point.as_bytes()[1..]; // 移除前缀0x04
    let hash = Keccak256::digest(public_key_bytes);
    encode(&hash[hash.len() - 20..]) // 取哈希的最后20字节
}