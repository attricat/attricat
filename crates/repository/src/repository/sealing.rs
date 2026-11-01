//! AES-256-GCM for stored credentials. Each ciphertext is the nonce followed
//! by the sealed value, and is bound by `context` to the row and field it
//! belongs to, so it cannot be moved to another.

use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};

/// Size of the keys [`seal`] and [`open`] take.
pub(super) const SEALING_KEY_BYTES: usize = 32;

fn sealing_key(key: &[u8; SEALING_KEY_BYTES]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).expect("a 32-byte AES-256-GCM key"))
}

/// Encrypts `plaintext`; `None` if the system's random source is unavailable.
pub(super) fn seal(
    key: &[u8; SEALING_KEY_BYTES],
    context: &str,
    plaintext: &str,
) -> Option<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new().fill(&mut nonce).ok()?;
    let mut sealed = plaintext.as_bytes().to_vec();
    sealing_key(key)
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(context.as_bytes()),
            &mut sealed,
        )
        .ok()?;
    let mut ciphertext = nonce.to_vec();
    ciphertext.extend_from_slice(&sealed);
    Some(ciphertext)
}

/// Decrypts a [`seal`] result; `None` if it was tampered with, moved from
/// another `context`, or sealed under another key.
pub(super) fn open(
    key: &[u8; SEALING_KEY_BYTES],
    context: &str,
    ciphertext: &[u8],
) -> Option<String> {
    if ciphertext.len() < NONCE_LEN {
        return None;
    }
    let (nonce, sealed) = ciphertext.split_at(NONCE_LEN);
    let mut sealed = sealed.to_vec();
    let plaintext = sealing_key(key)
        .open_in_place(
            Nonce::try_assume_unique_for_key(nonce).ok()?,
            Aad::from(context.as_bytes()),
            &mut sealed,
        )
        .ok()?;
    String::from_utf8(plaintext.to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::{open, seal};

    #[test]
    fn a_ciphertext_opens_only_with_its_key_and_context() {
        let key = [7; 32];
        let sealed = seal(&key, "row-a:secret", "value").unwrap();
        assert_eq!(open(&key, "row-a:secret", &sealed).unwrap(), "value");
        assert!(open(&key, "row-b:secret", &sealed).is_none());
        assert!(open(&[8; 32], "row-a:secret", &sealed).is_none());
        assert!(open(&key, "row-a:secret", &sealed[..4]).is_none());
    }
}
