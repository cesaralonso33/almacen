use axum::{extract::{Path, State}, http::StatusCode, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone)] struct App { db: PgPool }
type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;
fn err(code: StatusCode, msg: &str) -> (StatusCode, Json<Value>) { (code, Json(json!({"error":msg}))) }
fn db_err(e: sqlx::Error) -> (StatusCode, Json<Value>) { tracing_error(&e); err(StatusCode::INTERNAL_SERVER_ERROR, "Error de base de datos") }
fn tracing_error(e: &sqlx::Error) { eprintln!("database error: {e}"); }

#[derive(Deserialize)] struct CreatePedido { solicitante_id: Uuid, expedientes: Vec<Uuid> }
#[derive(Deserialize)] struct Actor { actor_id: Uuid }
#[derive(Deserialize)] struct Descarte { actor_id: Uuid, motivo: String }
#[derive(Deserialize)] struct Dispatch { actor_id: Uuid, item_ids: Vec<Uuid> }
#[derive(Deserialize)] struct Confirm { receptor_id: Uuid }
#[derive(Deserialize)] struct Return { actor_id: Uuid }
#[derive(Serialize)] struct ItemInfo { id: Uuid, estado: String }

async fn create_pedido(State(app): State<App>, Json(input): Json<CreatePedido>) -> ApiResult {
 if input.expedientes.is_empty() || input.expedientes.len() > 100 { return Err(err(StatusCode::BAD_REQUEST,"Debe solicitar entre 1 y 100 expedientes")); }
 let mut ids = input.expedientes.clone(); ids.sort(); ids.dedup();
 if ids.len()!=input.expedientes.len() { return Err(err(StatusCode::BAD_REQUEST,"Expedientes duplicados")); }
 let mut tx=app.db.begin().await.map_err(db_err)?;
 let id: Uuid=sqlx::query_scalar("INSERT INTO pedidos(solicitante_id) VALUES ($1) RETURNING id")
  .bind(input.solicitante_id).fetch_one(&mut *tx).await.map_err(db_err)?;
 for expediente in ids {
  sqlx::query("INSERT INTO pedido_items(pedido_id, expediente_id) VALUES ($1,$2)")
   .bind(id).bind(expediente).execute(&mut *tx).await.map_err(db_err)?;
 }
 tx.commit().await.map_err(db_err)?;
 Ok(Json(json!({"pedido_id":id})))
}

async fn get_pedido(State(app): State<App>, Path(id): Path<Uuid>) -> ApiResult {
 let rows: Vec<(Uuid,String)> = sqlx::query_as("SELECT id, estado FROM pedido_items WHERE pedido_id=$1 ORDER BY id")
  .bind(id).fetch_all(&app.db).await.map_err(db_err)?;
 let exists: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pedidos WHERE id=$1)").bind(id).fetch_one(&app.db).await.map_err(db_err)?;
 if !exists {return Err(err(StatusCode::NOT_FOUND,"Pedido no encontrado"));}
 let total=rows.len(); let entregados=rows.iter().filter(|r|r.1=="ENTREGADO").count();
 let descartados=rows.iter().filter(|r|r.1=="DESCARTADO").count();
 let estado=if entregados+descartados==total {"COMPLETADO"} else if entregados+descartados>0 {"PARCIAL"} else {"PENDIENTE"};
 let items:Vec<ItemInfo>=rows.into_iter().map(|r|ItemInfo{id:r.0,estado:r.1}).collect();
 Ok(Json(json!({"id":id,"estado":estado,"entregados":entregados,"descartados":descartados,"items":items})))
}

async fn discard(State(app): State<App>, Path(id): Path<Uuid>, Json(input): Json<Descarte>) -> ApiResult {
 if input.motivo.trim().len()<5 {return Err(err(StatusCode::BAD_REQUEST,"Motivo mínimo de 5 caracteres"));}
 let mut tx=app.db.begin().await.map_err(db_err)?;
 let item:Option<(Uuid,String)>=sqlx::query_as("SELECT expediente_id, estado FROM pedido_items WHERE id=$1 FOR UPDATE")
  .bind(id).fetch_optional(&mut *tx).await.map_err(db_err)?;
 let (exp,estado)=item.ok_or_else(||err(StatusCode::NOT_FOUND,"Item no encontrado"))?;
 if estado!="PENDIENTE" && estado!="PREPARADO" {return Err(err(StatusCode::CONFLICT,"Item no descartable"));}
 sqlx::query("UPDATE pedido_items SET estado='DESCARTADO',motivo_descarte=$2 WHERE id=$1")
  .bind(id).bind(input.motivo.trim()).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("INSERT INTO movimientos(expediente_id,tipo,actor_id,detalle) VALUES ($1,'DESCARTE_PEDIDO',$2,$3)")
  .bind(exp).bind(input.actor_id).bind(&input.motivo).execute(&mut *tx).await.map_err(db_err)?;
 tx.commit().await.map_err(db_err)?;
 Ok(Json(json!({"item_id":id,"estado":"DESCARTADO"})))
}

async fn prepare(State(app): State<App>, Path(id): Path<Uuid>, Json(input): Json<Actor>) -> ApiResult {
 let mut tx=app.db.begin().await.map_err(db_err)?;
 let item:Option<(Uuid,String)>=sqlx::query_as("SELECT expediente_id,estado FROM pedido_items WHERE id=$1 FOR UPDATE")
  .bind(id).fetch_optional(&mut *tx).await.map_err(db_err)?;
 let (exp,estado)=item.ok_or_else(||err(StatusCode::NOT_FOUND,"Item no encontrado"))?;
 if estado!="PENDIENTE" {return Err(err(StatusCode::CONFLICT,"Item no pendiente"));}
 let current:Option<String>=sqlx::query_scalar("SELECT estado FROM expedientes WHERE id=$1 FOR UPDATE")
  .bind(exp).fetch_optional(&mut *tx).await.map_err(db_err)?;
 if current.as_deref()!=Some("ALMACEN") {return Err(err(StatusCode::CONFLICT,"Expediente no disponible en almacén"));}
 let reserved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pedido_items WHERE expediente_id=$1 AND id<>$2 AND estado IN ('PREPARADO','EN_ENTREGA'))")
  .bind(exp).bind(id).fetch_one(&mut *tx).await.map_err(db_err)?;
 if reserved {return Err(err(StatusCode::CONFLICT,"Expediente reservado por otro pedido"));}
 sqlx::query("UPDATE pedido_items SET estado='PREPARADO' WHERE id=$1").bind(id).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("INSERT INTO movimientos(expediente_id,tipo,actor_id) VALUES ($1,'PREPARACION',$2)")
  .bind(exp).bind(input.actor_id).execute(&mut *tx).await.map_err(db_err)?;
 tx.commit().await.map_err(db_err)?;
 Ok(Json(json!({"item_id":id,"estado":"PREPARADO"})))
}

async fn dispatch(State(app): State<App>, Json(input): Json<Dispatch>) -> ApiResult {
 if input.item_ids.is_empty() || input.item_ids.len()>100 {return Err(err(StatusCode::BAD_REQUEST,"Seleccione entre 1 y 100 items"));}
 let mut ids=input.item_ids; ids.sort(); ids.dedup();
 let mut tx=app.db.begin().await.map_err(db_err)?;
 // Lock every referenced expediente in deterministic order to prevent competing dispatches.
 let exps:Vec<Uuid>=sqlx::query_scalar("SELECT expediente_id FROM pedido_items WHERE id=ANY($1) ORDER BY expediente_id")
  .bind(&ids).fetch_all(&mut *tx).await.map_err(db_err)?;
 if exps.len()!=ids.len() {return Err(err(StatusCode::NOT_FOUND,"Uno o más items no existen"));}
 let mut ordered=exps; ordered.sort(); ordered.dedup();
 for exp in ordered {
  sqlx::query("SELECT id FROM expedientes WHERE id=$1 FOR UPDATE").bind(exp).fetch_one(&mut *tx).await.map_err(db_err)?;
 }
 let mut entregas=Vec::new();
 for item_id in ids {
  let row:Option<(Uuid,Uuid,String,String)>=sqlx::query_as(
   "SELECT i.expediente_id,p.solicitante_id,i.estado,e.estado FROM pedido_items i JOIN pedidos p ON p.id=i.pedido_id JOIN expedientes e ON e.id=i.expediente_id WHERE i.id=$1 FOR UPDATE OF i")
   .bind(item_id).fetch_optional(&mut *tx).await.map_err(db_err)?;
  let (exp,receptor,estado,estado_exp)=row.ok_or_else(||err(StatusCode::NOT_FOUND,"Item no encontrado"))?;
  if estado!="PREPARADO" || estado_exp!="ALMACEN" {return Err(err(StatusCode::CONFLICT,"Expediente no preparado o no disponible"));}
  let entrega:Uuid=sqlx::query_scalar("INSERT INTO entregas(pedido_item_id,entregador_id,receptor_id) VALUES($1,$2,$3) RETURNING id")
   .bind(item_id).bind(input.actor_id).bind(receptor).fetch_one(&mut *tx).await.map_err(db_err)?;
  sqlx::query("UPDATE pedido_items SET estado='EN_ENTREGA' WHERE id=$1").bind(item_id).execute(&mut *tx).await.map_err(db_err)?;
  sqlx::query("INSERT INTO movimientos(expediente_id,tipo,actor_id,destino_id) VALUES($1,'DESPACHO',$2,$3)")
   .bind(exp).bind(input.actor_id).bind(receptor).execute(&mut *tx).await.map_err(db_err)?;
  entregas.push(entrega);
 }
 tx.commit().await.map_err(db_err)?;
 Ok(Json(json!({"entregas":entregas,"estado":"PENDIENTE_RECEPCION"})))
}

async fn confirm(State(app): State<App>, Path(id): Path<Uuid>, Json(input): Json<Confirm>) -> ApiResult {
 let mut tx=app.db.begin().await.map_err(db_err)?;
 let exp:Option<Uuid>=sqlx::query_scalar("SELECT i.expediente_id FROM entregas t JOIN pedido_items i ON i.id=t.pedido_item_id WHERE t.id=$1")
  .bind(id).fetch_optional(&mut *tx).await.map_err(db_err)?;
 let exp=exp.ok_or_else(||err(StatusCode::NOT_FOUND,"Entrega no encontrada"))?;
 sqlx::query("SELECT id FROM expedientes WHERE id=$1 FOR UPDATE").bind(exp).fetch_one(&mut *tx).await.map_err(db_err)?;
 let row:Option<(Uuid,Uuid,String,String)>=sqlx::query_as(
  "SELECT t.pedido_item_id,t.receptor_id,t.estado,i.estado FROM entregas t JOIN pedido_items i ON i.id=t.pedido_item_id WHERE t.id=$1 FOR UPDATE OF t,i")
  .bind(id).fetch_optional(&mut *tx).await.map_err(db_err)?;
 let (item,receptor,estado,item_estado)=row.ok_or_else(||err(StatusCode::NOT_FOUND,"Entrega no encontrada"))?;
 if receptor!=input.receptor_id {return Err(err(StatusCode::FORBIDDEN,"Solo el receptor confirma"));}
 if estado!="PENDIENTE_RECEPCION" || item_estado!="EN_ENTREGA" {return Err(err(StatusCode::CONFLICT,"Entrega ya procesada"));}
 let available:String=sqlx::query_scalar("SELECT estado FROM expedientes WHERE id=$1").bind(exp).fetch_one(&mut *tx).await.map_err(db_err)?;
 if available!="ALMACEN" {return Err(err(StatusCode::CONFLICT,"Custodia incompatible"));}
 sqlx::query("INSERT INTO custodias(expediente_id,usuario_id,entrega_id) VALUES($1,$2,$3)")
  .bind(exp).bind(receptor).bind(id).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("UPDATE expedientes SET estado='EN_CUSTODIA',custodio_id=$2,version=version+1 WHERE id=$1")
  .bind(exp).bind(receptor).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("UPDATE entregas SET estado='CONFIRMADA',confirmada_en=now() WHERE id=$1").bind(id).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("UPDATE pedido_items SET estado='ENTREGADO' WHERE id=$1").bind(item).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("INSERT INTO movimientos(expediente_id,tipo,actor_id,destino_id) VALUES($1,'RECEPCION',$2,$2)")
  .bind(exp).bind(receptor).execute(&mut *tx).await.map_err(db_err)?;
 tx.commit().await.map_err(db_err)?;
 Ok(Json(json!({"entrega_id":id,"estado":"CONFIRMADA","custodio_id":receptor})))
}

async fn devolver(State(app): State<App>, Path(exp): Path<Uuid>, Json(input): Json<Return>) -> ApiResult {
 let mut tx=app.db.begin().await.map_err(db_err)?;
 let current:Option<(String,Option<Uuid>)>=sqlx::query_as("SELECT estado,custodio_id FROM expedientes WHERE id=$1 FOR UPDATE")
  .bind(exp).fetch_optional(&mut *tx).await.map_err(db_err)?;
 let (estado,custodio)=current.ok_or_else(||err(StatusCode::NOT_FOUND,"Expediente no encontrado"))?;
 if estado!="EN_CUSTODIA" || custodio!=Some(input.actor_id) {return Err(err(StatusCode::CONFLICT,"El usuario no es custodio actual"));}
 sqlx::query("UPDATE custodias SET fin=now() WHERE expediente_id=$1 AND fin IS NULL")
  .bind(exp).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("UPDATE expedientes SET estado='ALMACEN',custodio_id=NULL,version=version+1 WHERE id=$1")
  .bind(exp).execute(&mut *tx).await.map_err(db_err)?;
 sqlx::query("INSERT INTO movimientos(expediente_id,tipo,actor_id) VALUES($1,'DEVOLUCION',$2)")
  .bind(exp).bind(input.actor_id).execute(&mut *tx).await.map_err(db_err)?;
 tx.commit().await.map_err(db_err)?;
 Ok(Json(json!({"expediente_id":exp,"estado":"ALMACEN"})))
}

#[tokio::main] async fn main() -> anyhow::Result<()> {
 tracing_subscriber::fmt::init();
 let url=std::env::var("DATABASE_URL")?;
 let db=PgPool::connect(&url).await?;
 sqlx::migrate!("./migrations").run(&db).await?;
 let app=Router::new()
  .route("/health",get(||async {Json(json!({"ok":true}))}))
  .route("/api/v1/pedidos",post(create_pedido))
  .route("/api/v1/pedidos/{id}",get(get_pedido))
  .route("/api/v1/items/{id}/descartar",post(discard))
  .route("/api/v1/items/{id}/preparar",post(prepare))
  .route("/api/v1/entregas",post(dispatch))
  .route("/api/v1/entregas/{id}/confirmar",post(confirm))
  .route("/api/v1/expedientes/{id}/devolver",post(devolver))
  .with_state(App{db});
 let listener=tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
 axum::serve(listener,app).await?;
 Ok(())
}
