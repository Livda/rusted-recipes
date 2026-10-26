use argon2::{Argon2, PasswordHash, PasswordVerifier};

/// Hache un mot de passe en bcrypt, lisible aussi par Symfony.
pub fn hash(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
}

/// Vérifie un mot de passe contre un hash Symfony (`auto` : bcrypt ou argon2id).
pub fn verify(password: &str, hash: &str) -> bool {
    if hash.starts_with("$argon2") {
        PasswordHash::new(hash)
            .map(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
            .unwrap_or(false)
    } else if hash.starts_with("$2") {
        bcrypt::verify(password, hash).unwrap_or(false)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::PasswordHasher;

    #[test]
    fn bcrypt_hash() {
        let hash = bcrypt::hash("secret", 4).unwrap();
        assert!(verify("secret", &hash));
        assert!(!verify("other", &hash));
    }

    #[test]
    fn argon2id_hash() {
        let hash = Argon2::default()
            .hash_password(b"secret")
            .unwrap()
            .to_string();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify("secret", &hash));
        assert!(!verify("other", &hash));
    }

    #[test]
    fn unknown_format_is_rejected() {
        assert!(!verify("secret", "secret"));
        assert!(!verify("secret", ""));
        assert!(!verify("secret", "$argon2id$garbage"));
    }
}
