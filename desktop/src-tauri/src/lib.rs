use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::Manager;
use std::path::PathBuf;

#[derive(Serialize)] struct Expediente {id:String,numero:String,descripcion:String,estado:String}
struct DbPath(PathBuf);
fn open(path:&PathBuf)->Result<Connection,String>{Connection::open(path).map_err(|e|e.to_string())}
#[tauri::command]
fn listar_expedientes(filtro:String,state:tauri::State<'_,DbPath>)->Result<Vec<Expediente>,String>{
 let db=open(&state.0)?;
 let mut stmt=db.prepare("SELECT id,numero,descripcion,estado FROM expedientes WHERE numero LIKE ?1 ORDER BY numero LIMIT 300").map_err(|e|e.to_string())?;
 let rows=stmt.query_map(params![format!("%{}%",filtro)],|r|Ok(Expediente{id:r.get(0)?,numero:r.get(1)?,descripcion:r.get(2)?,estado:r.get(3)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn crear_expediente_local(numero:String,descripcion:String,state:tauri::State<'_,DbPath>)->Result<(),String>{
 if numero.trim().is_empty(){return Err("Número requerido".into());}
 let mut db=open(&state.0)?;
 let tx=db.transaction().map_err(|e|e.to_string())?;
 let id=uuid::Uuid::new_v4().to_string();
 tx.execute("INSERT INTO expedientes(id,numero,descripcion,estado) VALUES (?1,?2,?3,'PENDIENTE_SYNC')",params![id,numero,descripcion]).map_err(|e|e.to_string())?;
 tx.execute("INSERT INTO outbox(id,tipo,entidad_id,payload,estado) VALUES (?1,'CREAR_EXPEDIENTE',?2,?3,'PENDIENTE')",params![uuid::Uuid::new_v4().to_string(),id,format!("{}|{}",numero,descripcion)]).map_err(|e|e.to_string())?;
 tx.commit().map_err(|e|e.to_string())
}
#[cfg_attr(mobile,tauri::mobile_entry_point)]
pub fn run(){
 tauri::Builder::default().setup(|app|{
  let path=app.path().app_data_dir()?;std::fs::create_dir_all(&path)?;
  let dbpath=path.join("expedientes.sqlite");
  let db=Connection::open(&dbpath)?;
  db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS expedientes(id TEXT PRIMARY KEY,numero TEXT NOT NULL UNIQUE,descripcion TEXT NOT NULL,estado TEXT NOT NULL); CREATE TABLE IF NOT EXISTS outbox(id TEXT PRIMARY KEY,tipo TEXT NOT NULL,entidad_id TEXT NOT NULL,payload TEXT NOT NULL,estado TEXT NOT NULL);")?;
  app.manage(DbPath(dbpath));Ok(())
 }).invoke_handler(tauri::generate_handler![listar_expedientes,crear_expediente_local]).run(tauri::generate_context!()).expect("Tauri error");
}
