use super::*;
use crate::database::connect_database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SampleConfig {
    value: String,
}

#[test]
fn settings_repository_round_trips_app_config() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-settings-test-{}.sqlite",
                current_timestamp_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let value = SampleConfig {
                value: "test".to_string(),
            };

            set_app_config_value(&database.pool, "download", &value)
                .await
                .expect("app config should save");
            let app_config: Option<SampleConfig> = get_app_config_value(&database.pool, "download")
                .await
                .expect("app config should read");
            assert_eq!(app_config, Some(value));

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}
