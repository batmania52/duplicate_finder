use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use dup_scanner::model::ScanResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScanStatus {
    #[default]
    Idle,
    Scanning,
    Done,
    Cancelled,
    Error,
}

#[derive(Debug, Default)]
pub struct ScanState {
    pub status: ScanStatus,
    pub log: Vec<String>,
    pub result: Option<ScanResult>,
    pub timestamp: Option<String>,
    pub paths: Vec<String>,
    pub session_uuid: Option<String>,
    pub cancel_token: Option<CancellationToken>,
}

impl ScanState {
    pub fn reset(&mut self) {
        self.status = ScanStatus::Idle;
        self.log.clear();
        self.result = None;
        self.timestamp = None;
        self.paths.clear();
        self.session_uuid = None;
        self.cancel_token = None;
    }
}

pub type SharedState = Arc<Mutex<ScanState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(Mutex::new(ScanState::default()))
}
