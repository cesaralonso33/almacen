//! Protección de la clave de SQLCipher.
//!
//! En Windows se guarda en el Administrador de credenciales, que la protege con
//! DPAPI y el perfil del usuario. La clave nunca se escribe en el repositorio.

use uuid::Uuid;

fn nueva_clave() -> String {
    format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

#[cfg(windows)]
pub fn clave_sqlite() -> Result<String, String> {
    let entrada = keyring::Entry::new("Custodia CradSystem", "sqlite-key")
        .map_err(|error| format!("No se pudo abrir el almacén de credenciales: {error}"))?;

    match entrada.get_password() {
        Ok(clave) if clave.len() >= 32 => Ok(clave),
        Ok(_) => {
            let clave = nueva_clave();
            entrada
                .set_password(&clave)
                .map_err(|error| format!("No se pudo guardar la clave local: {error}"))?;
            Ok(clave)
        }
        Err(keyring::Error::NoEntry) => {
            let clave = nueva_clave();
            entrada
                .set_password(&clave)
                .map_err(|error| format!("No se pudo guardar la clave local: {error}"))?;
            Ok(clave)
        }
        Err(error) => Err(format!("No se pudo leer la clave local: {error}")),
    }
}

#[cfg(not(windows))]
pub fn clave_sqlite() -> Result<String, String> {
    let clave = std::env::var("CUSTODIA_SQLITE_KEY")
        .ok()
        .filter(|clave| clave.len() >= 32)
        .unwrap_or_else(|| "clave-local-de-pruebas-con-mas-de-32-caracteres".to_string());
    Ok(clave)
}
