use axum::extract::{Path as AxumPath, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use operations_core::models::{Position, RulePack};
use operations_core::{CreateOptions, OperationEnvironment, discover_data_root, load_rule_pack};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": self.message,
            })),
        )
            .into_response()
    }
}

struct Session {
    seed: u64,
    environment: OperationEnvironment,
    events: Vec<Value>,
}

struct AppState {
    pack: Arc<RulePack>,
    data_root: PathBuf,
    static_root: PathBuf,
    sessions: Mutex<HashMap<String, Mutex<Session>>>,
    sequence: AtomicU64,
}

type SharedState = Arc<AppState>;
type ApiResult<T> = Result<T, ApiError>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut host = "127.0.0.1".to_owned();
    let mut port: u16 = 8765;
    let mut open_browser = true;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--host" => host = args.next().ok_or("--host 缺少参数")?,
            "--port" => port = args.next().ok_or("--port 缺少参数")?.parse()?,
            "--no-open" => open_browser = false,
            value => return Err(format!("未知参数：{value}").into()),
        }
    }

    let data_root = discover_data_root()?;
    let pack = Arc::new(load_rule_pack(&data_root)?);
    let static_root = static_root()?;
    let state = Arc::new(AppState {
        pack: Arc::clone(&pack),
        data_root: data_root.clone(),
        static_root,
        sessions: Mutex::new(HashMap::new()),
        sequence: AtomicU64::new(1),
    });
    let app = Router::new()
        .route("/", get(index))
        .route("/static/{*path}", get(static_file))
        .route("/assets/{*path}", get(catalog_asset))
        .route("/api/meta", get(meta))
        .route("/api/sessions", post(create_session))
        .route("/api/sessions/{id}", get(get_session))
        .route("/api/sessions/{id}/actions", post(apply_action))
        .with_state(state);

    let address: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = TcpListener::bind(address).await?;
    let url = format!("http://{host}:{port}/");
    println!("Rust 运营模拟网页已启动：{url}");
    println!(
        "数据快照：{}，角色 {}，羁绊 {}，词缀 {}",
        pack.snapshot_id,
        pack.units.len(),
        pack.traits.len(),
        pack.affixes.len()
    );
    if open_browser {
        open_url(&url);
    }
    axum::serve(listener, app).await?;
    Ok(())
}

fn static_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(value) = env::var("HSR_OPERATIONS_WEB_ROOT") {
        return Ok(PathBuf::from(value).canonicalize()?);
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web");
    Ok(path.canonicalize()?)
}

fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
}

async fn index(State(state): State<SharedState>) -> ApiResult<Response> {
    serve_file(&state.static_root, "index.html")
}

async fn static_file(
    State(state): State<SharedState>,
    AxumPath(path): AxumPath<String>,
) -> ApiResult<Response> {
    serve_file(&state.static_root, &path)
}

async fn catalog_asset(
    State(state): State<SharedState>,
    AxumPath(path): AxumPath<String>,
) -> ApiResult<Response> {
    serve_file(&state.data_root.join("assets"), &path)
}

fn serve_file(root: &Path, relative: &str) -> ApiResult<Response> {
    let relative = relative.trim_start_matches(['/', '\\']);
    let root = root
        .canonicalize()
        .map_err(|_| ApiError::not_found("文件不存在"))?;
    let path = root.join(relative);
    let canonical = path
        .canonicalize()
        .map_err(|_| ApiError::not_found("文件不存在"))?;
    if !canonical.starts_with(root) {
        return Err(ApiError::not_found("文件不存在"));
    }
    let bytes = fs::read(&canonical).map_err(|error| ApiError::internal(error.to_string()))?;
    let content_type = match canonical.extension().and_then(|value| value.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    };
    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}

async fn meta(State(state): State<SharedState>) -> Json<Value> {
    let pack = &state.pack;
    Json(json!({
        "game_version": pack.game_version,
        "snapshot_id": pack.snapshot_id,
        "content_sha256": pack.content_sha256,
        "validation": pack.validation_status,
        "character_count": pack.units.len(),
        "trait_count": pack.traits.len(),
        "assumptions": pack.assumptions,
        "difficulties": pack.difficulties.iter().map(|difficulty| json!({
            "id": difficulty.id,
            "name": difficulty.name,
            "enemy_difficulty": difficulty.enemy_difficulty,
            "affix_count": difficulty.affix_count,
        })).collect::<Vec<_>>(),
        "difficulty_limits": {
            "enemy_min": 1,
            "enemy_max": pack.rules.enemy_difficulty_max,
            "affix_min": 0,
            "affix_max": pack.affixes.len().min(pack.rules.affix_count_max),
        },
        "environments": pack.environments.iter().map(|environment| json!({
            "id": environment.id,
            "name": environment.name,
            "description": environment.description,
        })).collect::<Vec<_>>(),
    }))
}

async fn create_session(
    State(state): State<SharedState>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let difficulty_id = optional_string(&payload, "difficulty_id");
    let enemy_difficulty = optional_i32(&payload, "enemy_difficulty")?;
    let affix_count = optional_i32(&payload, "affix_count")?;
    let environment_id = optional_string(&payload, "environment_id");
    let seed = optional_u64(&payload, "seed")?.unwrap_or(7);
    let battle_progress = optional_i32(&payload, "battle_progress")?.unwrap_or(100);
    let environment = OperationEnvironment::create(
        (*state.pack).clone(),
        CreateOptions {
            difficulty_id,
            enemy_difficulty,
            affix_count,
            environment_id,
            seed,
            battle_progress,
        },
    )
    .map_err(ApiError::bad_request)?;
    let id = session_id(&state);
    let session = Session {
        seed,
        events: vec![json!({
            "sequence": 1,
            "kind": "start",
            "message": format!("创建 Rust 对局：敌人难度 {}，词缀 {}，种子 {}", environment.enemy_difficulty, environment.affix_count, seed),
        })],
        environment,
    };
    let response = session
        .environment
        .state_json(&id, session.seed, &session.events);
    state
        .sessions
        .lock()
        .map_err(|_| ApiError::internal("会话锁失败"))?
        .insert(id, Mutex::new(session));
    Ok(Json(response))
}

async fn get_session(
    State(state): State<SharedState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<Value>> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| ApiError::internal("会话锁失败"))?;
    let session = sessions
        .get(&id)
        .ok_or_else(|| ApiError::not_found("对局不存在"))?
        .lock()
        .map_err(|_| ApiError::internal("对局锁失败"))?;
    Ok(Json(session.environment.state_json(
        &id,
        session.seed,
        &session.events,
    )))
}

async fn apply_action(
    State(state): State<SharedState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<Value>,
) -> ApiResult<Json<Value>> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| ApiError::internal("会话锁失败"))?;
    let mut session = sessions
        .get(&id)
        .ok_or_else(|| ApiError::not_found("对局不存在"))?
        .lock()
        .map_err(|_| ApiError::internal("对局锁失败"))?;
    let action_type = required_string(&payload, "type")?;
    let event = match action_type.as_str() {
        "set_battle_progress" => {
            let progress = required_i32(&payload, "progress")?;
            session
                .environment
                .set_battle_progress(progress)
                .map_err(ApiError::bad_request)?;
            json!({"kind":"battle_progress","message":format!("战斗进度设为 {progress}%")})
        }
        "lock_shop" => {
            let locked = payload
                .get("locked")
                .and_then(Value::as_bool)
                .ok_or_else(|| ApiError::bad_request("locked 必须是布尔值"))?;
            session.environment.set_shop_locked(locked);
            json!({"kind":"shop_lock","message":if locked {"锁定商店"} else {"解除商店锁定"}})
        }
        "reroll_shop" => {
            let paid = session
                .environment
                .reroll_shop()
                .map_err(ApiError::bad_request)?;
            json!({"kind":"reroll","message":if paid {"刷新商店，消耗 2 金币"} else {"刷新商店，使用免费刷新"}})
        }
        "buy_offer" => {
            let index = required_u64(&payload, "index")? as usize;
            let unit_id = session
                .environment
                .market
                .offers
                .get(index)
                .and_then(Clone::clone)
                .ok_or_else(|| ApiError::bad_request("商店位置没有角色"))?;
            let name = session
                .environment
                .pack
                .units
                .get(&unit_id)
                .map(|unit| unit.name.clone())
                .unwrap_or_default();
            let merges = session
                .environment
                .buy_shop_offer(index)
                .map_err(ApiError::bad_request)?;
            json!({"kind":"purchase","message":format!("购买 {name}{}", if merges.is_empty() { String::new() } else { format!("，合成 {}", merges.join(",")) })})
        }
        "sell_unit" => {
            let unit_id = required_string(&payload, "unit_id")?;
            let star = required_i32(&payload, "star")?;
            let name = session
                .environment
                .pack
                .units
                .get(&unit_id)
                .map(|unit| unit.name.clone())
                .unwrap_or_default();
            let gold = session
                .environment
                .sell_unit(&unit_id, star)
                .map_err(ApiError::bad_request)?;
            json!({"kind":"sell","message":format!("出售 {star} 星 {name}，获得 {gold} 金币")})
        }
        "buy_xp" => {
            let count = optional_i32(&payload, "count")?.unwrap_or(1);
            let levels = session
                .environment
                .buy_xp(count)
                .map_err(ApiError::bad_request)?;
            json!({"kind":"xp","message":format!("购买 {count} 次经验，当前 {} 级{}", session.environment.market.level, if levels.is_empty() { String::new() } else { format!("，升至 {}", levels.iter().map(u8::to_string).collect::<Vec<_>>().join(",")) })})
        }
        "choose_investment" => {
            let index = required_u64(&payload, "index")? as usize;
            let name = session
                .environment
                .investment_offers
                .get(index)
                .map(|investment| investment.name.clone())
                .ok_or_else(|| ApiError::bad_request("投资策略位置不存在"))?;
            session
                .environment
                .choose_investment(index)
                .map_err(ApiError::bad_request)?;
            json!({"kind":"investment","message":format!("选择投资策略：{name}")})
        }
        "advance_node" => {
            let result = session
                .environment
                .advance_node()
                .map_err(ApiError::bad_request)?;
            json!({
                "kind": "node",
                "message": format!(
                    "节点 {} {} {}，战斗进度 {}%，生命 {:+}，金币 {:+}",
                    result.node_index + 1,
                    result.node_type,
                    if result.won { "胜利" } else { "失败" },
                    result.progress,
                    result.health_delta,
                    result.gold_delta,
                ),
            })
        }
        "set_unit_position" => {
            let unit_id = required_string(&payload, "unit_id")?;
            let star = required_i32(&payload, "star")?;
            let position = match payload.get("position") {
                None | Some(Value::Null) => None,
                Some(value) => {
                    let value = value
                        .as_str()
                        .ok_or_else(|| ApiError::bad_request("position 必须是字符串"))?;
                    Some(Position::parse(value).map_err(ApiError::bad_request)?)
                }
            };
            session
                .environment
                .set_unit_position(&unit_id, star, position)
                .map_err(ApiError::bad_request)?;
            json!({"kind":"deployment","message":format!("调整 {unit_id} 的位置")})
        }
        "swap_units" => {
            let source = payload
                .get("source")
                .ok_or_else(|| ApiError::bad_request("缺少 source"))?;
            let target = payload
                .get("target")
                .ok_or_else(|| ApiError::bad_request("缺少 target"))?;
            session
                .environment
                .swap_unit_slots(
                    &required_string(source, "location")?,
                    required_u64(source, "index")? as usize,
                    &required_string(target, "location")?,
                    required_u64(target, "index")? as usize,
                )
                .map_err(ApiError::bad_request)?;
            json!({"kind":"deployment","message":"交换角色格位"})
        }
        other => return Err(ApiError::bad_request(format!("未知操作：{other}"))),
    };
    let sequence = session.events.len() + 1;
    session.events.push(json!({
        "sequence": sequence,
        "kind": event["kind"],
        "message": event["message"],
    }));
    Ok(Json(session.environment.state_json(
        &id,
        session.seed,
        &session.events,
    )))
}

fn session_id(state: &AppState) -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = state.sequence.fetch_add(1, Ordering::Relaxed);
    format!("{time:x}{sequence:x}")
}

fn required_string(value: &Value, key: &str) -> ApiResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ApiError::bad_request(format!("{key} 必须是非空字符串")))
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn required_i32(value: &Value, key: &str) -> ApiResult<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value as i32)
        .ok_or_else(|| ApiError::bad_request(format!("{key} 必须是整数")))
}

fn optional_i32(value: &Value, key: &str) -> ApiResult<Option<i32>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => Ok(Some(required_i32(value, key)?)),
    }
}

fn required_u64(value: &Value, key: &str) -> ApiResult<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ApiError::bad_request(format!("{key} 必须是非负整数")))
}

fn optional_u64(value: &Value, key: &str) -> ApiResult<Option<u64>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => Ok(Some(required_u64(value, key)?)),
    }
}
