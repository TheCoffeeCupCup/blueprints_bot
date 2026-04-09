use aes_gcm::{
    AeadCore as _, Aes256Gcm, Key, KeyInit,
    aead::{Aead as _, OsRng},
};
use sha2::{Digest as _, Sha256};

use crate::logging;

pub struct Passphrase {
    pub primary: String,
    pub secondary: Option<String>,
}

fn passphrase_to_key(passphrase: &str) -> Key<Aes256Gcm> {
    let hash = Sha256::digest(passphrase);
    *Key::<Aes256Gcm>::from_slice(&hash)
}

pub fn encrypt(data: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let key = passphrase_to_key(passphrase);
    let cipher = Aes256Gcm::new(&key);

    // Ensures that two inputs with the same content will produce different outputs.
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let encrypted_data = cipher
        .encrypt(&nonce, data)
        .map_err(|err| err.to_string())?;

    let mut result = Vec::new();
    result.extend(nonce);
    result.extend(encrypted_data);

    Ok(result)
}

fn try_decrypt(encrypted_data: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let Some((nonce, encrypted_data)) = encrypted_data.split_at_checked(12) else {
        return Err(format!(
            "Not enough bytes to fit nonce and encrypted data: {}",
            encrypted_data.len()
        ));
    };

    let key = passphrase_to_key(passphrase);
    let cipher = Aes256Gcm::new(&key);

    let decrypted_data = cipher
        .decrypt(nonce.into(), encrypted_data)
        .map_err(|_| "encryption key is wrong or data is corrupted".to_string());

    decrypted_data
}

pub fn decrypt(encrypted_data: &[u8], passphrase: &Passphrase) -> Option<Vec<u8>> {
    match try_decrypt(encrypted_data, &passphrase.primary) {
        Ok(decrypted_data) => Some(decrypted_data),
        Err(primary_key_err) => {
            let Some(secondary_key) = passphrase.secondary.as_ref() else {
                logging::error!("Couldn't decrypt data: {primary_key_err}");
                return None;
            };

            logging::warning!("Couldn't decrypt data with primary key: {primary_key_err}");

            match try_decrypt(encrypted_data, secondary_key) {
                Ok(decrypted_data) => Some(decrypted_data),
                Err(err) => {
                    logging::error!("Couldn't decrypt data with secondary key: {err}");
                    return None;
                }
            }
        }
    }
}
