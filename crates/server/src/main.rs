use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use snake_shared::{
    GetOrCreateUserRequest, LeaderboardEntry, MAX_HIGH_SCORES, ScoresResponse, SubmitScoreRequest,
    User, UserResponse,
};
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DatabaseConfig {
    url: Option<String>,
    host: String,
    port: u16,
    user: String,
    password: String,
    name: String,
}

impl DatabaseConfig {
    fn from_env() -> Self {
        Self {
            url: env_optional("DATABASE_URL"),
            host: env_or_default("DATABASE_HOST", "127.0.0.1"),
            port: env_or_default("DATABASE_PORT", "5432")
                .parse()
                .unwrap_or(5432),
            user: env_first_or_default(&["DATABASE_USER", "POSTGRES_USER"], "snake"),
            password: env_first_or_default(&["DATABASE_PASSWORD", "POSTGRES_PASSWORD"], "snake"),
            name: env_first_or_default(&["DATABASE_NAME", "POSTGRES_DB"], "snake"),
        }
    }

    async fn connect(&self) -> anyhow::Result<PgPool> {
        let pool = if let Some(url) = &self.url {
            PgPoolOptions::new().max_connections(5).connect(url).await?
        } else {
            let options = PgConnectOptions::new()
                .host(&self.host)
                .port(self.port)
                .username(&self.user)
                .password(&self.password)
                .database(&self.name);
            PgPoolOptions::new()
                .max_connections(5)
                .connect_with(options)
                .await?
        };

        Ok(pool)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();

    let database_config = DatabaseConfig::from_env();
    let server_addr = env_or_default("SERVER_ADDR", "127.0.0.1:3001");
    let pool = database_config.connect().await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let app = app(AppState { pool });
    let listener = tokio::net::TcpListener::bind(&server_addr).await?;
    println!("snake-server listening on http://{server_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/users", post(get_or_create_user))
        .route("/scores", get(get_scores).post(post_score))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn get_or_create_user(
    State(state): State<AppState>,
    Json(payload): Json<GetOrCreateUserRequest>,
) -> ApiResult<UserResponse> {
    let name = clean_player_name(&payload.name);
    let user = sqlx::query(
        r#"
        INSERT INTO users (name)
        VALUES ($1)
        ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
        RETURNING id, name, created_at
        "#,
    )
    .bind(name)
    .fetch_one(&state.pool)
    .await
    .map(row_to_user)
    .map_err(internal_error)?;

    Ok(Json(UserResponse { user }))
}

async fn get_scores(State(state): State<AppState>) -> ApiResult<ScoresResponse> {
    Ok(Json(ScoresResponse {
        scores: fetch_leaderboard(&state.pool).await?,
    }))
}

async fn post_score(
    State(state): State<AppState>,
    Json(payload): Json<SubmitScoreRequest>,
) -> ApiResult<ScoresResponse> {
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1")
        .bind(payload.user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(internal_error)?;

    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "user not found".to_string()));
    }

    sqlx::query(
        r#"
        INSERT INTO scores (user_id, score)
        VALUES ($1, $2)
        ON CONFLICT (user_id) DO UPDATE
        SET score = EXCLUDED.score,
            created_at = NOW()
        WHERE scores.score < EXCLUDED.score
        "#,
    )
    .bind(payload.user_id)
    .bind(i64::from(payload.score))
    .execute(&state.pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(ScoresResponse {
        scores: fetch_leaderboard(&state.pool).await?,
    }))
}

async fn fetch_leaderboard(pool: &PgPool) -> Result<Vec<LeaderboardEntry>, (StatusCode, String)> {
    let rows = sqlx::query(
        r#"
        SELECT scores.user_id, users.name, scores.score, scores.created_at
        FROM scores
        JOIN users ON users.id = scores.user_id
        ORDER BY scores.score DESC, scores.created_at ASC
        LIMIT $1
        "#,
    )
    .bind(MAX_HIGH_SCORES as i64)
    .fetch_all(pool)
    .await
    .map_err(internal_error)?;

    Ok(rows
        .into_iter()
        .map(|row| LeaderboardEntry {
            user_id: row.get("user_id"),
            player_name: row.get("name"),
            score: row.get::<i64, _>("score").try_into().unwrap_or(u32::MAX),
            created_at: row.get("created_at"),
        })
        .collect())
}

fn row_to_user(row: sqlx::postgres::PgRow) -> User {
    User {
        id: row.get("id"),
        name: row.get("name"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
    }
}

fn clean_player_name(name: &str) -> String {
    let cleaned = name.trim();
    if cleaned.is_empty() {
        "Player".to_string()
    } else {
        cleaned.chars().take(24).collect()
    }
}

fn load_env() {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = std::path::Path::new(&manifest_dir).join(".env");
        dotenvy::from_path(path).ok();
    }
    dotenvy::dotenv().ok();
}

fn env_or_default(key: &str, default: &str) -> String {
    env_optional(key).unwrap_or_else(|| default.to_string())
}

fn env_first_or_default(keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| env_optional(key))
        .unwrap_or_else(|| default.to_string())
}

fn env_optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn internal_error(error: sqlx::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_blank_player_name() {
        assert_eq!(clean_player_name("  "), "Player");
    }

    #[test]
    fn trims_and_limits_player_name() {
        assert_eq!(
            clean_player_name("  1234567890123456789012345  "),
            "123456789012345678901234"
        );
    }
}
