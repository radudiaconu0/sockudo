use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event" , content = "data")]
pub enum PusherEvent {
    #[serde(rename = "pusher_internal:subscription_succeeded")]
    SubscriptionSucceeded {
        channel: String,
        data: Option<Value>,
    },

    #[serde(rename = "pusher_internal:member_added")]
    MemberAdded {
        channel: String,
        user_id: String,
        user_info: Value,
    },

    #[serde(rename = "pusher_internal:member_removed")]
    MemberRemoved {
        channel: String,
        user_id: String,
    },

    #[serde(rename = "pusher:subscription_error")]
    SubscriptionError {
        channel: String,
        error: String,
    },

    ClientEvent {
        event: String,
        channel: String,
        data: Value,
    },

    // This variant can be used for custom events
    Custom {
        channel: String,
        data: Value,
    },
}


#[derive(Debug, Serialize, Deserialize)]
pub struct PusherApiEvent {
    pub name: String,
    pub(crate) data: String,
    pub(crate) channels: Vec<String>,
    pub channel: Option<String>,
    pub socket_id: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PusherApiEventResponse {
    pub channel: String,
    pub event: String,
    pub data: Option<Value>
}