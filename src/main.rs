use crate::log::Log;
use crate::server::start_server;

pub mod channel;
pub mod connection;
pub mod handlers;
pub mod protocol;
pub mod error;
pub mod server;
pub mod application;
pub mod log;
pub mod websocket;

#[tokio::main]
async fn main() {
    match  start_server().await {
        Ok(_) => Log::info("Server started"),
        Err(e) => Log::error(format!("Error starting server: {}", e)),
    }
}
