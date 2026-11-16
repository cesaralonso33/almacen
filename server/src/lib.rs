//! Biblioteca de la API de almacén y custodia de expedientes jurídicos.

pub mod api;
pub mod audit;
pub mod auth;
pub mod config;
pub mod error;
pub mod limiter;
pub mod sync;

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::limiter::LoginLimiter;

/// Estado compartido por la API: conexión, configuración y limitador de acceso.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub limitador: Arc<LoginLimiter>,
}

impl AppState {
    pub fn nuevo(db: PgPool, config: Config, limitador: LoginLimiter) -> Self {
        Self { db, config: Arc::new(config), limitador: Arc::new(limitador) }
    }
}

