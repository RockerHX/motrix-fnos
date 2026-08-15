use super::*;
use crate::fnos::{FnosApiError, SemanticPath};

fn converted(results: Vec<SemanticPath>) -> ConvertedPaths {
    ConvertedPaths {
        results,
        http_status: 200,
        business_code: 0,
    }
}

fn semantic(path: &str, display_path: &str) -> SemanticPath {
    SemanticPath {
        path: path.to_string(),
        semantic_path: display_path.to_string(),
    }
}

#[test]
fn matches_results_by_exact_original_path_without_reordering() {
    let paths = vec!["/vol1/a".to_string(), "/vol1/b".to_string()];
    let result = match_converted_paths(
        &paths,
        converted(vec![
            semantic("/vol1/b", "Storage 1/b"),
            semantic("/vol1/a", "Storage 1/a"),
        ]),
    );

    assert_eq!(
        result,
        vec![
            DisplayPath {
                path: "/vol1/a".to_string(),
                display_path: "Storage 1/a".to_string(),
            },
            DisplayPath {
                path: "/vol1/b".to_string(),
                display_path: "Storage 1/b".to_string(),
            },
        ]
    );
}

#[test]
fn missing_duplicate_and_blank_results_fall_back_per_path() {
    let paths = vec![
        "/vol1/missing".to_string(),
        "/vol1/duplicate".to_string(),
        "/vol1/blank".to_string(),
        "/vol1/good".to_string(),
    ];
    let result = match_converted_paths(
        &paths,
        converted(vec![
            semantic("/vol1/duplicate", "first"),
            semantic("/vol1/duplicate", "second"),
            semantic("/vol1/blank", "  "),
            semantic("/vol1/good", "Storage 1/good"),
            semantic("/vol1/extra", "Storage 1/extra"),
        ]),
    );

    assert_eq!(result[0].display_path, "/vol1/missing");
    assert_eq!(result[1].display_path, "/vol1/duplicate");
    assert_eq!(result[2].display_path, "/vol1/blank");
    assert_eq!(result[3].display_path, "Storage 1/good");
}

#[test]
fn upstream_failures_fall_back_without_changing_real_paths() {
    let paths = vec!["/vol1/a".to_string(), "/vol1/b".to_string()];
    let fallback = match Err::<ConvertedPaths, _>(FnosApiError::TokenMissing) {
        Ok(result) => match_converted_paths(&paths, result),
        Err(_) => fallback_paths(&paths),
    };

    assert_eq!(fallback[0].path, "/vol1/a");
    assert_eq!(fallback[0].display_path, "/vol1/a");
    assert_eq!(fallback[1].path, "/vol1/b");
    assert_eq!(fallback[1].display_path, "/vol1/b");
}
