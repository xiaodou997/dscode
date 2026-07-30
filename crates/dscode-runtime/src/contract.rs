use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppServerClient, RuntimeError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyContractReport {
    pub model_count: usize,
    pub default_model: Option<String>,
    pub thread_count: usize,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub namespace_tools: bool,
    pub image_generation: bool,
    pub web_search: bool,
}

pub fn run_read_only_contract(
    client: &mut AppServerClient,
    provider_id: &str,
    timeout: Duration,
) -> Result<ReadOnlyContractReport, RuntimeError> {
    let capabilities = client.request("modelProvider/capabilities/read", json!({}), timeout)?;
    let capabilities = serde_json::from_value(capabilities).map_err(|source| {
        RuntimeError::InvalidProtocol(format!("cannot decode provider capabilities: {source}"))
    })?;

    let models = client.request("model/list", json!({}), timeout)?;
    let models = parse_model_list(models)?;

    let threads = client.request(
        "thread/list",
        json!({
            "limit": 10,
            "modelProviders": [provider_id],
            "useStateDbOnly": true
        }),
        timeout,
    )?;
    let thread_count = parse_data_count(threads, "thread/list")?;

    Ok(ReadOnlyContractReport {
        model_count: models.data.len(),
        default_model: models
            .data
            .into_iter()
            .find(|model| model.is_default)
            .map(|model| model.model),
        thread_count,
        capabilities,
    })
}

#[derive(Deserialize)]
struct ModelListResponse {
    data: Vec<ModelSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelSummary {
    model: String,
    is_default: bool,
}

fn parse_model_list(value: Value) -> Result<ModelListResponse, RuntimeError> {
    serde_json::from_value(value).map_err(|source| {
        RuntimeError::InvalidProtocol(format!("cannot decode model/list response: {source}"))
    })
}

fn parse_data_count(value: Value, method: &str) -> Result<usize, RuntimeError> {
    value
        .get("data")
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| {
            RuntimeError::InvalidProtocol(format!("`{method}` response has no data array"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_default_model() {
        let models = parse_model_list(json!({
            "data": [
                { "model": "gpt-a", "isDefault": false },
                { "model": "gpt-b", "isDefault": true }
            ]
        }))
        .expect("valid model response");

        let default = models
            .data
            .into_iter()
            .find(|model| model.is_default)
            .map(|model| model.model);

        assert_eq!(default.as_deref(), Some("gpt-b"));
    }

    #[test]
    fn rejects_thread_lists_without_data() {
        let error = parse_data_count(json!({ "threads": [] }), "thread/list")
            .expect_err("invalid thread response");

        assert!(matches!(error, RuntimeError::InvalidProtocol(_)));
    }
}
