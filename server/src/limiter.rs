use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

/// Limitador simple de intentos de acceso en memoria.
///
/// Se usa por correo electrónico normalizado; suficiente para ~20 usuarios y protege
/// contra fuerza bruta básica sin dependencias externas. En varias instancias de API
/// debería sustituirse por un almacén compartido (Redis o la propia base).
pub struct LoginLimiter {
    max: usize,
    ventana: Duration,
    intentos: Mutex<HashMap<String, Vec<Instant>>>,
}

impl LoginLimiter {
    pub fn nuevo(max: usize, ventana_segundos: u64) -> Self {
        Self {
            max,
            ventana: Duration::from_secs(ventana_segundos),
            intentos: Mutex::new(HashMap::new()),
        }
    }

    pub fn permitir(&self, clave: &str) -> bool {
        let mut guardia = self.intentos.lock().unwrap_or_else(|e| e.into_inner());
        let ahora = Instant::now();
        let lista = guardia.entry(clave.to_string()).or_default();
        lista.retain(|marca| ahora.duration_since(*marca) < self.ventana);
        lista.len() < self.max
    }

    pub fn registrar_fallo(&self, clave: &str) {
        let mut guardia = self.intentos.lock().unwrap_or_else(|e| e.into_inner());
        let ahora = Instant::now();
        let lista = guardia.entry(clave.to_string()).or_default();
        lista.retain(|marca| ahora.duration_since(*marca) < self.ventana);
        lista.push(ahora);
    }

    pub fn limpiar(&self, clave: &str) {
        let mut guardia = self.intentos.lock().unwrap_or_else(|e| e.into_inner());
        guardia.remove(clave);
    }
}
