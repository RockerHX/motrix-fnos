use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Aria2TaskStatus {
    pub(super) gid: Option<String>,
    pub(super) status: String,
    pub(super) total_length: String,
    pub(super) completed_length: String,
    pub(super) download_speed: String,
    pub(super) error_code: Option<String>,
    pub(super) error_message: Option<String>,
    pub(super) dir: Option<String>,
    pub(super) files: Option<Vec<Aria2FileStatus>>,
    pub(super) followed_by: Option<Vec<String>>,
    pub(super) bittorrent: Option<Aria2BittorrentStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Aria2FileStatus {
    #[serde(default, deserialize_with = "deserialize_aria2_u32")]
    pub(super) index: u32,
    pub(super) path: String,
    #[serde(default)]
    pub(super) length: String,
    #[serde(default)]
    pub(super) completed_length: String,
    #[serde(default)]
    pub(super) selected: String,
    #[serde(default)]
    pub(super) uris: Vec<Aria2UriStatus>,
}

pub(super) fn deserialize_aria2_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| serde::de::Error::custom("invalid aria2 u32 number")),
        serde_json::Value::String(text) => text
            .parse::<u32>()
            .map_err(|_| serde::de::Error::custom("invalid aria2 u32 string")),
        serde_json::Value::Null => Ok(0),
        _ => Err(serde::de::Error::custom("invalid aria2 u32 value")),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Aria2UriStatus {
    pub(super) uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Aria2BittorrentStatus {
    pub(super) info: Option<Aria2BittorrentInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Aria2BittorrentInfo {
    pub(super) name: Option<String>,
}
