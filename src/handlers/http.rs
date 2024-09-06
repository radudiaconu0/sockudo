use crate::channel::ChannelType;
use crate::error::AppError;
use crate::log::Log;
use crate::protocol::events::{PusherApiEvent};
use crate::server::AppState;
use axum::extract::Query;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct AuthRequest {
    socket_id: String,
    channel_name: String,
    #[serde(default)]
    channel_data: Option<String>,
}

#[derive(Serialize)]
pub struct AuthResponse {
    auth: String,
}

pub async fn auth(
    State(state): State<AppState>,
    Path(app_id): Path<String>,
    Json(payload): Json<AuthRequest>,
) -> Result<impl IntoResponse, AppError> {
    let app = state
        .application_manager
        .get_application(&app_id)
        .await
        .ok_or_else(|| AppError::NotFound("Application not found".into()))?;

    let channel_type = determine_channel_type(&payload.channel_name);

    match channel_type {
        ChannelType::Private | ChannelType::Presence => {
            // In a real implementation, you'd verify the user's credentials here
            let auth_signature = generate_auth_signature(
                &app.key,
                &app.secret,
                &payload.socket_id,
                &payload.channel_name,
                payload.channel_data.as_deref(),
            );
            Ok((
                StatusCode::OK,
                Json(AuthResponse {
                    auth: auth_signature,
                }),
            ))
        }
        ChannelType::Public => Err(AppError::BadRequest(
            "Public channels don't need authentication".into(),
        )),
    }
}

pub async fn channel_users(
    State(state): State<AppState>,
    Path((app_id, channel_name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let app = state
        .application_manager
        .get_application(&app_id)
        .await
        .ok_or_else(|| AppError::NotFound("Application not found".into()))?;

    let channel = app
        .channel_manager
        .get_channel(&channel_name)
        .await
        .unwrap()
        .unwrap();

    Ok((StatusCode::OK, Json(channel.subscribers().await)))
}

pub async fn channel_state(
    State(state): State<AppState>,
    Path((app_id, channel_name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let app = state
        .application_manager
        .get_application(&app_id)
        .await
        .ok_or_else(|| AppError::NotFound("Application not found".into()))?;

    let channel = app
        .channel_manager
        .get_channel(&channel_name)
        .await
        .unwrap()
        .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;

    let subscriber_count = channel.subscriber_count().await.unwrap();

    let state = serde_json::json!({
        "occupied": subscriber_count > 0,
        "subscription_count": subscriber_count,
    });

    Ok((StatusCode::OK, Json(state)))
}

#[derive(Deserialize, Serialize)]
pub struct EventQuery {
    auth_key: String,
    auth_timestamp: String,
    auth_version: String,
    body_md5: String,
    auth_signature: String,
}
pub async fn events(
    State(state): State<AppState>,
    Path(app_id): Path<String>,
    Query(query): Query<EventQuery>,
    Json(event): Json<PusherApiEvent>,
) -> Result<impl IntoResponse, AppError> {
    let app = state
        .application_manager
        .get_application(&app_id)
        .await
        .ok_or_else(|| AppError::NotFound("Application not found".into()))?;
    let message = serde_json::to_string(&event)?;
    Log::info(format!("Received event: {}", message));
    let channels = event.channels;

    Log::info(format!("Broadcasting event to channels: {:?}", channels));
    for channel_name in channels {
        let message = json!({
            "event": event.name,
            "data": event.data,
            "channel": channel_name,
        });
        Log::info(format!("Broadcasting event to channel: {}", channel_name));
        let channel = app
            .channel_manager
            .get_channel(&channel_name)
            .await
            .unwrap()
            .ok_or_else(|| AppError::NotFound("Channel not found".into()))?;

        channel.broadcast(message.to_string()).await.unwrap();
    }

    Log::info(format!("Event data: {:?}", event.data));

    Ok(StatusCode::OK)
}

fn determine_channel_type(channel_name: &str) -> ChannelType {
    if channel_name.starts_with("private-") {
        ChannelType::Private
    } else if channel_name.starts_with("presence-") {
        ChannelType::Presence
    } else {
        ChannelType::Public
    }
}

fn generate_auth_signature(
    app_key: &str,
    app_secret: &str,
    socket_id: &str,
    channel_name: &str,
    channel_data: Option<&str>,
) -> String {
    use hex;
    use sha2::{Digest, Sha256};

    let mut string_to_sign = format!("{}:{}:{}", socket_id, channel_name, app_secret);
    if let Some(data) = channel_data {
        string_to_sign.push(':');
        string_to_sign.push_str(data);
    }

    let mut hasher = Sha256::new();
    hasher.update(string_to_sign);
    let result = hasher.finalize();

    format!("{}:{}", app_key, hex::encode(result))
}
