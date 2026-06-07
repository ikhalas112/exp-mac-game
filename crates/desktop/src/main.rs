use std::time::Duration;

use dioxus::prelude::*;
use serde::Deserialize;
use snake_shared::{
    BOARD_HEIGHT, BOARD_WIDTH, CellKind, Direction, GameState, GameStatus, GetOrCreateUserRequest,
    LeaderboardEntry, ScoresResponse, SubmitScoreRequest, User, UserResponse,
};

const STYLE: Asset = asset!("/assets/style.css");
const TICK_MS: u64 = 120;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// Auth-server response structs — field names match the server's snake_case serde output
// (no rename_all on RedeemResult / MeResult / LinkView — verified in services/launch.rs + identity.rs)

#[derive(Deserialize, Clone, Debug)]
struct RedeemResult {
    access_token: String,
    #[allow(dead_code)]
    refresh_token: String,
    player_id: String,
    account_id: String,
    #[allow(dead_code)]
    expires_in: i64,
}

#[derive(Deserialize, Clone, Debug)]
struct LinkView {
    provider: String,
    provider_account_id: String,
    #[allow(dead_code)]
    email: Option<String>,
    #[allow(dead_code)]
    display_name: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
struct MeResult {
    #[allow(dead_code)] // stored in auth_player_id signal; compiler can't trace reactive reads
    player_id: String,
    display_name: String,
    #[allow(dead_code)]
    display_name_required: bool,
    links: Vec<LinkView>,
}

fn main() {
    load_env();
    dioxus::launch(App);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppConfig {
    backend_url: String,
    player_name: String,
    launch_ticket: String,
    auth_api_url: String,
}

impl AppConfig {
    fn from_env() -> Self {
        Self {
            backend_url: env_or_default("BACKEND_URL", "http://127.0.0.1:3001"),
            player_name: env_or_default("PLAYER_NAME", "Player"),
            launch_ticket: env_or_default("LAUNCH_TICKET", ""),
            auth_api_url: env_or_default("AUTH_API_URL", "http://127.0.0.1:5000"),
        }
    }
}

#[component]
fn App() -> Element {
    let mut game = use_signal(GameState::default);
    let mut high_scores = use_signal(Vec::<LeaderboardEntry>::new);
    let mut sync_status = use_signal(|| "Enter your name to start.".to_string());
    let config = use_signal(AppConfig::from_env);
    let mut player_name = use_signal(|| config.read().player_name.clone());
    let mut current_user = use_signal(|| None::<User>);
    let mut is_syncing_user = use_signal(|| false);

    // Auth identity signals — populated from launch ticket redemption on startup
    let mut auth_player_id = use_signal(|| String::new());
    let mut auth_account_id = use_signal(|| String::new());
    let mut auth_maxtag = use_signal(|| String::new());
    let mut auth_links = use_signal(Vec::<LinkView>::new);
    let mut auth_status = use_signal(|| "".to_string()); // "" = not applicable; "Loading" = in flight; "Err: ..." = failed

    use_future(move || async move {
        let backend_url = config.read().backend_url.clone();
        match fetch_scores(&backend_url).await {
            Ok(scores) => {
                high_scores.set(scores);
                sync_status.set("Enter your name to start.".to_string());
            }
            Err(error) => sync_status.set(format!("Score sync offline: {error}")),
        }
    });

    // Redeem launch ticket on startup (if provided by the launcher)
    use_future(move || async move {
        let ticket = config.read().launch_ticket.clone();
        let auth_url = config.read().auth_api_url.clone();
        if ticket.is_empty() {
            // Standalone run — no identity panel, no panic
            return;
        }
        auth_status.set("Loading".to_string());
        match redeem_ticket(&auth_url, &ticket).await {
            Err(err) => {
                auth_status.set(format!("Err: {err}"));
            }
            Ok(redeem) => {
                auth_player_id.set(redeem.player_id.clone());
                auth_account_id.set(redeem.account_id.clone());
                match fetch_me(&auth_url, &redeem.access_token).await {
                    Err(err) => {
                        auth_status.set(format!("Err: {err}"));
                    }
                    Ok(me) => {
                        auth_maxtag.set(me.display_name.clone());
                        auth_links.set(me.links.clone());
                        auth_status.set("Ok".to_string());
                    }
                }
            }
        }
    });

    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(TICK_MS)).await;
            let previous_status = game.read().status.clone();
            game.write().tick();
            let current = game.read().clone();

            if previous_status == GameStatus::Running && current.status == GameStatus::GameOver {
                if let Some(user) = current_user.read().clone() {
                    let backend_url = config.read().backend_url.clone();
                    match submit_score(&backend_url, user.id, current.score).await {
                        Ok(scores) => {
                            high_scores.set(scores);
                            sync_status.set("Score submitted.".to_string());
                        }
                        Err(error) => sync_status.set(format!("Score submit failed: {error}")),
                    }
                }
            }
        }
    });

    let status_text = match game.read().status {
        GameStatus::Ready => "Ready",
        GameStatus::Running => "Running",
        GameStatus::GameOver => "Game over",
    };
    let score = game.read().score;
    let best_score = high_scores
        .read()
        .first()
        .map(|entry| entry.score)
        .unwrap_or(0);
    let cells = game.read().cells();
    let active_player = current_user
        .read()
        .as_ref()
        .map(|user| user.name.clone())
        .unwrap_or_else(|| "No player".to_string());
    let start_label = if is_syncing_user() {
        "Syncing..."
    } else if game.read().status == GameStatus::GameOver {
        "Restart"
    } else {
        "Start"
    };

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }
        main {
            class: "app",
            tabindex: "0",
            autofocus: "true",
            onkeydown: move |event| {
                let direction = match event.key() {
                    Key::ArrowUp => Some(Direction::Up),
                    Key::ArrowDown => Some(Direction::Down),
                    Key::ArrowLeft => Some(Direction::Left),
                    Key::ArrowRight => Some(Direction::Right),
                    Key::Character(value) if value.eq_ignore_ascii_case("w") => Some(Direction::Up),
                    Key::Character(value) if value.eq_ignore_ascii_case("s") => Some(Direction::Down),
                    Key::Character(value) if value.eq_ignore_ascii_case("a") => Some(Direction::Left),
                    Key::Character(value) if value.eq_ignore_ascii_case("d") => Some(Direction::Right),
                    Key::Character(value) if value == " " => {
                        if current_user.read().is_some() {
                            if game.read().status == GameStatus::GameOver {
                                game.write().restart();
                            } else {
                                game.write().start();
                            }
                        } else {
                            sync_status.set("Enter your name and press Start first.".to_string());
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(direction) = direction {
                    if current_user.read().is_some() {
                        game.write().set_direction(direction);
                        game.write().start();
                    } else {
                        sync_status.set("Enter your name and press Start first.".to_string());
                    }
                }
            },
            section { class: "hud",
                div {
                    h1 { "Snake" }
                    p { class: "status", "{status_text}" }
                }
                label { class: "player",
                    span { "Player" }
                    input {
                        value: "{player_name}",
                        disabled: game.read().status == GameStatus::Running || is_syncing_user(),
                        maxlength: "24",
                        oninput: move |event| player_name.set(event.value()),
                    }
                }
                div { class: "metrics",
                    div { class: "metric", span { "Active" } strong { "{active_player}" } }
                    div { class: "metric", span { "Score" } strong { "{score}" } }
                    div { class: "metric", span { "Best" } strong { "{best_score}" } }
                }

                // Auth identity panel — visible only when a launch ticket was provided
                {
                    let status = auth_status.read().clone();
                    if status == "Loading" {
                        rsx! {
                            div { class: "player-info player-info--loading",
                                span { "Loading identity…" }
                            }
                        }
                    } else if let Some(msg) = status.strip_prefix("Err: ") {
                        let msg = msg.to_string();
                        rsx! {
                            div { class: "player-info player-info--error",
                                span { "Identity error: {msg}" }
                            }
                        }
                    } else if status == "Ok" {
                        let pid = auth_player_id.read().clone();
                        let aid = auth_account_id.read().clone();
                        let tag = auth_maxtag.read().clone();
                        let links = auth_links.read().clone();
                        rsx! {
                            div { class: "player-info",
                                div { class: "metric", span { "Player ID" } strong { "{pid}" } }
                                div { class: "metric", span { "Account ID" } strong { "{aid}" } }
                                div { class: "metric", span { "Maxtag" } strong { "{tag}" } }
                                for link in links {
                                    div { class: "identity-link",
                                        span { class: "identity-link__provider", "{link.provider}" }
                                        span { class: "identity-link__account", "{link.provider_account_id}" }
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! {}
                    }
                }

                div { class: "actions",
                    button {
                        disabled: is_syncing_user(),
                        onclick: move |_| {
                            let backend_url = config.read().backend_url.clone();
                            let name = player_name.read().clone();
                            is_syncing_user.set(true);
                            sync_status.set("Checking player...".to_string());
                            spawn(async move {
                                match get_or_create_user(&backend_url, &name).await {
                                    Ok(user) => {
                                        let display_name = user.name.clone();
                                        current_user.set(Some(user));
                                        game.write().restart();
                                        sync_status.set(format!("Playing as {display_name}."));
                                    }
                                    Err(error) => sync_status.set(format!("Player sync failed: {error}")),
                                }
                                is_syncing_user.set(false);
                            });
                        },
                        "{start_label}"
                    }
                    button {
                        onclick: move |_| {
                            game.set(GameState::default());
                            current_user.set(None);
                            sync_status.set("Enter your name to start.".to_string());
                        },
                        "Reset"
                    }
                }
            }

            section { class: "game-layout",
                div { class: "playfield",
                    div {
                        class: "board",
                        style: "--cols: {BOARD_WIDTH}; --rows: {BOARD_HEIGHT};",
                        for cell in cells {
                            div {
                                key: "{cell.point.x}-{cell.point.y}",
                                class: cell_class(cell.kind),
                            }
                        }
                    }
                }
                aside { class: "leaderboard",
                    h2 { "Leaderboard" }
                    div { class: "leaderboard-list",
                        for (index, entry) in high_scores.read().iter().enumerate() {
                            div {
                                class: "leaderboard-row",
                                key: "{entry.user_id}-{entry.created_at}",
                                span { class: "rank", "#{index + 1}" }
                                span { class: "name", "{entry.player_name}" }
                                strong { "{entry.score}" }
                            }
                        }
                        if high_scores.read().is_empty() {
                            p { class: "empty", "No scores yet." }
                        }
                    }
                }
            }

            section { class: "footer",
                p { "{sync_status}" }
                p { "v{APP_VERSION} · Use arrow keys or WASD. Space starts or restarts." }
            }
        }
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
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn cell_class(kind: CellKind) -> &'static str {
    match kind {
        CellKind::Empty => "cell",
        CellKind::Head => "cell head",
        CellKind::Body => "cell body",
        CellKind::Food => "cell food",
    }
}

async fn fetch_scores(base_url: &str) -> anyhow::Result<Vec<LeaderboardEntry>> {
    let response = reqwest::get(format!("{base_url}/scores"))
        .await?
        .error_for_status()?
        .json::<ScoresResponse>()
        .await?;
    Ok(response.scores)
}

async fn get_or_create_user(base_url: &str, name: &str) -> anyhow::Result<User> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{base_url}/users"))
        .json(&GetOrCreateUserRequest {
            name: name.to_string(),
        })
        .send()
        .await?
        .error_for_status()?
        .json::<UserResponse>()
        .await?;
    Ok(response.user)
}

async fn submit_score(
    base_url: &str,
    user_id: uuid::Uuid,
    score: u32,
) -> anyhow::Result<Vec<LeaderboardEntry>> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{base_url}/scores"))
        .json(&SubmitScoreRequest { user_id, score })
        .send()
        .await?
        .error_for_status()?
        .json::<ScoresResponse>()
        .await?;
    Ok(response.scores)
}

/// POST {auth_url}/v1/launch/redemptions — redeem a one-time launch ticket for a game session
async fn redeem_ticket(auth_url: &str, ticket: &str) -> anyhow::Result<RedeemResult> {
    #[derive(serde::Serialize)]
    struct RedeemBody<'a> {
        ticket: &'a str,
    }
    let client = reqwest::Client::new();
    let result = client
        .post(format!("{auth_url}/v1/launch/redemptions"))
        .json(&RedeemBody { ticket })
        .send()
        .await?
        .error_for_status()?
        .json::<RedeemResult>()
        .await?;
    Ok(result)
}

/// GET {auth_url}/v1/me — fetch player identity using the game access token
async fn fetch_me(auth_url: &str, access_token: &str) -> anyhow::Result<MeResult> {
    let client = reqwest::Client::new();
    let result = client
        .get(format!("{auth_url}/v1/me"))
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await?
        .error_for_status()?
        .json::<MeResult>()
        .await?;
    Ok(result)
}
