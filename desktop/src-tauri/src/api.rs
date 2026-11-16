//! Cliente HTTP hacia la API institucional.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::sync::Transporte;

pub struct ClienteHttp {
    base: String,
    token: String,
    cliente: Client,
}

impl ClienteHttp {
    pub fn nuevo(base: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            token: token.into(),
            cliente: Client::new(),
        }
    }

    pub async fn login(
        base: &str,
        correo: &str,
        password: &str,
    ) -> Result<Value, String> {
        let cliente = Client::new();
        let respuesta = cliente
            .post(format!("{}/api/v1/auth/login", base.trim_end_matches('/')))
            .json(&json!({ "correo": correo, "password": password }))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        interpretar(respuesta).await
    }

    pub async fn registrar_dispositivo(&self, nombre: &str) -> Result<Value, String> {
        let respuesta = self
            .cliente
            .post(format!("{}/api/v1/sync/dispositivos", self.base))
            .bearer_auth(&self.token)
            .json(&json!({ "nombre": nombre, "plataforma": std::env::consts::OS }))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        interpretar(respuesta).await
    }
}

async fn interpretar(respuesta: reqwest::Response) -> Result<Value, String> {
    let estado = respuesta.status();
    let cuerpo = respuesta.text().await.map_err(|error| error.to_string())?;
    if !estado.is_success() {
        return Err(format!("HTTP {estado}: {cuerpo}"));
    }
    serde_json::from_str(&cuerpo).map_err(|error| format!("respuesta no JSON: {error}"))
}

#[async_trait]
impl Transporte for ClienteHttp {
    async fn pull(&self, dispositivo: &str, cursor: i64, limite: i64) -> Result<Value, String> {
        let respuesta = self
            .cliente
            .get(format!("{}/api/v1/sync/pull", self.base))
            .bearer_auth(&self.token)
            .query(&[
                ("dispositivo_id", dispositivo.to_string()),
                ("cursor", cursor.to_string()),
                ("limite", limite.to_string()),
            ])
            .send()
            .await
            .map_err(|error| error.to_string())?;
        interpretar(respuesta).await
    }

    async fn push(&self, dispositivo: &str, operaciones: Vec<Value>) -> Result<Value, String> {
        let respuesta = self
            .cliente
            .post(format!("{}/api/v1/sync/push", self.base))
            .bearer_auth(&self.token)
            .json(&json!({ "dispositivo_id": dispositivo, "operaciones": operaciones }))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        interpretar(respuesta).await
    }

    async fn snapshot(&self, dispositivo: &str) -> Result<Value, String> {
        let respuesta = self
            .cliente
            .get(format!("{}/api/v1/sync/snapshot", self.base))
            .bearer_auth(&self.token)
            .query(&[("dispositivo_id", dispositivo)])
            .send()
            .await
            .map_err(|error| error.to_string())?;
        interpretar(respuesta).await
    }
}
