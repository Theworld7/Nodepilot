use super::types::NodeVersion;

#[derive(Debug, Clone)]
pub enum VersionEvent {
    VersionsUpdated(Vec<NodeVersion>),
    InstallProgress {
        version: String,
        stage: String,
        percent: f64,
    },
    VersionActivated {
        version: String,
    },
}

pub trait EventSink: Send {
    fn emit(&mut self, event: VersionEvent);
}
