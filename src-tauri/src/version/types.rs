use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteVersion {
    pub version: String,
    pub date: String,
    pub files: Vec<String>,
    pub npm: Option<String>,
    pub v8: Option<String>,
    pub uv: Option<String>,
    pub zlib: Option<String>,
    pub openssl: Option<String>,
    pub modules: Option<String>,
    #[serde(deserialize_with = "deserialize_lts")]
    pub lts: LtsStatus,
    pub security: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LtsStatus {
    Codename(String),
    False(bool),
}

impl LtsStatus {
    pub fn is_lts(&self) -> bool {
        matches!(self, LtsStatus::Codename(_))
    }

    pub fn codename(&self) -> Option<&str> {
        match self {
            LtsStatus::Codename(s) => Some(s.as_str()),
            LtsStatus::False(_) => None,
        }
    }
}

fn deserialize_lts<'de, D>(deserializer: D) -> Result<LtsStatus, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(LtsStatus::Codename(s)),
        serde_json::Value::Bool(b) => Ok(LtsStatus::False(b)),
        _ => Err(de::Error::custom("expected string or bool for lts")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeVersion {
    pub version: String,
    pub date: String,
    pub lts: bool,
    pub lts_codename: Option<String>,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

impl From<RemoteVersion> for NodeVersion {
    fn from(rv: RemoteVersion) -> Self {
        NodeVersion {
            version: rv.version,
            date: rv.date,
            lts: rv.lts.is_lts(),
            lts_codename: rv.lts.codename().map(String::from),
            files: rv.files,
            installed: None,
            active: None,
        }
    }
}
