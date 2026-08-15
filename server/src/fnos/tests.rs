use super::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

static TEST_SOCKET_ID: AtomicU64 = AtomicU64::new(1);

impl FnosApiClient {
    pub(crate) fn with_limits(
        socket_path: PathBuf,
        request_timeout: Duration,
        max_response_bytes: usize,
    ) -> Self {
        Self {
            socket_path,
            request_timeout,
            max_response_bytes,
        }
    }
}

fn socket_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-api-{label}-{}-{}.sock",
        std::process::id(),
        TEST_SOCKET_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

async fn serve_once(path: &Path, response: Vec<u8>) -> tokio::task::JoinHandle<Vec<u8>> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).expect("test socket should bind");
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("request should connect");
        let request = read_http_request(&mut stream).await;
        stream
            .write_all(&response)
            .await
            .expect("response should write");
        request
    })
}

async fn read_http_request(stream: &mut UnixStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).await.expect("request should read");
        assert_ne!(read, 0, "request ended before body completed");
        request.extend_from_slice(&buffer[..read]);
        let Some(header_end) = request.windows(4).position(|value| value == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .expect("content length should exist");
        if request.len() >= header_end + 4 + content_length {
            return request;
        }
    }
}

fn response(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[tokio::test]
async fn sends_expected_shared_folder_request_without_changing_app_identity() {
    let path = socket_path("request");
    let server = serve_once(
        &path,
        response(
            "200 OK",
            r#"{"code":0,"msg":"","data":{"paths":["/vol1/downloads"]}}"#,
        ),
    )
    .await;
    let client = FnosApiClient::with_limits(path.clone(), Duration::from_secs(1), 4096);

    let result = client
        .query_shared_accessible_folders_with_token(Some("test-token"))
        .await
        .expect("query should succeed");
    let request = String::from_utf8(server.await.expect("server should finish"))
        .expect("request should be utf8");
    let _ = std::fs::remove_file(path);

    assert_eq!(result.paths, vec!["/vol1/downloads"]);
    assert!(request.starts_with("POST /api/v1/trimapp HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-token"));
    assert!(request.contains(r#""req":"trim.file.getSharedAccessibleFolders""#));
    assert!(request.contains(r#""appName":"motrix""#));
}

#[tokio::test]
async fn sends_expected_batch_path_conversion_request_with_language() {
    let path = socket_path("convert-request");
    let server = serve_once(
        &path,
        response(
            "200 OK",
            r#"{"code":0,"msg":"","data":{"status":0,"result":[{"path":"/vol1/a","semanticPath":"Storage 1/a"},{"path":"/vol1/b","semanticPath":"Storage 1/b"}]}}"#,
        ),
    )
    .await;
    let client = FnosApiClient::with_limits(path.clone(), Duration::from_secs(1), 4096);

    let result = client
        .convert_paths_with_token(
            Some("test-token"),
            &["/vol1/a".to_string(), "/vol1/b".to_string()],
            PathLanguage::EnUs,
        )
        .await
        .expect("conversion should succeed");
    let request = String::from_utf8(server.await.expect("server should finish"))
        .expect("request should be utf8");
    let _ = std::fs::remove_file(path);

    assert_eq!(
        result.results,
        vec![
            SemanticPath {
                path: "/vol1/a".to_string(),
                semantic_path: "Storage 1/a".to_string(),
            },
            SemanticPath {
                path: "/vol1/b".to_string(),
                semantic_path: "Storage 1/b".to_string(),
            },
        ]
    );
    assert_eq!(result.http_status, 200);
    assert_eq!(result.business_code, 0);
    assert!(request.starts_with("POST /api/v1/trimapp HTTP/1.1"));
    assert!(request.contains(r#""req":"trim.file.convertPath""#));
    assert!(request.contains(r#""appName":"motrix""#));
    assert!(request.contains(r#""path":["/vol1/a","/vol1/b"]"#));
    assert!(request.contains(r#""language":"en-US""#));
}

#[tokio::test]
async fn path_conversion_supports_empty_batches_and_both_languages() {
    for (label, language, expected_language) in [
        ("convert-zh", PathLanguage::ZhCn, "zh-CN"),
        ("convert-en", PathLanguage::EnUs, "en-US"),
    ] {
        let path = socket_path(label);
        let server = serve_once(
            &path,
            response(
                "200 OK",
                r#"{"code":0,"msg":"","data":{"status":0,"result":[]}}"#,
            ),
        )
        .await;
        let client = FnosApiClient::with_limits(path.clone(), Duration::from_secs(1), 4096);

        let result = client
            .convert_paths_with_token(Some("test-token"), &[], language)
            .await
            .expect("empty conversion should succeed");
        let request = String::from_utf8(server.await.expect("server should finish"))
            .expect("request should be utf8");
        let _ = std::fs::remove_file(path);

        assert!(result.results.is_empty());
        assert!(request.contains(r#""path":[]"#));
        assert!(request.contains(&format!(r#""language":"{expected_language}""#)));
    }
}

#[tokio::test]
async fn path_conversion_rejects_status_and_malformed_payloads() {
    let cases = [
        (
            "convert-status",
            r#"{"code":0,"msg":"","data":{"status":7,"result":[]}}"#,
            Err(FnosApiError::Rejected {
                http_status: Some(200),
                business_code: Some(7),
            }),
        ),
        (
            "convert-result-missing",
            r#"{"code":0,"msg":"","data":{"status":0}}"#,
            Err(FnosApiError::InvalidResponse),
        ),
        (
            "convert-path-missing",
            r#"{"code":0,"msg":"","data":{"status":0,"result":[{"semanticPath":"Storage 1/a"}]}}"#,
            Err(FnosApiError::InvalidResponse),
        ),
        (
            "convert-semantic-missing",
            r#"{"code":0,"msg":"","data":{"status":0,"result":[{"path":"/vol1/a"}]}}"#,
            Err(FnosApiError::InvalidResponse),
        ),
        (
            "convert-not-json",
            "not-json",
            Err(FnosApiError::InvalidResponse),
        ),
    ];

    for (label, body, expected) in cases {
        let path = socket_path(label);
        let server = serve_once(&path, response("200 OK", body)).await;
        let client = FnosApiClient::with_limits(path.clone(), Duration::from_secs(1), 4096);
        let result = client
            .convert_paths_with_token(
                Some("test-token"),
                &["/vol1/a".to_string()],
                PathLanguage::EnUs,
            )
            .await;
        let _ = server.await;
        let _ = std::fs::remove_file(path);
        assert_eq!(result, expected);
    }
}

#[tokio::test]
async fn path_conversion_uses_shared_timeout_and_response_limit() {
    let timeout_path = socket_path("convert-timeout");
    let listener = UnixListener::bind(&timeout_path).expect("timeout socket should bind");
    let waiting_server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("request should connect");
        tokio::time::sleep(Duration::from_secs(1)).await;
    });
    let timeout_client =
        FnosApiClient::with_limits(timeout_path.clone(), Duration::from_millis(10), 4096);
    assert_eq!(
        timeout_client
            .convert_paths_with_token(
                Some("test-token"),
                &["/vol1/a".to_string()],
                PathLanguage::ZhCn,
            )
            .await,
        Err(FnosApiError::Timeout)
    );
    waiting_server.abort();
    let _ = std::fs::remove_file(timeout_path);

    let large_path = socket_path("convert-large");
    let server = serve_once(
        &large_path,
        response(
            "200 OK",
            r#"{"code":0,"msg":"","data":{"status":0,"result":[]}}"#,
        ),
    )
    .await;
    let large_client = FnosApiClient::with_limits(large_path.clone(), Duration::from_secs(1), 16);
    assert_eq!(
        large_client
            .convert_paths_with_token(Some("test-token"), &[], PathLanguage::ZhCn)
            .await,
        Err(FnosApiError::ResponseTooLarge)
    );
    let _ = server.await;
    let _ = std::fs::remove_file(large_path);
}

#[tokio::test]
async fn rejects_missing_or_malformed_tokens_before_connecting() {
    let client = FnosApiClient::with_limits(socket_path("token"), Duration::from_secs(1), 4096);

    assert_eq!(
        client
            .query_shared_accessible_folders_with_token(None)
            .await,
        Err(FnosApiError::TokenMissing)
    );
    assert_eq!(
        client
            .query_shared_accessible_folders_with_token(Some("bad\ntoken"))
            .await,
        Err(FnosApiError::TokenInvalid)
    );
}

#[tokio::test]
async fn rejects_non_success_http_and_official_business_codes() {
    for (label, status, body, expected) in [
        (
            "http",
            "401 Unauthorized",
            r#"{"code":1000001,"msg":"denied","data":null}"#,
            FnosApiError::Rejected {
                http_status: Some(401),
                business_code: Some(1_000_001),
            },
        ),
        (
            "business",
            "200 OK",
            r#"{"code":1000002,"msg":"denied","data":null}"#,
            FnosApiError::Rejected {
                http_status: Some(200),
                business_code: Some(1_000_002),
            },
        ),
    ] {
        let path = socket_path(label);
        let server = serve_once(&path, response(status, body)).await;
        let client = FnosApiClient::with_limits(path.clone(), Duration::from_secs(1), 4096);
        let result = client
            .query_shared_accessible_folders_with_token(Some("test-token"))
            .await;
        let _ = server.await;
        let _ = std::fs::remove_file(path);
        assert_eq!(result, Err(expected));
    }
}

#[tokio::test]
async fn accepts_empty_paths_and_rejects_invalid_success_payloads() {
    let cases = [
        (
            "empty",
            r#"{"code":0,"msg":"","data":{"paths":[]}}"#,
            Ok(Vec::<String>::new()),
        ),
        ("json", "not-json", Err(FnosApiError::InvalidResponse)),
        (
            "missing",
            r#"{"code":0,"msg":"","data":{}}"#,
            Err(FnosApiError::InvalidResponse),
        ),
        (
            "non-string",
            r#"{"code":0,"msg":"","data":{"paths":[42]}}"#,
            Err(FnosApiError::InvalidResponse),
        ),
    ];

    for (label, body, expected) in cases {
        let path = socket_path(label);
        let server = serve_once(&path, response("200 OK", body)).await;
        let client = FnosApiClient::with_limits(path.clone(), Duration::from_secs(1), 4096);
        let result = client
            .query_shared_accessible_folders_with_token(Some("test-token"))
            .await
            .map(|folders| folders.paths);
        let _ = server.await;
        let _ = std::fs::remove_file(path);
        assert_eq!(result, expected);
    }
}

#[tokio::test]
async fn enforces_timeout_and_response_size_limit() {
    let timeout_path = socket_path("timeout");
    let _ = std::fs::remove_file(&timeout_path);
    let listener = UnixListener::bind(&timeout_path).expect("timeout socket should bind");
    let waiting_server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("request should connect");
        tokio::time::sleep(Duration::from_secs(1)).await;
    });
    let timeout_client =
        FnosApiClient::with_limits(timeout_path.clone(), Duration::from_millis(10), 4096);
    assert_eq!(
        timeout_client
            .query_shared_accessible_folders_with_token(Some("test-token"))
            .await,
        Err(FnosApiError::Timeout)
    );
    waiting_server.abort();
    let _ = std::fs::remove_file(timeout_path);

    let large_path = socket_path("large");
    let server = serve_once(
        &large_path,
        response(
            "200 OK",
            r#"{"code":0,"msg":"","data":{"paths":["/vol1/downloads"]}}"#,
        ),
    )
    .await;
    let large_client = FnosApiClient::with_limits(large_path.clone(), Duration::from_secs(1), 16);
    assert_eq!(
        large_client
            .query_shared_accessible_folders_with_token(Some("test-token"))
            .await,
        Err(FnosApiError::ResponseTooLarge)
    );
    let _ = server.await;
    let _ = std::fs::remove_file(large_path);
}

#[test]
fn display_messages_never_include_tokens_paths_or_gateway_messages() {
    let errors = [
        FnosApiError::TokenMissing,
        FnosApiError::TokenInvalid,
        FnosApiError::SocketUnavailable,
        FnosApiError::Timeout,
        FnosApiError::Transport,
        FnosApiError::ResponseTooLarge,
        FnosApiError::Rejected {
            http_status: Some(401),
            business_code: Some(1_000_001),
        },
        FnosApiError::InvalidResponse,
    ];
    for message in errors.map(|error| error.to_string()) {
        assert!(!message.contains("test-token"));
        assert!(!message.contains("/vol1/"));
        assert!(!message.contains(GATEWAY_SOCKET_PATH));
    }
}
