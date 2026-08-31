use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::app::{insert_activity_log, new_id, normalize_optional_text, now_sqlite, sqlite_pool};
use crate::db::models::{
    AiChannel, AiChannelRecord, CreateAiChannel, ListAiChannelModelsResult, TestAiChannelPayload,
    TestAiChannelResult, UpdateAiChannel,
};
use crate::native::model::{ModelClient, ModelClientConfig, RetryConfig};
use crate::native::model_catalog::normalize_channel_model_config;
use crate::native::protocol::{
    normalize_base_url, normalize_extra_headers_json, normalize_protocol,
    parse_channel_models_json, record_to_channel, serialize_channel_models,
};
use crate::native::secret_store::{delete_channel_api_key, resolve_channel_api_key};

fn normalize_channel_name(value: &str) -> Result<String, String> {
    let name = value.trim().to_string();
    if name.is_empty() {
        return Err("渠道名称不能为空".to_string());
    }
    Ok(name)
}

pub(crate) async fn fetch_channel_record(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<AiChannelRecord, String> {
    sqlx::query_as::<_, AiChannelRecord>("SELECT * FROM ai_channels WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|_| format!("渠道 {id} 不存在"))
}

fn non_empty_secret(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

pub(crate) fn should_migrate_channel_api_key(record: &AiChannelRecord) -> Option<String> {
    if non_empty_secret(record.api_key.as_deref()).is_some() {
        return None;
    }
    non_empty_secret(record.api_key_ref.as_deref())
}

pub(crate) fn apply_migrated_api_key(record: &mut AiChannelRecord, secret: &str) {
    record.api_key = Some(secret.to_string());
    record.api_key_ref = None;
}

fn channel_stored_api_key(record: &AiChannelRecord) -> Option<String> {
    non_empty_secret(record.api_key.as_deref())
}

async fn persist_channel_api_key_column(
    pool: &sqlx::SqlitePool,
    id: &str,
    api_key: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE ai_channels SET api_key = $1, api_key_ref = NULL WHERE id = $2")
        .bind(api_key)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| format!("写入渠道 API 密钥失败: {error}"))?;
    Ok(())
}

pub(crate) async fn hydrate_channel_record(
    pool: &sqlx::SqlitePool,
    mut record: AiChannelRecord,
) -> AiChannelRecord {
    let Some(secret_ref) = should_migrate_channel_api_key(&record) else {
        return record;
    };
    match resolve_channel_api_key(&secret_ref) {
        Ok(Some(secret)) => {
            if persist_channel_api_key_column(pool, &record.id, &secret)
                .await
                .is_ok()
            {
                let _ = delete_channel_api_key(&secret_ref);
            }
            apply_migrated_api_key(&mut record, &secret);
            record
        }
        Ok(None) | Err(_) => record,
    }
}

pub(crate) async fn require_channel_api_key(
    pool: &sqlx::SqlitePool,
    record: AiChannelRecord,
) -> Result<(AiChannelRecord, String), String> {
    let had_legacy_ref = should_migrate_channel_api_key(&record).is_some();
    let record = hydrate_channel_record(pool, record).await;
    match channel_stored_api_key(&record) {
        Some(api_key) => Ok((record, api_key)),
        None if had_legacy_ref => Err("渠道 API 密钥不存在或无法读取".to_string()),
        None => Err("渠道未配置 API 密钥".to_string()),
    }
}

async fn fetch_channel(pool: &sqlx::SqlitePool, id: &str) -> Result<AiChannel, String> {
    let record = hydrate_channel_record(pool, fetch_channel_record(pool, id).await?).await;
    record_to_channel(record)
}

fn parse_header_map(raw: Option<&str>) -> Result<HashMap<String, String>, String> {
    let Some(json) = normalize_extra_headers_json(raw)? else {
        return Ok(HashMap::new());
    };
    let value: Value =
        serde_json::from_str(&json).map_err(|_| "额外请求头必须是 JSON 对象".to_string())?;
    let mut headers = HashMap::new();
    if let Some(object) = value.as_object() {
        for (key, item) in object {
            if let Some(text) = item.as_str() {
                headers.insert(key.clone(), text.to_string());
            }
        }
    }
    Ok(headers)
}

struct ResolvedChannelHttp {
    protocol: String,
    base_url: String,
    extra_headers_json: Option<String>,
    api_key: String,
    label: String,
    first_model: Option<String>,
}

async fn resolve_channel_http<R: Runtime>(
    app: &AppHandle<R>,
    payload: &TestAiChannelPayload,
) -> Result<ResolvedChannelHttp, String> {
    let pool = sqlite_pool(app).await?;
    let stored = if let Some(id) = payload
        .id
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        Some(hydrate_channel_record(&pool, fetch_channel_record(&pool, id).await?).await)
    } else {
        None
    };

    let protocol = normalize_protocol(
        payload
            .protocol
            .as_deref()
            .or(stored.as_ref().map(|item| item.protocol.as_str()))
            .unwrap_or(""),
    )?
    .to_string();
    let base_url = normalize_base_url(
        payload
            .base_url
            .as_deref()
            .or(stored.as_ref().map(|item| item.base_url.as_str()))
            .unwrap_or(""),
    )?;
    let extra_headers_json = payload.extra_headers_json.clone().or_else(|| {
        stored
            .as_ref()
            .and_then(|item| item.extra_headers_json.clone())
    });
    let api_key = match normalize_optional_text(payload.api_key.as_deref()) {
        Some(value) => value,
        None => stored
            .as_ref()
            .and_then(channel_stored_api_key)
            .ok_or_else(|| "请先填写渠道 API 密钥".to_string())?,
    };
    Ok(ResolvedChannelHttp {
        protocol,
        base_url,
        extra_headers_json,
        api_key,
        label: stored
            .as_ref()
            .map(|item| item.name.clone())
            .unwrap_or_else(|| "未保存".to_string()),
        first_model: stored.as_ref().and_then(|item| {
            parse_channel_models_json(&item.models_json)
                .ok()
                .and_then(|models| models.into_iter().next().map(|model| model.id))
        }),
    })
}

fn channel_client(
    protocol: &str,
    base_url: &str,
    api_key: &str,
    extra_headers_json: Option<&str>,
) -> Result<ModelClient, String> {
    ModelClient::new(ModelClientConfig {
        protocol: protocol.to_string(),
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
        extra_headers: parse_header_map(extra_headers_json)?,
        retry: RetryConfig::none(),
        timeout: Duration::from_secs(20),
    })
}

async fn send_probe_request(
    protocol: &str,
    base_url: &str,
    api_key: &str,
    extra_headers_json: Option<&str>,
    model: &str,
) -> Result<TestAiChannelResult, String> {
    let client = channel_client(protocol, base_url, api_key, extra_headers_json)?;
    match client.probe(model).await {
        Ok(()) => Ok(TestAiChannelResult {
            ok: true,
            status: Some(200),
            message: "渠道测通成功".to_string(),
        }),
        Err(message) => Ok(TestAiChannelResult {
            ok: false,
            status: None,
            message,
        }),
    }
}

#[tauri::command]
pub async fn list_ai_channels<R: Runtime>(app: AppHandle<R>) -> Result<Vec<AiChannel>, String> {
    let pool = sqlite_pool(&app).await?;
    let records = sqlx::query_as::<_, AiChannelRecord>(
        "SELECT * FROM ai_channels ORDER BY enabled DESC, name COLLATE NOCASE, created_at",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("读取渠道列表失败: {error}"))?;
    let mut channels = Vec::with_capacity(records.len());
    for record in records {
        let record = hydrate_channel_record(&pool, record).await;
        channels.push(record_to_channel(record)?);
    }
    Ok(channels)
}

#[tauri::command]
pub async fn create_ai_channel<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateAiChannel,
) -> Result<AiChannel, String> {
    let pool = sqlite_pool(&app).await?;
    let name = normalize_channel_name(&payload.name)?;
    let protocol = normalize_protocol(&payload.protocol)?.to_string();
    let base_url = normalize_base_url(&payload.base_url)?;
    let extra_headers_json = normalize_extra_headers_json(payload.extra_headers_json.as_deref())?;
    let mut models = payload.models.unwrap_or_default();
    for model in &mut models {
        normalize_channel_model_config(model)?;
    }
    let models_json = serialize_channel_models(&models);
    let enabled = i64::from(payload.enabled.unwrap_or(true));
    let id = new_id();
    let now = now_sqlite();
    let api_key = normalize_optional_text(payload.api_key.as_deref());

    sqlx::query(
        "INSERT INTO ai_channels (id, name, protocol, base_url, api_key, extra_headers_json, models_json, enabled, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(&id)
    .bind(&name)
    .bind(&protocol)
    .bind(&base_url)
    .bind(&api_key)
    .bind(&extra_headers_json)
    .bind(&models_json)
    .bind(enabled)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|error| format!("创建渠道失败: {error}"))?;

    insert_activity_log(
        &pool,
        "ai_channel_created",
        &format!("新增 AI 渠道 {name}（{protocol}）"),
        None,
        None,
        None,
    )
    .await?;

    fetch_channel(&pool, &id).await
}

#[tauri::command]
pub async fn update_ai_channel<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateAiChannel,
) -> Result<AiChannel, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_channel_record(&pool, &id).await?;
    let name = match updates.name.as_deref() {
        Some(value) => normalize_channel_name(value)?,
        None => current.name.clone(),
    };
    let protocol = match updates.protocol.as_deref() {
        Some(value) => normalize_protocol(value)?.to_string(),
        None => normalize_protocol(&current.protocol)?.to_string(),
    };
    let base_url = match updates.base_url.as_deref() {
        Some(value) => normalize_base_url(value)?,
        None => current.base_url.clone(),
    };
    let extra_headers_json = match updates.extra_headers_json {
        Some(Some(value)) => normalize_extra_headers_json(Some(&value))?,
        Some(None) => None,
        None => current.extra_headers_json.clone(),
    };
    let models_json = match updates.models {
        Some(mut models) => {
            for model in &mut models {
                normalize_channel_model_config(model)?;
            }
            serialize_channel_models(&models)
        }
        None => current.models_json.clone(),
    };
    let enabled = updates.enabled.map(i64::from).unwrap_or(current.enabled);
    let now = now_sqlite();
    let incoming_key = normalize_optional_text(updates.api_key.as_deref());
    let (api_key, api_key_ref) = if let Some(secret) = incoming_key {
        if let Some(secret_ref) = current.api_key_ref.as_deref() {
            let _ = delete_channel_api_key(secret_ref);
        }
        (Some(secret), None)
    } else {
        (current.api_key.clone(), current.api_key_ref.clone())
    };

    sqlx::query(
        "UPDATE ai_channels SET name = $1, protocol = $2, base_url = $3, api_key = $4, api_key_ref = $5, extra_headers_json = $6, models_json = $7, enabled = $8, updated_at = $9 WHERE id = $10",
    )
    .bind(&name)
    .bind(&protocol)
    .bind(&base_url)
    .bind(&api_key)
    .bind(&api_key_ref)
    .bind(&extra_headers_json)
    .bind(&models_json)
    .bind(enabled)
    .bind(&now)
    .bind(&id)
    .execute(&pool)
    .await
    .map_err(|error| format!("更新渠道失败: {error}"))?;

    insert_activity_log(
        &pool,
        "ai_channel_updated",
        &format!("更新 AI 渠道 {name}（{protocol}）"),
        None,
        None,
        None,
    )
    .await?;

    fetch_channel(&pool, &id).await
}

#[tauri::command]
pub async fn delete_ai_channel<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_channel_record(&pool, &id).await?;
    let referenced: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM employees WHERE ai_channel_id = $1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .map_err(|error| format!("检查渠道引用失败: {error}"))?;
    if referenced > 0 {
        return Err(format!(
            "渠道「{}」仍被 {} 名员工使用，无法删除",
            current.name, referenced
        ));
    }

    sqlx::query("DELETE FROM ai_channels WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("删除渠道失败: {error}"))?;

    if let Some(secret_ref) = current.api_key_ref.as_deref() {
        let _ = delete_channel_api_key(secret_ref);
    }

    insert_activity_log(
        &pool,
        "ai_channel_deleted",
        &format!("删除 AI 渠道 {}", current.name),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn test_ai_channel<R: Runtime>(
    app: AppHandle<R>,
    payload: TestAiChannelPayload,
) -> Result<TestAiChannelResult, String> {
    let target = resolve_channel_http(&app, &payload).await?;
    let model = payload
        .model
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .or(target.first_model.clone())
        .unwrap_or_else(|| "dummy".to_string());

    let result = send_probe_request(
        &target.protocol,
        &target.base_url,
        &target.api_key,
        target.extra_headers_json.as_deref(),
        &model,
    )
    .await?;

    let pool = sqlite_pool(&app).await?;
    insert_activity_log(
        &pool,
        "ai_channel_tested",
        &format!(
            "测通 AI 渠道 {}（{}）: {}",
            target.label,
            target.protocol,
            if result.ok { "成功" } else { "失败" }
        ),
        None,
        None,
        None,
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn list_ai_channel_models<R: Runtime>(
    app: AppHandle<R>,
    payload: TestAiChannelPayload,
) -> Result<ListAiChannelModelsResult, String> {
    let target = resolve_channel_http(&app, &payload).await?;
    let client = channel_client(
        &target.protocol,
        &target.base_url,
        &target.api_key,
        target.extra_headers_json.as_deref(),
    )?;
    let models = client.list_models().await?;
    let message = if models.is_empty() {
        "网关未返回可用模型，请检查协议、Base URL 和密钥".to_string()
    } else {
        format!("已获取 {} 个模型", models.len())
    };

    let pool = sqlite_pool(&app).await?;
    insert_activity_log(
        &pool,
        "ai_channel_models_fetched",
        &format!(
            "拉取 AI 渠道 {}（{}）模型 {} 个",
            target.label,
            target.protocol,
            models.len()
        ),
        None,
        None,
        None,
    )
    .await?;

    Ok(ListAiChannelModelsResult { models, message })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::AiChannelRecord;

    fn sample_record() -> AiChannelRecord {
        AiChannelRecord {
            id: "c1".to_string(),
            name: "demo".to_string(),
            protocol: "openai".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key_ref: None,
            api_key: None,
            extra_headers_json: None,
            models_json: "[]".to_string(),
            enabled: 1,
            created_at: "2026-08-20 00:00:00".to_string(),
            updated_at: "2026-08-20 00:00:00".to_string(),
        }
    }

    #[test]
    fn migrate_needed_only_when_column_empty_and_ref_present() {
        let mut record = sample_record();
        assert!(should_migrate_channel_api_key(&record).is_none());

        record.api_key_ref = Some("channel:c1".to_string());
        assert_eq!(
            should_migrate_channel_api_key(&record).as_deref(),
            Some("channel:c1")
        );

        record.api_key = Some("sk-live".to_string());
        assert!(should_migrate_channel_api_key(&record).is_none());
    }

    #[test]
    fn apply_migrated_api_key_clears_legacy_ref() {
        let mut record = sample_record();
        record.api_key_ref = Some("channel:c1".to_string());
        apply_migrated_api_key(&mut record, "sk-live");
        assert_eq!(record.api_key.as_deref(), Some("sk-live"));
        assert!(record.api_key_ref.is_none());
    }
}
