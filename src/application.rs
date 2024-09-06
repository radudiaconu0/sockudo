use crate::channel::{create_channel_manager, SafeChannelManager};
use crate::connection::{create_connection_manager, SafeConnectionManager};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Application {
    pub app_id: String,
    pub key: String,
    pub secret: String,
    pub channel_manager: SafeChannelManager,
    pub connection_manager: SafeConnectionManager,
}

impl Application {
    pub fn new(app_id: String, key: String, secret: String) -> Self {
        Self {
            app_id,
            key,
            secret,
            channel_manager: create_channel_manager(),
            connection_manager: create_connection_manager(),
        }
    }
}

pub struct ApplicationManager {
    applications: RwLock<HashMap<String, Arc<Application>>>,
}

impl ApplicationManager {
    pub fn new() -> Self {
        let application = HashMap::from([(
            "test".to_string(),
            Arc::new(Application::new(
                "test".to_string(),
                "test".to_string(),
                "test".to_string(),
            )),
        )]);
        Self {
            applications: RwLock::new(HashMap::from(application)),
        }
    }

    pub async fn add_application(&self, app_id: String, key: String, secret: String) {
        let application = Arc::new(Application::new(app_id.clone(), key, secret));
        let mut applications = self.applications.write().await;
        applications.insert(app_id, application);
    }

    pub async fn get_application(&self, app_id: &str) -> Option<Arc<Application>> {
        let applications = self.applications.read().await;
        applications.get(app_id).cloned()
    }

    pub async fn remove_application(&self, app_id: &str) {
        let mut applications = self.applications.write().await;
        applications.remove(app_id);
    }

    pub async fn authenticate_key(&self, key: &str) -> Option<Arc<Application>> {
        let applications = self.applications.read().await;
        applications.values().find(|app| app.key == key).cloned()
    }
}

pub type SafeApplicationManager = Arc<ApplicationManager>;

pub fn create_application_manager() -> SafeApplicationManager {
    Arc::new(ApplicationManager::new())
}
