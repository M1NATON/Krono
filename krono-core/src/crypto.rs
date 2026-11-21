use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chacha20poly1305::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const ENVELOPE_VERSION: u8 = 1;
pub const KDF_ID: &str = "argon2id-v19";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub version: u8,
    pub kdf: String,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    if password.is_empty() {
        return Err("Master password must not be empty".to_string());
    }
    let params = Params::new(19_456, 2, 1, Some(32))
        .map_err(|e| format!("Argon2 params error: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut *key)
        .map_err(|e| format!("Key derivation failed: {e}"))?;
    Ok(key)
}

/// Encrypts `plaintext` bytes with `password` using Argon2id + ChaCha20Poly1305.
/// Salt (16B) and nonce (12B) are always generated with `OsRng`.
pub fn encrypt_note_bytes(plaintext: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if password.is_empty() {
        return Err("Master password must not be empty".to_string());
    }
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    let key_bytes = derive_key(password, &salt)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&*key_bytes));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| "Encryption failed".to_string())?;

    let env = EncryptedEnvelope {
        version: ENVELOPE_VERSION,
        kdf: KDF_ID.to_string(),
        salt: BASE64.encode(salt),
        nonce: BASE64.encode(nonce_bytes),
        ciphertext: BASE64.encode(ciphertext),
    };
    serde_json::to_vec_pretty(&env).map_err(|e| format!("Envelope encode error: {e}"))
}

/// Decrypts an `.md.enc` JSON envelope with `password`, verifying the AEAD tag.
pub fn decrypt_note_bytes(
    envelope_bytes: &[u8],
    password: &str,
) -> Result<Zeroizing<Vec<u8>>, String> {
    if password.is_empty() {
        return Err("Master password must not be empty".to_string());
    }
    let env: EncryptedEnvelope =
        serde_json::from_slice(envelope_bytes).map_err(|e| format!("Invalid encrypted note format: {e}"))?;
    if env.version != ENVELOPE_VERSION || env.kdf != KDF_ID {
        return Err(format!(
            "Unsupported encrypted envelope version {} / kdf {}",
            env.version, env.kdf
        ));
    }
    let salt = BASE64
        .decode(env.salt.trim())
        .map_err(|_| "Invalid salt encoding".to_string())?;
    let nonce_bytes = BASE64
        .decode(env.nonce.trim())
        .map_err(|_| "Invalid nonce encoding".to_string())?;
    if nonce_bytes.len() != 12 {
        return Err("Invalid nonce length".to_string());
    }
    let ciphertext = BASE64
        .decode(env.ciphertext.trim())
        .map_err(|_| "Invalid ciphertext encoding".to_string())?;

    let key_bytes = derive_key(password, &salt)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&*key_bytes));
    let plain = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext.as_ref())
        .map_err(|_| "Decryption failed: wrong password or corrupted data".to_string())?;
    Ok(Zeroizing::new(plain))
}

pub fn encrypt_note_str(plaintext: &str, password: &str) -> Result<String, String> {
    let bytes = encrypt_note_bytes(plaintext.as_bytes(), password)?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 error: {e}"))
}

pub fn decrypt_note_str(
    envelope_str: &str,
    password: &str,
) -> Result<Zeroizing<String>, String> {
    let plain = decrypt_note_bytes(envelope_str.as_bytes(), password)?;
    let s = String::from_utf8(plain.to_vec())
        .map_err(|_| "Decrypted content is not valid UTF-8".to_string())?;
    Ok(Zeroizing::new(s))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretEntry {
    pub label: String,
    pub value: String,
    pub sensitive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SecretBlock {
    pub title: Option<String>,
    pub entries: Vec<SecretEntry>,
}

fn is_sensitive_key(label: &str) -> bool {
    let l = label.trim().to_lowercase();
    if matches!(
        l.as_str(),
        "сервис" | "service" | "сайт" | "site" | "url" | "логин" | "login" | "user" | "username" | "email" | "почта" | "заметка" | "note"
    ) {
        return false;
    }
    true
}

/// Parses a fenced ```` ```secret ```` block into structured key/value pairs.
pub fn parse_secret_block(block_body: &str) -> SecretBlock {
    let mut block = SecretBlock::default();
    for raw_line in block_body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(title) = line.strip_prefix('#').map(str::trim) {
            if !title.is_empty() && block.title.is_none() {
                block.title = Some(title.to_string());
                continue;
            }
        }
        if let Some((k, v)) = line.split_once(':') {
            let label = k.trim().to_string();
            let value = v.trim().to_string();
            if !label.is_empty() {
                let sensitive = is_sensitive_key(&label);
                block.entries.push(SecretEntry {
                    label,
                    value,
                    sensitive,
                });
            }
        }
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypt_decrypt_cyrillic() {
        let text = "# Секреты\nСервис: GitHub\nПароль: супер-секретный-пароль-123";
        let enc = encrypt_note_str(text, "master-pass").expect("encrypt");
        let dec = decrypt_note_str(&enc, "master-pass").expect("decrypt");
        assert_eq!(&*dec, text);
    }

    #[test]
    fn wrong_password_returns_error_without_panic() {
        let enc = encrypt_note_bytes(b"secret payload", "correct-pass").unwrap();
        let err = decrypt_note_bytes(&enc, "wrong-pass").unwrap_err();
        assert!(err.contains("Decryption failed"));
    }

    #[test]
    fn tampered_ciphertext_fails_aead_check() {
        let enc = encrypt_note_str("hello vault", "pw123").unwrap();
        let mut env: EncryptedEnvelope = serde_json::from_str(&enc).unwrap();
        let mut raw = BASE64.decode(&env.ciphertext).unwrap();
        raw[0] ^= 0xFF;
        env.ciphertext = BASE64.encode(raw);
        let tampered = serde_json::to_string(&env).unwrap();
        assert!(decrypt_note_str(&tampered, "pw123").is_err());
    }

    #[test]
    fn empty_password_is_rejected() {
        assert!(encrypt_note_bytes(b"abc", "").is_err());
    }

    #[test]
    fn each_encryption_uses_unique_salt_and_nonce() {
        let e1: EncryptedEnvelope =
            serde_json::from_str(&encrypt_note_str("same", "pw").unwrap()).unwrap();
        let e2: EncryptedEnvelope =
            serde_json::from_str(&encrypt_note_str("same", "pw").unwrap()).unwrap();
        assert_ne!(e1.salt, e2.salt);
        assert_ne!(e1.nonce, e2.nonce);
        assert_ne!(e1.ciphertext, e2.ciphertext);
    }

    #[test]
    fn parses_secret_block_entries() {
        let body = "# Банк\nСервис: Tinkoff\nЛогин: user@mail.ru\nПароль: S3cr3t!\nAPI-ключ: tok_abc123";
        let parsed = parse_secret_block(body);
        assert_eq!(parsed.title.as_deref(), Some("Банк"));
        assert_eq!(parsed.entries.len(), 4);
        assert!(!parsed.entries[0].sensitive);
        assert!(!parsed.entries[1].sensitive);
        assert!(parsed.entries[2].sensitive);
        assert!(parsed.entries[3].sensitive);
        assert_eq!(parsed.entries[2].value, "S3cr3t!");
    }
}
