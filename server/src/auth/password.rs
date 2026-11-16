use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};

/// Genera un hash Argon2id con sal aleatoria.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let sal = SaltString::generate(&mut OsRng);
    Ok(Argon2::default().hash_password(password.as_bytes(), &sal)?.to_string())
}

/// Verifica una contraseña contra un hash Argon2id almacenado.
pub fn verificar_password(hash: &str, password: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(almacenado) => Argon2::default()
            .verify_password(password.as_bytes(), &almacenado)
            .is_ok(),
        Err(_) => false,
    }
}
