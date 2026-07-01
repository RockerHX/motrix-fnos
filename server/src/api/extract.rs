use crate::api::error::ApiError;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::Json;
use serde::de::DeserializeOwned;

pub struct ApiJson<T>(pub T);

#[axum::async_trait]
impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(payload)| Self(payload))
            .map_err(map_json_rejection)
    }
}

fn map_json_rejection(rejection: JsonRejection) -> ApiError {
    ApiError::bad_request("invalid_json", format!("请求体 JSON 无效：{}", rejection))
}
