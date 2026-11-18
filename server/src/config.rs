use std::time::Duration;

use anyhow::{bail, Context, Result};

/// Configuración obtenida del entorno. No contiene valores por omisión para secretos.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub access_ttl: Duration,
    pub refresh_ttl: Duration,
    pub idle_ttl: Duration,
    pub reserva_ttl: Duration,
    pub bind_addr: String,
}

impl Config {
    pub fn desde_entorno() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL es obligatoria")?;
        let jwt_secret = std::env::var("JWT_SECRET")
            .context("JWT_SECRET es obligatoria (mínimo 32 caracteres)")?;
        if jwt_secret.len() < 32 {
            bail!("JWT_SECRET debe tener al menos 32 caracteres");
        }
        let access_ttl = minutos("ACCESS_TTL_MINUTOS", 18 * 60)?;
        let refresh_ttl = minutos("REFRESH_TTL_MINUTOS", 24 * 60)?;
        let idle_ttl = minutos("IDLE_TTL_MINUTOS", 18 * 60)?;
        let reserva_ttl = minutos("RESERVA_TTL_MINUTOS", 30)?;
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

        Ok(Self { database_url, jwt_secret, access_ttl, refresh_ttl, idle_ttl, reserva_ttl, bind_addr })
    }

    /// Configuración determinista para pruebas automatizadas.
    pub fn para_pruebas(database_url: String) -> Self {
        Self {
            database_url,
            jwt_secret: "secreto-de-pruebas-con-mas-de-32-caracteres".to_string(),
            access_ttl: Duration::from_secs(18 * 60 * 60),
            refresh_ttl: Duration::from_secs(24 * 60 * 60),
            idle_ttl: Duration::from_secs(18 * 60 * 60),
            reserva_ttl: Duration::from_secs(30 * 60),
            bind_addr: "127.0.0.1:0".to_string(),
        }
    }
}

fn minutos(variable: &str, por_omision: u64) -> Result<Duration> {
    match std::env::var(variable) {
        Ok(valor) => {
            let n: u64 = valor
                .parse()
                .with_context(|| format!("{variable} debe ser un número entero de minutos"))?;
            if n == 0 {
                bail!("{variable} debe ser mayor que cero");
            }
            Ok(Duration::from_secs(n * 60))
        }
        Err(_) => Ok(Duration::from_secs(por_omision * 60)),
    }
}
