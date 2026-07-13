use super::*;

#[test]
fn compare_versions_uses_numeric_segments() {
    assert_eq!(compare_versions("1.3.3", "1.2.0"), Ordering::Greater);
    assert_eq!(compare_versions("v1.2.0", "1.2.0"), Ordering::Equal);
    assert_eq!(compare_versions("1.2.0", "1.10.0"), Ordering::Less);
}

#[test]
fn release_assets_only_returns_fpk_archives() {
    let assets = release_assets(vec![
        GitHubReleaseAsset {
            name: "motrix.fnos_1.3.3_x86.fpk".to_string(),
            browser_download_url: "https://example.com/x86".to_string(),
        },
        GitHubReleaseAsset {
            name: "motrix.fnos_1.3.3_arm.fpk".to_string(),
            browser_download_url: "https://example.com/arm".to_string(),
        },
        GitHubReleaseAsset {
            name: "SHA256SUMS.txt".to_string(),
            browser_download_url: "https://example.com/sums".to_string(),
        },
    ]);

    assert_eq!(
        assets
            .iter()
            .map(|asset| asset.architecture.as_str())
            .collect::<Vec<_>>(),
        vec!["x86", "arm"]
    );
}

#[test]
fn unavailable_update_check_keeps_release_page_link() {
    let response = unavailable_update_check("1.2.0", "network unavailable");

    assert_eq!(response.status, UpdateCheckStatus::Unavailable);
    assert!(!response.has_update);
    assert_eq!(response.release_url.as_deref(), Some(RELEASE_PAGE_URL));
    assert!(response.message.contains("network unavailable"));
}

#[test]
fn update_check_from_release_detects_newer_version() {
    let response = update_check_from_release(
        "1.2.0",
        GitHubRelease {
            tag_name: "v1.3.3".to_string(),
            html_url: "https://example.com/release".to_string(),
            assets: Vec::new(),
        },
    );

    assert_eq!(response.latest_version.as_deref(), Some("1.3.3"));
    assert!(response.has_update);
    assert_eq!(response.status, UpdateCheckStatus::Available);
}
