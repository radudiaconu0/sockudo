use super::{Channel, ChannelError, ChannelManager, ChannelType, PresenceChannel, PresenceUser};
use crate::connection::SafeConnection;
use crate::log::Log;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

struct PublicChannel {
    name: String,
    subscribers: RwLock<HashMap<String, SafeConnection>>,
}

struct PrivateChannel {
    name: String,
    subscribers: RwLock<HashMap<String, SafeConnection>>,
}

struct PresenceChannelImpl {
    name: String,
    subscribers: RwLock<HashMap<String, (SafeConnection, PresenceUser)>>,
}

#[async_trait]
impl Channel for PublicChannel {
    fn name(&self) -> &str {
        &self.name
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Public
    }

    async fn subscribers(&self) -> Vec<String> {
        let subscribers = self.subscribers.read().await;
        subscribers.keys().cloned().collect()
    }

    async fn subscribe(&self, connection: &SafeConnection) -> Result<(), ChannelError> {
        let mut subscribers = self.subscribers.write().await;
        subscribers.insert(connection.socket_id.clone(), Arc::clone(connection));
        Log::info(format!(
            "Subscribed {} to channel {}",
            connection.socket_id, self.name
        ));
        Ok(())
    }

    async fn unsubscribe(&self, socket_id: &str) -> Result<(), ChannelError> {
        self.subscribers.write().await.remove(socket_id);
        Ok(())
    }

    async fn broadcast(&self, message: String) -> Result<(), ChannelError> {
        let subscribers = self.subscribers.write().await;
        Log::info(format!(
            "Broadcasting message to {} subscribers: {}",
            subscribers.len(),
            message
        ));
        let now = chrono::Utc::now();
        let cloned_message = message.clone();

        for connection in subscribers.keys() {
            Log::info(format!("Subscriber id {}", connection));
        }

        tokio::join!(async {
            for connection in subscribers.values() {
                connection.send_message(cloned_message.clone()).await;
            }
        },);
        let elapsed = chrono::Utc::now().signed_duration_since(now);
        Log::info(format!("Broadcast completed in {:?}", elapsed));
        Ok(())
    }

    async fn send_to_connection(
        &self,
        socket_id: &str,
        message: String,
    ) -> Result<(), ChannelError> {
        let subscribers = self.subscribers.read().await;
        if let Some(connection) = subscribers.get(socket_id) {
            connection.send_message(message).await;
            Ok(())
        } else {
            Err(ChannelError::InternalError(
                "Connection not found".to_string(),
            ))
        }
    }

    async fn subscriber_count(&self) -> Result<usize, ChannelError> {
        let subscribers = self.subscribers.read().await;
        Ok(subscribers.len())
    }
}

#[async_trait]
impl Channel for PrivateChannel {
    fn name(&self) -> &str {
        todo!()
    }

    fn channel_type(&self) -> ChannelType {
        todo!()
    }

    async fn subscribers(&self) -> Vec<String> {
        todo!()
    }

    async fn subscribe(&self, connection: &SafeConnection) -> Result<(), ChannelError> {
        todo!()
    }

    async fn unsubscribe(&self, socket_id: &str) -> Result<(), ChannelError> {
        todo!()
    }

    async fn broadcast(&self, message: String) -> Result<(), ChannelError> {
        todo!()
    }

    async fn send_to_connection(
        &self,
        socket_id: &str,
        message: String,
    ) -> Result<(), ChannelError> {
        todo!()
    }

    async fn subscriber_count(&self) -> Result<usize, ChannelError> {
        todo!()
    }
    // Implementation is identical to PublicChannel
    // ...
}

#[async_trait]
impl Channel for PresenceChannelImpl {
    fn name(&self) -> &str {
        &self.name
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Presence
    }

    async fn subscribers(&self) -> Vec<String> {
        todo!()
    }
    async fn subscribe(&self, connection: &SafeConnection) -> Result<(), ChannelError> {
        // This should be called after add_presence_user
        Ok(())
    }

    async fn unsubscribe(&self, socket_id: &str) -> Result<(), ChannelError> {
        let mut subscribers = self.subscribers.write().await;
        subscribers.remove(socket_id);
        Ok(())
    }

    async fn broadcast(&self, message: String) -> Result<(), ChannelError> {
        let subscribers = self.subscribers.read().await;
        for (connection, _) in subscribers.values() {
            connection.send_message(message.clone()).await;
        }
        Ok(())
    }

    async fn send_to_connection(
        &self,
        socket_id: &str,
        message: String,
    ) -> Result<(), ChannelError> {
        let subscribers = self.subscribers.read().await;

        if let Some((connection, _)) = subscribers.get(socket_id) {
            connection.send_message(message).await;
            Ok(())
        } else {
            Err(ChannelError::InternalError(
                "Connection not found".to_string(),
            ))
        }
    }

    async fn subscriber_count(&self) -> Result<usize, ChannelError> {
        let subscribers = self.subscribers.read().await;
        Ok(subscribers.len())
    }
}

#[async_trait]
impl PresenceChannel for PresenceChannelImpl {
    async fn add_presence_user(
        &self,
        connection: SafeConnection,
        user: PresenceUser,
    ) -> Result<(), ChannelError> {
        let mut subscribers = self.subscribers.write().await;
        subscribers.insert(connection.socket_id.clone(), (connection, user));
        Ok(())
    }

    async fn remove_presence_user(&self, socket_id: &str) -> Result<(), ChannelError> {
        let mut subscribers = self.subscribers.write().await;
        subscribers.remove(socket_id);
        Ok(())
    }

    async fn get_presence_users(&self) -> Result<Vec<PresenceUser>, ChannelError> {
        let subscribers = self.subscribers.read().await;
        Ok(subscribers.values().map(|(_, user)| user.clone()).collect())
    }
}

pub struct MemoryChannelManager {
    channels: RwLock<HashMap<String, Arc<dyn Channel>>>,
}

impl MemoryChannelManager {
    pub fn new() -> Self {
        MemoryChannelManager {
            channels: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ChannelManager for MemoryChannelManager {
    async fn create_channel(
        &self,
        name: String,
        channel_type: ChannelType,
    ) -> Result<Arc<dyn Channel>, ChannelError> {
        let mut channels = self.channels.write().await;
        if channels.contains_key(&name) {
            return Ok(channels.get(&name).unwrap().clone());
        }
        let channel: Arc<dyn Channel> = match channel_type {
            ChannelType::Public => Arc::new(PublicChannel {
                name: name.clone(),
                subscribers: RwLock::new(HashMap::new()),
            }),
            ChannelType::Private => Arc::new(PrivateChannel {
                name: name.clone(),
                subscribers: RwLock::new(HashMap::new()),
            }),
            ChannelType::Presence => Arc::new(PresenceChannelImpl {
                name: name.clone(),
                subscribers: RwLock::new(HashMap::new()),
            }),
        };

        channels.insert(name.clone(), channel.clone());
        Ok(channel)
    }

    async fn get_channel(&self, name: &str) -> Result<Option<Arc<dyn Channel>>, ChannelError> {
        let channels = self.channels.read().await;
        Ok(channels.get(name).cloned())
    }

    async fn remove_channel(&self, name: &str) -> Result<(), ChannelError> {
        let mut channels = self.channels.write().await;
        channels.remove(name);
        Ok(())
    }

    async fn channel_exists(&self, name: &str) -> Result<bool, ChannelError> {
        let channels = self.channels.read().await;
        Ok(channels.contains_key(name))
    }
}
