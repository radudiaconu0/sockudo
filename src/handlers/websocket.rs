use crate::channel::{ChannelType, SafeChannelManager};
use crate::connection::{Connection, SafeConnection, SafeConnectionManager};

use crate::error::AppError;
use crate::log::Log;
use crate::protocol::events::{PusherApiEventResponse, PusherEvent};
use crate::protocol::messages::PusherMessage;
use crate::websocket::WebSocket;
use rand::Rng;
use serde_json::json;
use web_socket::Event;

pub async fn handle_socket(
    socket: WebSocket,
    channel_manager: &SafeChannelManager,
    connection_manager: &SafeConnectionManager,
) {
    let actual_connections = connection_manager.get_connections().await;
    Log::info("Existing connections:");
    for conn in actual_connections {
        Log::info(format!("Connection: {}", conn.socket_id));
    }
    // get connections from all channels
    let channels = channel_manager.get_channel("chat-room").await.unwrap();
    match channels {
        Some(channel) => {
            let subscribers = channel.subscribers().await;
            Log::info(format!("Subscribers: {:?}", subscribers));
        }
        None => {
            Log::info("No subscribers");
        }
    }
    Log::info("New WebSocket connection established");
    let socket_id = generate_socket_id();
    let connection = Connection::new(socket_id.clone(), socket);
    connection_manager.add_connection(connection.clone()).await;

    Log::info(format!("New connection established: {}", socket_id));

    // Send connection established message
    let conn_established = PusherMessage::ConnectionEstablished {
        socket_id: socket_id.clone(),
        activity_timeout: 120,
    };
    connection
        .send_message(serde_json::to_string(&conn_established).unwrap())
        .await;

    while let Ok(ev) = connection.recv().await {
        match ev {
            Event::Data { data, .. } => {
                let message = String::from_utf8(data.to_vec())
                    .map_err(|e| AppError::BadRequest(format!("Invalid message format: {}", e)))
                    .unwrap();
                handle_client_message(message, &connection, channel_manager)
                    .await
                    .expect("TODO: panic message");
            }
            Event::Ping(_) => {}
            Event::Pong(_) => {}
            Event::Error(_) => {
                Log::error("Error event received");
            }
            Event::Close { code, reason } => {
                // write the code and reason to the log
                break;
            }
        }
    }
    connection_manager.remove_connection(&socket_id).await;
    let subscribed_channels = { connection.get_subscribed_channels().await.clone() };
    for channel_name in subscribed_channels {
        if let Some(channel) = channel_manager.get_channel(&channel_name).await.unwrap()
        {
            channel.unsubscribe(&socket_id).await.unwrap();
        }
    }
    Log::websocket_title("❌ Connection closed:");
    Log::info(format!("Socket ID: {}", socket_id));
    connection.close("inchis").await;
}

async fn handle_client_message(
    message: String,
    connection: &SafeConnection,
    channel_manager: &SafeChannelManager,
) -> Result<(), AppError> {
    Log::info(format!("Received message: {:?}", message.clone()));
    let pusher_message: PusherMessage = serde_json::from_str(&message)
        .map_err(|e| AppError::BadRequest(format!("Invalid message format: {}", e)))?;

    match pusher_message {
        PusherMessage::Subscribe {
            channel,
            auth,
            channel_data,
        } => {
            handle_subscribe(channel, connection, channel_manager).await?;
        }
        PusherMessage::Unsubscribe { channel } => {
            connection.subscribed_channels.lock().await.remove(&channel);
            handle_unsubscribe(channel, connection, channel_manager).await?;
        }
        PusherMessage::Ping { data } => {
            connection
                .send_message(serde_json::to_string(&PusherMessage::Pong {
                    data: Some(json!({})),
                })?)
                .await;
        }
        PusherMessage::ClientEvent {
            channel,
            event,
            data,
        } => {
            handle_client_event(channel, event, data, connection, channel_manager).await?;
        }
        _ => {
            // Ignore other message types
        }
    }

    Ok(())
}

async fn handle_subscribe(
    channel_name: String,
    connection: &SafeConnection,
    channel_manager: &SafeChannelManager,
) -> Result<(), AppError> {
    let channel_type = determine_channel_type(&channel_name);
    let channel = channel_manager
        .create_channel(channel_name.clone(), channel_type)
        .await
        .unwrap();

    channel
        .subscribe(connection)
        .await
        .expect("TODO: panic message");
    connection.subscribe(channel_name.clone()).await;
    // For presence channels, you'd add presence data here

    let subscription_succeeded = PusherApiEventResponse {
        event: "pusher_internal:subscription_succeeded".to_string(),
        channel: channel_name,
        data: Some(json!({})),
    };
    connection
        .send_message(serde_json::to_string(&subscription_succeeded)?)
        .await;

    Ok(())
}

async fn handle_unsubscribe(
    channel_name: String,
    connection: &SafeConnection,
    channel_manager: &SafeChannelManager,
) -> Result<(), AppError> {
    if let Some(channel) = channel_manager.get_channel(&channel_name).await.unwrap() {
        channel.unsubscribe(&connection.socket_id).await.unwrap();
        connection.unsubscribe(&channel_name).await;
    }
    Ok(())
}

async fn handle_client_event(
    channel_name: String,
    event: String,
    data: serde_json::Value,
    connection: &SafeConnection,
    channel_manager: &SafeChannelManager,
) -> Result<(), AppError> {
    // Verify that client events are allowed for this channel
    if !channel_name.starts_with("private-") && !channel_name.starts_with("presence-") {
        return Err(AppError::BadRequest(
            "Client events are only allowed on private or presence channels".into(),
        ));
    }

    let channel = channel_manager.get_channel(&channel_name).await;

    match channel {
        Ok(channel) => match channel {
            Some(channel) => {
                let client_event = PusherEvent::ClientEvent {
                    channel: channel_name,
                    event,
                    data,
                };
                match channel
                    .broadcast(serde_json::to_string(&client_event)?)
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        return Err(AppError::InternalServerError(format!(
                            "Failed to broadcast event: {}",
                            e
                        )));
                    }
                }
            }
            None => {
                return Err(AppError::BadRequest("Channel not found".into()));
            }
        },
        Err(_) => {
            return Err(AppError::BadRequest("Error".into()));
        }
    }

    Ok(())
}

async fn send_message(socket: &mut WebSocket, message: PusherMessage) -> Result<(), AppError> {
    let message_str = serde_json::to_string(&message).map_err(|e| {
        AppError::InternalServerError(format!("Failed to serialize message: {}", e))
    })?;
    socket
        .send(message_str.as_bytes())
        .await
        .expect("TODO: panic message");
    Ok(())
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

fn generate_socket_id() -> String {
    let min: u64 = 0;
    let max: u64 = 10_000_000_000;

    let random_number = |min: u64, max: u64| -> u64 { rand::thread_rng().gen_range(min..=max) };

    format!("{}.{}", random_number(min, max), random_number(min, max))
}
