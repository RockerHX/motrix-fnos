use super::*;
use axum::{extract::State, routing::post, Json, Router};

#[tokio::test]
async fn session_save_requires_a_confirmed_ok_response() {
    for result in [
        serde_json::json!("OK"),
        serde_json::json!(false),
        serde_json::json!(null),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = Router::new().route("/jsonrpc", post(|State(result): State<serde_json::Value>, Json(body): Json<serde_json::Value>| async move {
            Json(serde_json::json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))
        })).with_state(result.clone());
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let mut config = Aria2Config::from_env();
        config.rpc_port = port;
        let logs = DebugLogStore::default();
        let saved = save_session(&Aria2RpcClient::new(), &config, Some(&logs)).await;
        assert_eq!(saved.is_ok(), result == "OK");
        assert_eq!(
            logs.list()
                .iter()
                .any(|entry| entry.message == "Aria2 session 已保存"),
            result == "OK"
        );
        handle.abort();
    }
}
