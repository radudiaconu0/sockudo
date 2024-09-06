use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum PusherMessage {
    #[serde(rename = "pusher:connection_established")]
    ConnectionEstablished {
        socket_id: String,
        activity_timeout: u32,
    },

    #[serde(rename = "pusher:subscribe")]
    Subscribe {
        channel: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        auth: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        channel_data: Option<String>,
    },

    #[serde(rename = "pusher:unsubscribe")]
    Unsubscribe { channel: String },

    #[serde(rename = "pusher:ping")]
    Ping { 
        #[serde(flatten)]
        data: Option<Value> 
    },

    #[serde(rename = "pusher:pong")]
    Pong { 
        #[serde(flatten)]
        data: Option<Value> 
    },

    #[serde(rename = "pusher:error")]
    Error { code: Option<u32>, message: String },

    #[serde(rename = "client_event")]
    ClientEvent {
        channel: String,
        event: String,
        data: Value,
    },
}
    

#[derive(Debug, Serialize, Deserialize)]
pub struct PresenceChannelData {
    pub user_id: String,
    pub user_info: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelSubscription {
    pub channel: String,
    pub auth: Option<String>,
    pub channel_data: Option<PresenceChannelData>,
}
