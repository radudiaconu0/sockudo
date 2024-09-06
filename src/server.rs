use crate::application::{create_application_manager, SafeApplicationManager};
use crate::error::AppError;
use crate::handlers::http::events;
use crate::handlers::{
    http::{auth, channel_state, channel_users},
    websocket::handle_socket,
};
use crate::log::Log;
use crate::websocket::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct AppState {
    pub application_manager: SafeApplicationManager,
}

pub async fn run_server() -> Result<(), AppError> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create application manager
    let application_manager = create_application_manager();

    // Create app state
    let app_state = AppState {
        application_manager,
    };

    // Build our application with routes
    let app = Router::new()
        .route("/app/:app_id", get(ws_handler))
        .route("/apps/:app_id/auth", post(auth))
        .route(
            "/apps/:app_id/channels/:channel_name/users",
            get(channel_users),
        )
        .route("/apps/:app_id/channels/:channel_name", get(channel_state))
        .route("/apps/:app_id/events", post(events))
        .with_state(app_state);

    // Run it
    let addr = SocketAddr::from(([0, 0, 0, 0], 6001));
    tracing::info!("listening on {}", addr);
    Log::info("Server started on port 6001");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:6001")
        .await?;
    match axum::serve(listener, app).await {
        Ok(_) => Ok(()),
        Err(e) => {
            Log::error(format!("Error running server: {}", e));
            Err(AppError::InternalServerError("Error running server".into()))
        }
    }
}

#[derive(Debug, serde::Deserialize, Serialize)]
struct PusherQuery {
    protocol: String,
    client: String,
    version: String,
    flash: String,
}

async fn ws_handler(
    Path(app_id): Path<String>,
    State(state): State<AppState>,
    Query(pusher): Query<PusherQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    Log::info(format!(
        "New WebSocket connection request for app: {}",
        app_id
    ));
    Log::success(format!("Pusher query: {:?}", pusher));

    match state.application_manager.get_application(&app_id).await {
        Some(app) => {
            let channel_manager = app.channel_manager.clone();
            let connection_manager = app.connection_manager.clone();

            ws.on_upgrade(move |socket| async move {
                handle_socket(
                    socket,
                    &channel_manager,
                    &connection_manager, // Clone the query params to use in handle_socket if needed
                )
                .await;
            })
        }
        None => {
            Log::error(format!("Application not found: {}", app_id));
            (StatusCode::NOT_FOUND, "Application not found").into_response()
        }
    }
}

pub async fn start_server() -> Result<(), AppError> {
    // You might want to perform any necessary setup here
    // For example, loading applications from a database

    run_server().await
}
