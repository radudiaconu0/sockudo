pub mod memory_channel_manager;

use async_trait::async_trait;
use std::sync::Arc;
use serde_json::Value;
use crate::connection::SafeConnection;

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelType {
    Public,
    Private,
    Presence,
}

#[derive(Clone)]
pub struct PresenceUser {
    pub user_id: String,
    pub user_info: Value,
}

#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    fn channel_type(&self) -> ChannelType;
    async fn subscribers(&self) -> Vec<String>;
    async fn subscribe(&self, connection: &SafeConnection) -> Result<(), ChannelError>;
    async fn unsubscribe(&self, socket_id: &str) -> Result<(), ChannelError>;
    async fn broadcast(&self, message: String) -> Result<(), ChannelError>;
    async fn send_to_connection(&self, socket_id: &str, message: String) -> Result<(), ChannelError>;
    async fn subscriber_count(&self) -> Result<usize, ChannelError>;
}

#[async_trait]
pub trait PresenceChannel: Channel {
    async fn add_presence_user(&self, connection: SafeConnection, user: PresenceUser) -> Result<(), ChannelError>;
    async fn remove_presence_user(&self, socket_id: &str) -> Result<(), ChannelError>;
    async fn get_presence_users(&self) -> Result<Vec<PresenceUser>, ChannelError>;
}

#[async_trait]
pub trait ChannelManager: Send + Sync {
    async fn create_channel(&self, name: String, channel_type: ChannelType) -> Result<Arc<dyn Channel>, ChannelError>;
    async fn get_channel(&self, name: &str) -> Result<Option<Arc<dyn Channel>>, ChannelError>;
    async fn remove_channel(&self, name: &str) -> Result<(), ChannelError>;
    async fn channel_exists(&self, name: &str) -> Result<bool, ChannelError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Channel already exists")]
    ChannelAlreadyExists,
    #[error("Channel not found")]
    ChannelNotFound,
    #[error("Invalid channel name")]
    InvalidChannelName,
    #[error("Invalid channel type")]
    InvalidChannelType,
    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type SafeChannelManager = Arc<dyn ChannelManager>;

pub fn create_channel_manager() -> SafeChannelManager {
    Arc::new(memory_channel_manager::MemoryChannelManager::new())
}