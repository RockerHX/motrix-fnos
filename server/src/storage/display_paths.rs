use crate::fnos::{ConvertedPaths, FnosApiClient, PathLanguage};
use std::collections::HashMap;

use super::DisplayPath;

pub(crate) async fn display_paths(
    client: &FnosApiClient,
    paths: &[String],
    language: PathLanguage,
) -> Vec<DisplayPath> {
    let converted = client.convert_paths(paths, language).await;
    match converted {
        Ok(converted) => match_converted_paths(paths, converted),
        Err(_) => fallback_paths(paths),
    }
}

fn match_converted_paths(paths: &[String], converted: ConvertedPaths) -> Vec<DisplayPath> {
    let mut matches = HashMap::<String, Vec<String>>::new();
    for result in converted.results {
        matches
            .entry(result.path)
            .or_default()
            .push(result.semantic_path);
    }

    paths
        .iter()
        .map(|path| {
            let display_path = matches
                .get(path)
                .filter(|values| values.len() == 1)
                .and_then(|values| values.first())
                .filter(|value| !value.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| path.clone());
            DisplayPath {
                path: path.clone(),
                display_path,
            }
        })
        .collect()
}

fn fallback_paths(paths: &[String]) -> Vec<DisplayPath> {
    paths
        .iter()
        .map(|path| DisplayPath {
            path: path.clone(),
            display_path: path.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
