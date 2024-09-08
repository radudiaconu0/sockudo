use crate::log::Log;
use crate::websocket::WebSocket;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use web_socket::{CloseReason, Event, Frame};

pub struct Connection {
    pub socket_id: String,
    pub socket: Mutex<WebSocket>,
    pub subscribed_channels: Mutex<HashSet<String>>,
    pub user_id: Mutex<Option<String>>,
    pub user_data: Mutex<Option<Value>>,
    sender: mpsc::UnboundedSender<String>,
}

impl Connection {
    pub fn new(socket_id: String, socket: WebSocket) -> Arc<Self> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let connection = Arc::new(Self {
            socket_id,
            socket: Mutex::new(socket),
            subscribed_channels: Mutex::new(HashSet::new()),
            user_id: Mutex::new(None),
            user_data: Mutex::new(None),
            sender,
        });
        let conn_clone = Arc::clone(&connection);
        task::spawn(async move {
            while let Some(message) = receiver.recv().await {
                if let Err(e) = conn_clone.send_message_internal(message).await {
                    Log::error(format!("Failed to send message: {}", e));
                    // Optionally break the loop if you want to stop on first error
                }
            }
        });
        connection
    }

    pub async fn send_message(&self, message: String) {
        Log::info(format!(
            "Queueing message for {}: {}",
            self.socket_id, message
        ));
        if let Err(e) = self.sender.send(message) {
            Log::error(format!("Failed to queue message: {}", e));
        }
    }

    async fn send_message_internal(
        &self,
        message: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut socket = self.socket.lock().await;
        socket.send(message.as_str()).await?;
        Ok(())
    }

    pub async fn subscribe(&self, channel: String) {
        self.subscribed_channels.lock().await.insert(channel);
    }

    pub async fn unsubscribe(&self, channel: &str) {
        self.subscribed_channels.lock().await.remove(channel);
    }

    pub async fn set_user_id(&self, user_id: String) {
        let mut uid = self.user_id.lock().await;
        *uid = Some(user_id);
    }

    pub async fn set_user_data(&self, user_data: Value) {
        let mut ud = self.user_data.lock().await;
        *ud = Some(user_data);
    }

    pub async fn get_subscribed_channels(&self) -> HashSet<String> {
        self.subscribed_channels.lock().await.clone()
    }

    pub async fn close(&self, reason: &str) {
        self.socket
            .lock()
            .await
            .send_raw(Frame {
                fin: true,
                opcode: 8,
                data: reason.to_bytes().as_ref(),
            })
            .await
            .expect("TODO: panic message");
        self.socket
            .lock()
            .await
            .stream
            .flush()
            .await
            .expect("TODO: panic message");
    }

    pub async fn recv(&self) -> std::io::Result<Event> {
        self.socket.lock().await.recv().await
    }
}

pub type SafeConnection = Arc<Connection>;

pub struct ConnectionManager {
    connections: Mutex<HashMap<String, SafeConnection>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_connection(&self, connection: SafeConnection) {
        let mut connections = self.connections.lock().await;
        connections.insert(connection.socket_id.clone(), connection);
    }

    pub async fn remove_connection(&self, socket_id: &str) {
        let mut connections = self.connections.lock().await;
        connections.remove(socket_id);
    }

    pub async fn get_connection(&self, socket_id: &str) -> Option<SafeConnection> {
        let connections = self.connections.lock().await;
        connections.get(socket_id).cloned()
    }

    pub async fn get_connections(&self) -> Vec<SafeConnection> {
        let connections = self.connections.lock().await;
        connections.values().cloned().collect()
    }
}

pub type SafeConnectionManager = Arc<ConnectionManager>;

pub fn create_connection_manager() -> SafeConnectionManager {
    Arc::new(ConnectionManager::new())
}
