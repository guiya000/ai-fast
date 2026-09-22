use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tokio::sync::watch;

const KEYRING_SERVICE: &str = "com.local.aidesk.models";
const CHAT_CANCELED: &str = "CHAT_CANCELED";

#[derive(Default)]
struct ChatState {
    canceled: Mutex<HashSet<String>>,
    cancelers: Mutex<HashMap<String, watch::Sender<bool>>>,
}

#[derive(Default)]
struct WorkspaceState {
    root: Mutex<Option<PathBuf>>,
}

impl ChatState {
    fn start(&self, stream_id: &str) -> watch::Receiver<bool> {
        self.canceled.lock().unwrap().remove(stream_id);
        let (sender, receiver) = watch::channel(false);
        self.cancelers
            .lock()
            .unwrap()
            .insert(stream_id.to_string(), sender);
        receiver
    }

    fn cancel(&self, stream_id: &str) {
        self.canceled.lock().unwrap().insert(stream_id.to_string());
        if let Some(sender) = self.cancelers.lock().unwrap().get(stream_id) {
            let _ = sender.send(true);
        }
    }

    fn finish(&self, stream_id: &str) {
        self.canceled.lock().unwrap().remove(stream_id);
        self.cancelers.lock().unwrap().remove(stream_id);
    }

    fn ensure_active(&self, stream_id: &str) -> Result<(), String> {
        if self.canceled.lock().unwrap().contains(stream_id) {
            Err(CHAT_CANCELED.to_string())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(default)]
    images: Vec<ChatImage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatImage {
    data_url: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct WorkspaceChat {
    #[serde(default)]
    messages: Vec<ChatMessage>,
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct WorkspaceConfig {
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    chats: HashMap<String, WorkspaceChat>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    stream_id: Option<String>,
    #[serde(flatten)]
    extra_params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImagesApiRequest {
    prompt: String,
    #[serde(flatten)]
    extra_params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageEditRequest {
    #[serde(default)]
    image_data_urls: Vec<String>,
    #[serde(default)]
    image_data_url: Option<String>,
    prompt: String,
    #[serde(flatten)]
    extra_params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListProviderModelsRequest {
    base_url: String,
    api_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderModelView {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    owned_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Conversation {
    id: String,
    title: String,
    messages: Vec<ChatMessage>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModelParam {
    key: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_param_type")]
    value_type: String,
    #[serde(default)]
    value: serde_json::Value,
    #[serde(default)]
    options: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModelProfile {
    id: String,
    name: String,
    #[serde(default = "default_function_type")]
    function_type: String,
    #[serde(default = "default_input_types")]
    input_types: Vec<String>,
    #[serde(default = "default_api_format")]
    api_format: String,
    #[serde(default)]
    request_path: String,
    #[serde(default, skip_serializing)]
    base_url: String,
    #[serde(default, rename = "model", skip_serializing)]
    legacy_model: Option<String>,
    #[serde(default)]
    api_key: String,
    #[serde(default = "default_temperature")]
    temperature: f32,
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
    #[serde(default)]
    params: Vec<ModelParam>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct SupplierConfig {
    #[serde(default)]
    official_address: String,
    #[serde(rename = "baseURL", alias = "baseUrl")]
    base_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Supplier {
    name: String,
    config: SupplierConfig,
    models: Vec<ModelProfile>,
}

fn default_temperature() -> f32 {
    0.7
}
fn default_max_tokens() -> u32 {
    2000
}
fn default_api_format() -> String {
    "chatCompletions".to_string()
}
fn default_param_type() -> String {
    "string".to_string()
}
fn default_function_type() -> String {
    "chat".to_string()
}
fn default_input_types() -> Vec<String> {
    vec!["text".to_string()]
}

fn model_supports_input_type(model: &ModelProfile, input_type: &str) -> bool {
    model.input_types.iter().any(|item| item == input_type)
}
fn model_endpoint(base_url: &str, request_path: &str, default_path: &str) -> String {
    let path = if request_path.trim().is_empty() {
        default_path
    } else {
        request_path.trim()
    };
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default)]
    autostart: bool,
    #[serde(default)]
    active_model_id: Option<String>,
    #[serde(default)]
    suppliers: Vec<Supplier>,
    #[serde(default = "default_editor_language")]
    editor_language: String,
    #[serde(default = "default_shortcuts")]
    shortcuts: HashMap<String, String>,
    #[serde(default, skip_serializing)]
    models: Vec<ModelProfile>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModelProfileView {
    config_id: String,
    supplier_name: String,
    id: String,
    name: String,
    function_type: String,
    input_types: Vec<String>,
    api_format: String,
    request_path: String,
    base_url: String,
    model: String,
    temperature: f32,
    max_tokens: u32,
    api_key: String,
    api_key_set: bool,
    params: Vec<ModelParam>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsView {
    theme: String,
    language: String,
    autostart: bool,
    active_model_id: Option<String>,
    editor_language: String,
    shortcuts: HashMap<String, String>,
    suppliers: Vec<SupplierView>,
    models: Vec<ModelProfileView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupplierView {
    name: String,
    config: SupplierConfig,
    models: Vec<ModelProfileView>,
}

fn default_theme() -> String {
    "system".to_string()
}
fn default_language() -> String {
    "zh-CN".to_string()
}

fn default_editor_language() -> String {
    "zh-CN".to_string()
}

fn default_shortcuts() -> HashMap<String, String> {
    HashMap::from([
        ("closeTab".to_string(), "Ctrl+W".to_string()),
        ("switchTab".to_string(), "Ctrl+Tab".to_string()),
        ("nextTab".to_string(), "Ctrl+PageDown".to_string()),
        ("previousTab".to_string(), "Ctrl+PageUp".to_string()),
        ("quickOpen".to_string(), "Ctrl+P".to_string()),
        ("commandPalette".to_string(), "Ctrl+Shift+P".to_string()),
        ("saveFile".to_string(), "Ctrl+S".to_string()),
        ("aiEdit".to_string(), "Ctrl+Enter".to_string()),
        ("formatDocument".to_string(), "Shift+Alt+F".to_string()),
        ("toggleComment".to_string(), "Ctrl+/".to_string()),
        ("goToDefinition".to_string(), "F12".to_string()),
        ("renameSymbol".to_string(), "F2".to_string()),
    ])
}

fn normalize_editor_settings(settings: &mut AppSettings) {
    if !matches!(settings.editor_language.as_str(), "zh-CN" | "en-US") {
        settings.editor_language = default_editor_language();
    }
    let defaults = default_shortcuts();
    for (key, value) in defaults {
        settings.shortcuts.entry(key).or_insert(value);
    }
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Unable to locate app config directory: {error}"))?
        .join("settings.json");
    Ok(path)
}

fn keyring_entry(model_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, model_id)
        .map_err(|error| format!("无法打开系统凭据存储：{error}"))
}

fn credential_id(supplier_name: &str, model_id: &str) -> String {
    format!("{supplier_name}::{model_id}")
}

fn read_model_api_key(model: &ModelProfile) -> String {
    model.api_key.trim().to_string()
}

fn read_keyring_api_key(supplier_name: &str, model_id: &str) -> String {
    keyring_entry(&credential_id(supplier_name, model_id))
        .and_then(|entry| entry.get_password().map_err(|error| error.to_string()))
        .unwrap_or_default()
}

fn read_api_key(model_id: &str) -> Result<String, String> {
    keyring_entry(model_id)?
        .get_password()
        .map_err(|_| "当前模型没有配置 API Key，请打开设置完成配置".to_string())
}

fn load_settings_file(app: &AppHandle) -> Result<Option<AppSettings>, String> {
    let path = settings_path(app)?;
    if path.exists() {
        let body =
            fs::read_to_string(path).map_err(|error| format!("读取应用设置失败：{error}"))?;
        return serde_json::from_str(&body)
            .map(Some)
            .map_err(|error| format!("应用设置格式错误：{error}"));
    }

    Ok(None)
}

fn default_settings() -> AppSettings {
    AppSettings {
        theme: default_theme(),
        language: default_language(),
        autostart: false,
        active_model_id: None,
        suppliers: Vec::new(),
        editor_language: default_editor_language(),
        shortcuts: default_shortcuts(),
        models: Vec::new(),
    }
}

fn migrate_settings(mut settings: AppSettings) -> Result<AppSettings, String> {
    if settings.suppliers.is_empty() && !settings.models.is_empty() {
        let legacy_models = settings.models;
        let base_url = legacy_models
            .first()
            .map(|model| model.base_url.clone())
            .unwrap_or_default();
        let models = legacy_models
            .into_iter()
            .map(|legacy| {
                let legacy_id = legacy.id.clone();
                let api_key = if legacy.api_key.is_empty() {
                    read_api_key(&legacy_id).unwrap_or_default()
                } else {
                    legacy.api_key
                };
                ModelProfile {
                    id: legacy.legacy_model.unwrap_or(legacy_id),
                    name: legacy.name,
                    function_type: if matches!(legacy.api_format.trim(), "images" | "imagesApi") {
                        "image".to_string()
                    } else {
                        legacy.function_type
                    },
                    input_types: default_input_types(),
                    api_format: legacy.api_format,
                    request_path: legacy.request_path,
                    base_url: legacy.base_url,
                    legacy_model: None,
                    api_key,
                    temperature: legacy.temperature,
                    max_tokens: legacy.max_tokens,
                    params: legacy.params,
                }
            })
            .collect();
        settings.suppliers = vec![Supplier {
            name: "Migrated supplier".to_string(),
            config: SupplierConfig {
                official_address: String::new(),
                base_url,
            },
            models,
        }];
        settings.models = Vec::new();
    }
    Ok(settings)
}

fn save_settings_file(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建设置目录失败：{error}"))?;
    }
    let body = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("保存应用设置失败：{error}"))?;
    fs::write(path, body).map_err(|error| format!("写入应用设置失败：{error}"))
}

fn apply_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let run_key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                winreg::enums::KEY_READ | winreg::enums::KEY_WRITE,
            )
            .map_err(|error| format!("无法访问 Windows 开机启动设置：{error}"))?;
        if enabled {
            let executable =
                std::env::current_exe().map_err(|error| format!("无法获取应用路径：{error}"))?;
            let value = format!("\"{}\"", executable.display());
            run_key
                .set_value("AI-fast", &value)
                .map_err(|error| format!("开启开机启动失败：{error}"))?;
        } else {
            let _ = run_key.delete_value("AI-fast");
        }
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("当前平台暂不支持开机自动启动".to_string())
    }
}

fn model_config_id(supplier_name: &str, model_name: &str) -> String {
    format!("{supplier_name}::{model_name}")
}

fn model_function_type(model: &ModelProfile) -> String {
    if matches!(model.function_type.as_str(), "image" | "images")
        || matches!(model.api_format.trim(), "images" | "imagesApi")
    {
        "image".to_string()
    } else {
        "chat".to_string()
    }
}

fn supplier_model_view(supplier: &Supplier, model: &ModelProfile) -> ModelProfileView {
    let api_key = model.api_key.clone();
    ModelProfileView {
        config_id: model_config_id(&supplier.name, &model.name),
        supplier_name: supplier.name.clone(),
        api_key_set: !api_key.is_empty(),
        id: model.id.clone(),
        name: model.name.clone(),
        function_type: model_function_type(model),
        input_types: model.input_types.clone(),
        api_format: model.api_format.clone(),
        request_path: model.request_path.clone(),
        base_url: supplier.config.base_url.clone(),
        model: model.id.clone(),
        temperature: model.temperature,
        max_tokens: model.max_tokens,
        api_key,
        params: model.params.clone(),
    }
}

fn settings_view(settings: AppSettings) -> AppSettingsView {
    let suppliers: Vec<SupplierView> = settings
        .suppliers
        .iter()
        .map(|supplier| SupplierView {
            name: supplier.name.clone(),
            config: supplier.config.clone(),
            models: supplier
                .models
                .iter()
                .map(|model| supplier_model_view(supplier, model))
                .collect(),
        })
        .collect();
    let models = suppliers
        .iter()
        .flat_map(|supplier| supplier.models.clone())
        .collect();
    AppSettingsView {
        theme: settings.theme,
        language: settings.language,
        autostart: settings.autostart,
        active_model_id: settings.active_model_id,
        editor_language: settings.editor_language,
        shortcuts: settings.shortcuts,
        suppliers,
        models,
    }
}

fn model_config_for_id(app: &AppHandle, requested_id: Option<&str>) -> Result<(ModelProfile, String), String> {
    let settings = migrate_settings(load_settings_file(app)?.unwrap_or_else(default_settings))?;
    let language = settings.language.clone();
    let active_id = requested_id.or(settings.active_model_id.as_deref());
    let selected = settings
        .suppliers
        .iter()
        .flat_map(|supplier| supplier.models.iter().map(move |model| (supplier, model)))
        .find(|&(supplier, model)| {
            active_id.is_none()
                || active_id == Some(model_config_id(&supplier.name, &model.name).as_str())
                || active_id == Some(model.id.as_str())
        });
    let (supplier, model) = selected.ok_or_else(|| "请先在设置中添加并选择一个模型".to_string())?;
    let mut model = model.clone();
    model.base_url = supplier.config.base_url.clone();
    model.api_key = read_model_api_key(&model);
    if model.base_url.trim().is_empty()
        || model.api_key.trim().is_empty()
        || model.id.trim().is_empty()
    {
        return Err("当前模型缺少 API 地址、API Key 或模型 ID".to_string());
    }
    Ok((model, language))
}

fn active_model_config(app: &AppHandle) -> Result<(ModelProfile, String), String> {
    model_config_for_id(app, None)
}

fn system_prompt_for(language: &str) -> &'static str {
    if language == "en-US" {
        "You are a helpful AI assistant. Please answer in English."
    } else {
        "你是一个有帮助的 AI 助手，请使用中文回答问题。"
    }
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<AppSettingsView, String> {
    let original = load_settings_file(&app)?.unwrap_or_else(default_settings);
    let was_legacy = original.suppliers.is_empty() && !original.models.is_empty();
    let mut settings = migrate_settings(original)?;
    normalize_editor_settings(&mut settings);
    let credentials_restored = migrate_model_credentials(&mut settings)?;
    let active_exists = settings
        .active_model_id
        .as_deref()
        .map(|active_id| {
            settings.suppliers.iter().any(|supplier| {
                supplier.models.iter().any(|model| {
                    model_config_id(&supplier.name, &model.name) == active_id
                        || model.id == active_id
                })
            })
        })
        .unwrap_or(false);
    if !active_exists {
        settings.active_model_id = settings
            .suppliers
            .first()
            .and_then(|supplier| supplier.models.first())
            .map(|model| model_config_id(&settings.suppliers[0].name, &model.name));
    }
    if was_legacy || credentials_restored {
        save_settings_file(&app, &settings)?;
    }
    Ok(settings_view(settings))
}

fn migrate_model_credentials(settings: &mut AppSettings) -> Result<bool, String> {
    let mut migrated = false;
    for supplier in &mut settings.suppliers {
        let supplier_name = supplier.name.clone();
        for model in &mut supplier.models {
            if model.api_key.trim().is_empty() {
                let api_key = read_keyring_api_key(&supplier_name, &model.id);
                if !api_key.is_empty() {
                    model.api_key = api_key;
                    migrated = true;
                }
            }
        }
    }
    Ok(migrated)
}

#[tauri::command]
fn save_settings(app: AppHandle, mut settings: AppSettings) -> Result<AppSettingsView, String> {
    normalize_editor_settings(&mut settings);
    let mut supplier_names = std::collections::HashSet::new();
    for supplier in &mut settings.suppliers {
        let supplier_name = supplier.name.trim().to_string();
        if supplier_name.chars().count() < 2
            || supplier_name.chars().count() > 20
            || supplier.config.base_url.trim().is_empty()
            || !supplier_names.insert(supplier_name.to_lowercase())
        {
            return Err(
                "供应商名称需为 2-20 个字符、地址不能为空且不能重复"
                    .to_string(),
            );
        }
        supplier.name = supplier_name;
        let mut model_names = std::collections::HashSet::new();
        for model in &mut supplier.models {
            let model_name = model.name.trim().to_string();
            let function_type = model_function_type(model);
            let api_key = model.api_key.trim().to_string();
            model.input_types.retain(|input_type| matches!(input_type.as_str(), "text" | "image"));
            if !model.input_types.iter().any(|input_type| input_type == "text") {
                model.input_types.insert(0, "text".to_string());
            }
            if model.id.trim().is_empty()
                || model_name.chars().count() < 2
                || model_name.chars().count() > 20
                || model.api_format.trim().is_empty()
                || api_key.is_empty()
                || !model.temperature.is_finite()
                || !(0.0..=2.0).contains(&model.temperature)
                || model.max_tokens < 500
                || !matches!(function_type.as_str(), "chat" | "image")
                || !model_names.insert(model_name.to_lowercase())
            {
                return Err(
                    "同一供应商下模型名称必须唯一，名称需为 2-20 个字符，且模型字段必须有效"
                        .to_string(),
                );
            }
            model.name = model_name;
            model.function_type = function_type;
            model.request_path = model.request_path.trim().to_string();
            model.api_key = api_key;
            let mut param_keys = HashSet::new();
            for param in &mut model.params {
                param.key = param.key.trim().to_string();
                param.description = param.description.trim().to_string();
                if param.key.is_empty()
                    || param.description.is_empty()
                    || !param_keys.insert(param.key.clone())
                    || !matches!(param.value_type.as_str(), "string" | "boolean" | "number")
                {
                    return Err("额外参数的键名和描述不能为空且不能重复，字段类型必须是字符串、布尔值或数字".to_string());
                }
                match param.value_type.as_str() {
                    "string" if !param.value.is_string() => {
                        return Err("字符串类型的额外参数值无效".to_string());
                    }
                    "boolean" if !param.value.is_boolean() => {
                        return Err("布尔值类型的额外参数值无效".to_string());
                    }
                    "number" if !param.value.is_number() => {
                        return Err("数字类型的额外参数值无效".to_string());
                    }
                    "string" if param.value.as_str().is_none_or(|value| value.trim().is_empty()) => {
                        return Err("字符串类型的额外参数值不能为空".to_string());
                    }
                    _ => {}
                }
            }
        }
        supplier.config.base_url = supplier.config.base_url.trim().to_string();
        supplier.config.official_address = supplier.config.official_address.trim().to_string();
    }
    let active_exists = settings
        .active_model_id
        .as_deref()
        .map(|active_id| {
            settings.suppliers.iter().any(|supplier| {
                supplier.models.iter().any(|model| {
                    model_config_id(&supplier.name, &model.name) == active_id
                        || model.id == active_id
                })
            })
        })
        .unwrap_or(false);
    if !active_exists {
        settings.active_model_id = settings
            .suppliers
            .first()
            .and_then(|supplier| supplier.models.first())
            .map(|model| model_config_id(&settings.suppliers[0].name, &model.name));
    }
    apply_autostart(settings.autostart)?;
    let mut stored = settings;
    stored.models = Vec::new();
    save_settings_file(&app, &stored)?;
    Ok(settings_view(stored))
}

#[tauri::command]
async fn list_provider_models(
    request: ListProviderModelsRequest,
) -> Result<Vec<ProviderModelView>, String> {
    let base_url = request.base_url.trim().trim_end_matches('/');
    let api_key = request.api_key.trim();
    if base_url.is_empty() || api_key.is_empty() {
        return Err("查询模型列表需要 API 地址和 API Key".to_string());
    }
    let endpoint = format!("{base_url}/models");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("无法创建模型列表请求客户端：{error}"))?;
    let response = client
        .get(&endpoint)
        .bearer_auth(api_key)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| format!("无法连接模型列表接口：{error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("无法读取模型列表响应：{error}"))?;
    if !status.is_success() {
        return Err(api_error(status, &body));
    }
    normalize_provider_models(&body)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImagesApiResult {
    images: Vec<ImageResultView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageResultView {
    data_url: String,
}

#[tauri::command]
async fn generate_image(
    app: AppHandle,
    request: ImagesApiRequest,
) -> Result<ImagesApiResult, String> {
    let prompt = request.prompt.trim();
    if prompt.is_empty() {
        return Err("图片描述不能为空".to_string());
    }
    let (model, _) = active_model_config(&app)?;
    if model_function_type(&model) != "image"
        || !matches!(model.api_format.trim(), "images" | "imagesApi")
    {
        return Err("当前模型未配置图片功能或 Images API".to_string());
    }

    let endpoint = model_endpoint(
        &model.base_url,
        &model.request_path,
        "/images/generations",
    );
    let body = ImagesApiRequestBody {
        model: model.id,
        prompt: prompt.to_string(),
        n: image_result_count(&request.extra_params),
    };
    let body = request_json(&body, &request.extra_params, &["model", "prompt", "n"]);
    debug_log_json_request("images", &endpoint, &body);
    let response = reqwest::Client::new()
        .post(&endpoint)
        .bearer_auth(model.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("Unable to connect to image generation API: {error}"))?;
    let status = response.status();
    let response_body = response
        .text()
        .await
        .map_err(|error| format!("Unable to read image generation response: {error}"))?;
    if !status.is_success() {
        return Err(api_error(status, &response_body));
    }
    let data = parse_images_response(&response_body, "image generation")?;
    let images = image_results(data, "图片生成接口没有返回图片数据").await?;
    Ok(ImagesApiResult { images })
}

#[tauri::command]
async fn edit_image(app: AppHandle, request: ImageEditRequest) -> Result<ImagesApiResult, String> {
    let prompt = request.prompt.trim();
    if prompt.is_empty() {
        return Err("图片编辑描述不能为空".to_string());
    }
    let (model, _) = active_model_config(&app)?;
    if model_function_type(&model) != "image"
        || !matches!(model.api_format.trim(), "images" | "imagesApi")
    {
        return Err("当前模型未配置图片功能或 Images API".to_string());
    }
    let mut image_data_urls = request.image_data_urls;
    if image_data_urls.is_empty() {
        if let Some(image_data_url) = request.image_data_url {
            image_data_urls.push(image_data_url);
        }
    }
    if image_data_urls.is_empty() {
        return Err("至少需要一张参考图片".to_string());
    }
    let mut form = reqwest::multipart::Form::new()
        .text("model", model.id.clone())
        .text("prompt", prompt.to_string())
        .text("n", image_result_count(&request.extra_params).to_string());
    for (index, image_data_url) in image_data_urls.iter().take(3).enumerate() {
        let image_bytes = if let Some((metadata, encoded)) = image_data_url.split_once(",") {
            if !metadata.starts_with("data:image/") {
                return Err("不支持的图片格式".to_string());
            }
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| format!("无法读取原图片：{error}"))?
        } else {
            reqwest::get(image_data_url)
                .await
                .map_err(|error| format!("无法读取原图片：{error}"))?
                .bytes()
                .await
                .map_err(|error| format!("无法读取原图片：{error}"))?
                .to_vec()
        };
        let image_part = reqwest::multipart::Part::bytes(image_bytes)
            .file_name(format!("image-{index}.png"))
            .mime_str("image/png")
            .map_err(|error| format!("无法准备图片文件：{error}"))?;
        form = form.part("image", image_part);
    }
    let form = append_model_params_to_form(form, &request.extra_params);
    let endpoint = model_endpoint(&model.base_url, &model.request_path, "/images/edits");
    debug_log_json_request(
        "images-edit",
        &endpoint,
        &serde_json::json!({
            "model": model.id,
            "prompt": prompt,
            "n": image_result_count(&request.extra_params),
            "extraParams": request.extra_params,
            "image": "[IMAGE_BINARY_REDACTED]"
        }),
    );
    let response = reqwest::Client::new()
        .post(&endpoint)
        .bearer_auth(model.api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("Unable to connect to image edit API: {error}"))?;
    let status = response.status();
    let response_body = response
        .text()
        .await
        .map_err(|error| format!("Unable to read image edit response: {error}"))?;
    if !status.is_success() {
        return Err(api_error(status, &response_body));
    }
    let data = parse_images_response(&response_body, "image edit")?;
    let images = image_results(data, "图片编辑接口没有返回图片数据").await?;
    Ok(ImagesApiResult { images })
}

#[tauri::command]
fn clear_app_data(app: AppHandle) -> Result<(), String> {
    apply_autostart(false)?;

    if let Some(settings) = load_settings_file(&app)? {
        let settings = migrate_settings(settings)?;
        for supplier in &settings.suppliers {
            for model in &supplier.models {
                if let Ok(entry) = keyring_entry(&credential_id(&supplier.name, &model.id)) {
                    let _ = entry.delete_credential();
                }
            }
        }
    }

    let app_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Unable to locate app config directory: {error}"))?;
    for file_name in ["settings.json", "conversations.json", "config.json", "workspace.json"] {
        let path = app_dir.join(file_name);
        if path.exists() {
            fs::remove_file(path).map_err(|error| format!("清除应用数据失败：{error}"))?;
        }
    }
    Ok(())
}

fn conversations_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("conversations.json"))
        .map_err(|error| format!("Unable to locate app config directory: {error}"))
}

fn workspace_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("workspace.json"))
        .map_err(|error| format!("无法定位工作区配置目录：{error}"))
}

fn load_workspace_config_file(app: &AppHandle) -> Result<WorkspaceConfig, String> {
    let path = workspace_path(app)?;
    if !path.exists() {
        return Ok(WorkspaceConfig::default());
    }
    let body = fs::read_to_string(path).map_err(|error| format!("读取工作区配置失败：{error}"))?;
    if body.trim_start().starts_with('{') {
        serde_json::from_str(&body).map_err(|error| format!("工作区配置格式错误：{error}"))
    } else {
        let root = body.trim();
        Ok(WorkspaceConfig {
            root: (!root.is_empty()).then(|| root.to_string()),
            ..WorkspaceConfig::default()
        })
    }
}

fn save_workspace_config_file(app: &AppHandle, config: &WorkspaceConfig) -> Result<(), String> {
    let path = workspace_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建工作区配置目录失败：{error}"))?;
    }
    let body = serde_json::to_string_pretty(config)
        .map_err(|error| format!("保存工作区配置失败：{error}"))?;
    fs::write(path, body).map_err(|error| format!("写入工作区配置失败：{error}"))
}

#[tauri::command]
fn load_workspace_config(app: AppHandle) -> Result<WorkspaceConfig, String> {
    load_workspace_config_file(&app)
}

#[tauri::command]
fn save_workspace_config(app: AppHandle, config: WorkspaceConfig) -> Result<(), String> {
    save_workspace_config_file(&app, &config)
}

#[tauri::command]
fn load_conversations(app: AppHandle) -> Result<Vec<Conversation>, String> {
    let path = conversations_path(&app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let body = fs::read_to_string(&path).map_err(|error| format!("读取历史对话失败：{error}"))?;
    serde_json::from_str(&body).map_err(|error| format!("历史对话格式错误：{error}"))
}

#[tauri::command]
fn save_conversations(app: AppHandle, conversations: Vec<Conversation>) -> Result<(), String> {
    let path = conversations_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建数据目录失败：{error}"))?;
    }

    let body = serde_json::to_string_pretty(&conversations)
        .map_err(|error| format!("保存历史对话失败：{error}"))?;
    fs::write(path, body).map_err(|error| format!("写入历史对话失败：{error}"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceEntry {
    name: String,
    path: String,
    is_directory: bool,
    children: Vec<WorkspaceEntry>,
}

fn current_workspace_root(state: &WorkspaceState) -> Result<PathBuf, String> {
    state
        .root
        .lock()
        .map_err(|_| "工作区状态不可用".to_string())?
        .clone()
        .ok_or_else(|| "请先打开一个工作区".to_string())
}

fn safe_workspace_path(state: &WorkspaceState, relative_path: &str, allow_missing: bool) -> Result<PathBuf, String> {
    let root = current_workspace_root(state)?;
    let relative = Path::new(relative_path);
    if relative_path.trim().is_empty() || relative.is_absolute() {
        return Err("工作区路径无效".to_string());
    }
    if relative.components().any(|component| matches!(component, std::path::Component::ParentDir)) {
        return Err("工作区路径无效".to_string());
    }
    let candidate = root.join(relative);
    let checked = if candidate.exists() {
        candidate
            .canonicalize()
            .map_err(|error| format!("无法解析工作区路径：{error}"))?
    } else if allow_missing {
        let mut existing = candidate.as_path();
        while !existing.exists() {
            existing = existing
                .parent()
                .ok_or_else(|| "工作区路径无效".to_string())?;
        }
        let existing = existing
            .canonicalize()
            .map_err(|error| format!("无法解析工作区父目录：{error}"))?;
        let suffix = candidate
            .strip_prefix(existing.as_path())
            .unwrap_or_else(|_| Path::new(relative_path));
        existing.join(suffix)
    } else {
        return Err("工作区文件不存在".to_string());
    };
    if !checked.starts_with(&root) {
        return Err("工作区路径超出当前工作区范围".to_string());
    }
    Ok(checked)
}

fn ignored_workspace_entry(name: &str) -> bool {
    matches!(name, ".git" | ".idea" | "node_modules" | "target" | "dist" | "coverage")
}

fn workspace_entries(root: &Path, directory: &Path, depth: usize) -> Result<Vec<WorkspaceEntry>, String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("读取工作区目录失败：{error}"))?
        .filter_map(Result::ok)
        .filter(|entry| !ignored_workspace_entry(&entry.file_name().to_string_lossy()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| (!entry.path().is_dir(), entry.file_name()));
    entries
        .into_iter()
        .map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("解析工作区相对路径失败：{error}"))?
                .to_string_lossy()
                .replace('\\', "/");
            let is_directory = path.is_dir();
            let children = if is_directory && depth < 20 {
                workspace_entries(root, &path, depth + 1)?
            } else {
                Vec::new()
            };
            Ok(WorkspaceEntry { name, path: relative, is_directory, children })
        })
        .collect()
}

#[tauri::command]
fn set_workspace_root(app: AppHandle, state: State<'_, WorkspaceState>, path: String) -> Result<String, String> {
    let root = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("无法打开工作区：{error}"))?;
    if !root.is_dir() {
        return Err("工作区必须是文件夹".to_string());
    }
    let mut config = load_workspace_config_file(&app)?;
    config.root = Some(root.to_string_lossy().to_string());
    save_workspace_config_file(&app, &config)?;
    *state.root.lock().map_err(|_| "工作区状态不可用".to_string())? = Some(root.clone());
    Ok(root.to_string_lossy().to_string())
}

#[tauri::command]
fn get_workspace_root(app: AppHandle, state: State<'_, WorkspaceState>) -> Result<Option<String>, String> {
    let mut stored = state.root.lock().map_err(|_| "工作区状态不可用".to_string())?;
    if stored.is_none() {
        if let Ok(config) = load_workspace_config_file(&app) {
            if let Some(path) = config.root {
                let path = PathBuf::from(path.trim());
                if path.is_dir() {
                    *stored = Some(path);
                }
            }
        }
    }
    Ok(stored.as_ref().map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
fn list_workspace_entries(state: State<'_, WorkspaceState>) -> Result<Vec<WorkspaceEntry>, String> {
    let root = current_workspace_root(&state)?;
    workspace_entries(&root, &root, 0)
}

#[tauri::command]
fn read_workspace_file(state: State<'_, WorkspaceState>, path: String) -> Result<String, String> {
    let file = safe_workspace_path(&state, &path, false)?;
    if !file.is_file() {
        return Err("工作区路径不是文件".to_string());
    }
    let metadata = fs::metadata(&file).map_err(|error| format!("读取文件信息失败：{error}"))?;
    if metadata.len() > 5 * 1024 * 1024 {
        return Err("文件超过 5 MB，暂不支持在编辑器中打开".to_string());
    }
    fs::read_to_string(file).map_err(|error| format!("读取工作区文件失败：{error}"))
}

#[tauri::command]
fn write_workspace_file(state: State<'_, WorkspaceState>, path: String, content: String) -> Result<(), String> {
    let file = safe_workspace_path(&state, &path, true)?;
    if file.exists() && !file.is_file() {
        return Err("工作区路径不是文件".to_string());
    }
    fs::write(file, content).map_err(|error| format!("保存工作区文件失败：{error}"))
}

#[tauri::command]
fn create_workspace_file(state: State<'_, WorkspaceState>, path: String) -> Result<(), String> {
    let file = safe_workspace_path(&state, &path, true)?;
    if file.exists() {
        return Err("文件已经存在".to_string());
    }
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建文件目录失败：{error}"))?;
    }
    fs::File::create(file).map_err(|error| format!("创建工作区文件失败：{error}"))?;
    Ok(())
}

#[tauri::command]
fn delete_workspace_file(state: State<'_, WorkspaceState>, path: String) -> Result<(), String> {
    let file = safe_workspace_path(&state, &path, false)?;
    if file.is_dir() {
        return fs::remove_dir_all(file).map_err(|error| format!("删除工作区文件夹失败：{error}"));
    }
    fs::remove_file(file).map_err(|error| format!("删除工作区文件失败：{error}"))
}

#[tauri::command]
fn create_workspace_directory(state: State<'_, WorkspaceState>, path: String) -> Result<(), String> {
    let directory = safe_workspace_path(&state, &path, true)?;
    if directory.exists() {
        return Err("文件夹已经存在".to_string());
    }
    fs::create_dir_all(directory).map_err(|error| format!("创建工作区文件夹失败：{error}"))
}

fn workspace_child_path(state: &WorkspaceState, path: &str, name: &str) -> Result<PathBuf, String> {
    if name.trim().is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("名称无效".to_string());
    }
    let source = safe_workspace_path(state, path, false)?;
    let parent = source.parent().ok_or_else(|| "工作区路径无效".to_string())?;
    let root = current_workspace_root(state)?;
    let parent = parent.canonicalize().map_err(|error| format!("无法解析目标目录：{error}"))?;
    if !parent.starts_with(&root) {
        return Err("目标路径超出当前工作区范围".to_string());
    }
    let target = parent.join(name);
    if target.exists() {
        return Err("目标名称已经存在".to_string());
    }
    Ok(target)
}

#[tauri::command]
fn rename_workspace_entry(state: State<'_, WorkspaceState>, path: String, name: String) -> Result<(), String> {
    let source = safe_workspace_path(&state, &path, false)?;
    let target = workspace_child_path(&state, &path, name.trim())?;
    fs::rename(source, target).map_err(|error| format!("重命名工作区条目失败：{error}"))
}

fn copy_workspace_entry(source: &Path, target: &Path) -> Result<(), String> {
    if source.is_dir() {
        fs::create_dir(target).map_err(|error| format!("创建复制目录失败：{error}"))?;
        for entry in fs::read_dir(source).map_err(|error| format!("读取复制目录失败：{error}"))? {
            let entry = entry.map_err(|error| format!("读取复制条目失败：{error}"))?;
            copy_workspace_entry(&entry.path(), &target.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(source, target)
            .map(|_| ())
            .map_err(|error| format!("复制工作区条目失败：{error}"))
    }
}

#[tauri::command]
fn paste_workspace_entry(
    state: State<'_, WorkspaceState>,
    source: String,
    destination: String,
    cut: bool,
) -> Result<(), String> {
    let source_path = safe_workspace_path(&state, &source, false)?;
    let root = current_workspace_root(&state)?;
    let destination_path = if destination.trim().is_empty() {
        root.clone()
    } else {
        safe_workspace_path(&state, &destination, false)?
    };
    if !destination_path.is_dir() {
        return Err("粘贴目标不是文件夹".to_string());
    }
    let name = source_path.file_name().ok_or_else(|| "工作区路径无效".to_string())?;
    let target = destination_path.join(name);
    if target.exists() {
        return Err("目标文件夹中已经存在同名条目".to_string());
    }
    if target.starts_with(&source_path) {
        return Err("不能将文件夹粘贴到自身内部".to_string());
    }
    if cut {
        fs::rename(source_path, target).map_err(|error| format!("移动工作区条目失败：{error}"))
    } else {
        copy_workspace_entry(&source_path, &target)
    }
}

#[tauri::command]
fn open_workspace_in_explorer(state: State<'_, WorkspaceState>, path: Option<String>) -> Result<(), String> {
    let target = match path.filter(|value| !value.trim().is_empty()) {
        Some(path) => safe_workspace_path(&state, &path, false)?,
        None => current_workspace_root(&state)?,
    };
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("explorer");
        if target.is_file() {
            command.arg(format!("/select,{}", target.display()));
        } else {
            command.arg(target.as_os_str());
        }
        command.spawn().map_err(|error| format!("打开资源管理器失败：{error}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(target).spawn().map_err(|error| format!("打开文件管理器失败：{error}"))?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(target).spawn().map_err(|error| format!("打开文件管理器失败：{error}"))?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("当前平台暂不支持打开文件管理器".to_string())
}

#[derive(Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<ChatCompletionsMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    stream: bool,
}

fn merge_model_params(
    mut body: serde_json::Value,
    params: &serde_json::Map<String, serde_json::Value>,
    protected_keys: &[&str],
) -> serde_json::Value {
    let Some(object) = body.as_object_mut() else {
        return body;
    };
    for (key, value) in params {
        let key = key.trim();
        if key.is_empty() || protected_keys.contains(&key) || object.contains_key(key) {
            continue;
        }
        object.insert(key.to_string(), value.clone());
    }
    body
}

fn request_json<T: Serialize>(
    request: &T,
    params: &serde_json::Map<String, serde_json::Value>,
    protected_keys: &[&str],
) -> serde_json::Value {
    merge_model_params(
        serde_json::to_value(request).unwrap_or_else(|_| serde_json::json!({})),
        params,
        protected_keys,
    )
}

fn debug_log_json_request(protocol: &str, endpoint: &str, body: &serde_json::Value) {
    #[cfg(debug_assertions)]
    {
        let mut safe_body = body.clone();
        redact_debug_value(&mut safe_body, None);
        let payload = serde_json::to_string_pretty(&safe_body)
            .unwrap_or_else(|_| "<无法序列化请求体>".to_string());
        eprintln!("[ai-fast request] protocol={protocol} endpoint={endpoint}\n{payload}");
    }
}

fn redact_debug_value(value: &mut serde_json::Value, key: Option<&str>) {
    match value {
        serde_json::Value::Object(object) => {
            for (child_key, child_value) in object.iter_mut() {
                if is_sensitive_debug_key(child_key) {
                    *child_value = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    redact_debug_value(child_value, Some(child_key));
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_debug_value(item, key);
            }
        }
        serde_json::Value::String(text) if key == Some("url") && text.starts_with("data:") => {
            *text = "[IMAGE_DATA_REDACTED]".to_string();
        }
        _ => {}
    }
}

fn is_sensitive_debug_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "authorization",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|part| key.contains(part))
}

fn append_model_params_to_form(
    mut form: reqwest::multipart::Form,
    params: &serde_json::Map<String, serde_json::Value>,
) -> reqwest::multipart::Form {
    for (key, value) in params {
        let key = key.trim();
        if key.is_empty() || matches!(key, "image" | "model" | "prompt" | "n") {
            continue;
        }
        let value = match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Bool(value) => Some(value.to_string()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            _ => None,
        };
        if let Some(value) = value {
            form = form.text(key.to_string(), value);
        }
    }
    form
}

#[derive(Serialize)]
struct ChatCompletionsMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Serialize)]
struct ImagesApiRequestBody {
    model: String,
    prompt: String,
    n: u32,
}

#[derive(Deserialize)]
struct ImagesApiResponse {
    data: Option<Vec<ImagesApiItem>>,
}

#[derive(Deserialize)]
struct ImagesApiItem {
    url: Option<String>,
    b64_json: Option<String>,
}

fn image_result_count(params: &serde_json::Map<String, serde_json::Value>) -> u32 {
    params
        .get("n")
        .and_then(|value| value.as_u64())
        .unwrap_or(1)
        .clamp(1, 4) as u32
}

async fn image_results(
    response: ImagesApiResponse,
    empty_message: &str,
) -> Result<Vec<ImageResultView>, String> {
    let items = response.data.unwrap_or_default();
    if items.is_empty() {
        return Err(empty_message.to_string());
    }
    let mut images = Vec::with_capacity(items.len());
    for item in items {
        images.push(ImageResultView {
            data_url: image_item_data_url(item).await?,
        });
    }
    Ok(images)
}

fn response_preview(body: &str) -> String {
    let preview = body.trim().chars().take(240).collect::<String>();
    if preview.is_empty() {
        "<empty response>".to_string()
    } else {
        preview
    }
}

fn parse_images_response(body: &str, operation: &str) -> Result<ImagesApiResponse, String> {
    serde_json::from_str(body).map_err(|error| {
        format!(
            "Unable to parse {operation} response: {error}. Response preview: {}",
            response_preview(body)
        )
    })
}

async fn image_item_data_url(item: ImagesApiItem) -> Result<String, String> {
    if let Some(data_url) = item.b64_json {
        return Ok(if data_url.starts_with("data:image/") {
            data_url
        } else {
            format!("data:image/png;base64,{data_url}")
        });
    }

    let url = item
        .url
        .ok_or_else(|| "图片接口没有返回 URL 或 base64 图片数据".to_string())?;
    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0 Safari/537.36")
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            eprintln!("[ai-fast image] 无法创建图片下载客户端，将返回远程 URL：{error}");
            return Ok(url);
        }
    };
    let response = match client
        .get(&url)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            eprintln!("[ai-fast image] 无法下载生成的图片，将返回远程 URL：{error:?}");
            return Ok(url);
        }
    };
    let status = response.status();
    if !status.is_success() {
        eprintln!("[ai-fast image] 图片下载返回 HTTP {status}，将返回远程 URL");
        return Ok(url);
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("image/"))
        .unwrap_or("image/png")
        .split(';')
        .next()
        .unwrap_or("image/png")
        .to_string();
    let bytes = match response
        .bytes()
        .await
    {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("[ai-fast image] 无法读取生成的图片，将返回远程 URL：{error}");
            return Ok(url);
        }
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[derive(Deserialize)]
struct ApiResponse {
    choices: Option<Vec<ApiChoice>>,
}

#[derive(Deserialize)]
struct ApiChoice {
    message: Option<ApiMessage>,
    delta: Option<ApiDelta>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ApiDelta {
    content: Option<String>,
}

#[derive(Serialize)]
struct ResponsesRequest {
    model: String,
    instructions: String,
    input: Vec<ResponsesInputMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ResponsesReasoning>,
    stream: bool,
}

#[derive(Serialize)]
struct ResponsesInputMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    system: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

fn chat_completions_message(message: &ChatMessage) -> ChatCompletionsMessage {
    let content = if message.images.is_empty() {
        serde_json::Value::String(message.content.clone())
    } else {
        let mut parts = Vec::new();
        if !message.content.trim().is_empty() {
            parts.push(serde_json::json!({ "type": "text", "text": message.content }));
        }
        parts.extend(message.images.iter().map(|image| {
            serde_json::json!({
                "type": "image_url",
                "image_url": { "url": image.data_url }
            })
        }));
        serde_json::Value::Array(parts)
    };
    ChatCompletionsMessage {
        role: message.role.clone(),
        content,
    }
}

fn responses_input_message(message: &ChatMessage) -> ResponsesInputMessage {
    let content = if message.images.is_empty() {
        serde_json::Value::String(message.content.clone())
    } else {
        let mut parts = Vec::new();
        if !message.content.trim().is_empty() {
            parts.push(serde_json::json!({ "type": "input_text", "text": message.content }));
        }
        parts.extend(message.images.iter().map(|image| {
            serde_json::json!({
                "type": "input_image",
                "image_url": image.data_url
            })
        }));
        serde_json::Value::Array(parts)
    };
    ResponsesInputMessage {
        role: message.role.clone(),
        content,
    }
}

fn anthropic_message(message: &ChatMessage) -> AnthropicMessage {
    let content = if message.images.is_empty() {
        serde_json::Value::String(message.content.clone())
    } else {
        let mut parts = Vec::new();
        if !message.content.trim().is_empty() {
            parts.push(serde_json::json!({ "type": "text", "text": message.content }));
        }
        for image in &message.images {
            if let Some((metadata, encoded)) = image.data_url.split_once(',') {
                let media_type = metadata
                    .strip_prefix("data:")
                    .and_then(|value| value.strip_suffix(";base64"))
                    .unwrap_or("image/png");
                parts.push(serde_json::json!({
                    "type": "image",
                    "source": { "type": "base64", "media_type": media_type, "data": encoded }
                }));
            }
        }
        serde_json::Value::Array(parts)
    };
    AnthropicMessage {
        role: if message.role == "assistant" { "assistant" } else { "user" }.to_string(),
        content,
    }
}

#[derive(Serialize)]
struct ResponsesReasoning {
    effort: String,
}

#[derive(Deserialize)]
struct ResponsesApiResponse {
    output_text: Option<String>,
    output: Option<Vec<ResponsesOutputItem>>,
}

#[derive(Deserialize)]
struct ResponsesOutputItem {
    content: Option<Vec<ResponsesOutputContent>>,
}

#[derive(Deserialize)]
struct ResponsesOutputContent {
    text: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamEvent {
    stream_id: String,
    delta: String,
    done: bool,
}

fn emit_stream_event(
    app: &AppHandle,
    stream_id: &str,
    delta: String,
    done: bool,
) -> Result<(), String> {
    app.emit(
        "chat-stream",
        ChatStreamEvent {
            stream_id: stream_id.to_string(),
            delta,
            done,
        },
    )
    .map_err(|error| format!("发送流式响应失败：{error}"))
}

fn responses_content(data: ResponsesApiResponse) -> Option<String> {
    if let Some(content) = data.output_text.filter(|content| !content.is_empty()) {
        return Some(content);
    }
    let content = data
        .output
        .unwrap_or_default()
        .into_iter()
        .flat_map(|item| item.content.unwrap_or_default())
        .filter_map(|item| item.text)
        .collect::<String>();
    (!content.is_empty()).then_some(content)
}

fn response_content(data: ApiResponse) -> Option<String> {
    data.choices
        .and_then(|mut choices| choices.drain(..).next())
        .and_then(|choice| {
            choice
                .message
                .and_then(|message| message.content)
                .or(choice.text)
                .or_else(|| choice.delta.and_then(|delta| delta.content))
        })
        .filter(|content| !content.is_empty())
}

fn api_error(status: reqwest::StatusCode, body: &str) -> String {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|data| {
            data.pointer("/error/message")
                .or_else(|| data.get("message"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.to_string());
    format!("API returned {status}: {message}")
}

fn model_field<'a>(model: &'a serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<&'a serde_json::Value> {
    keys.iter().find_map(|key| model.get(*key))
}

fn normalized_provider_model(model: &serde_json::Value) -> Option<ProviderModelView> {
    let model = model.as_object()?;
    let id = model_field(model, &["id", "baseModelId", "base_model_id", "name"])
        .and_then(serde_json::Value::as_str)
        .map(|value| value.strip_prefix("models/").unwrap_or(value).to_string())?;
    if id.trim().is_empty() {
        return None;
    }
    let name = model_field(model, &["name", "displayName", "display_name"])
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&id)
        .to_string();
    let owned_by = model_field(model, &["owned_by", "ownedBy", "owner"])
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let created = model_field(model, &["created", "created_at", "createdAt"]).cloned();
    Some(ProviderModelView {
        id,
        name,
        owned_by,
        created,
    })
}

fn normalize_provider_models(body: &str) -> Result<Vec<ProviderModelView>, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| format!("无法解析模型列表响应：{error}"))?;
    let items = value
        .get("data")
        .or_else(|| value.get("models"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "模型列表响应缺少 data 或 models 数组".to_string())?;
    let mut models = Vec::new();
    let mut ids = HashSet::new();
    for item in items {
        if let Some(model) = normalized_provider_model(item) {
            if ids.insert(model.id.clone()) {
                models.push(model);
            }
        }
    }
    Ok(models)
}

fn sse_frame_end(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| (position, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) if left.0 < right.0 => Some(left),
        (Some(_), Some(right)) => Some(right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

async fn send_non_streaming(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    request: &ChatCompletionsRequest,
    params: &serde_json::Map<String, serde_json::Value>,
    state: &ChatState,
    stream_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<String, String> {
    if *cancellation.borrow() {
        return Err(CHAT_CANCELED.to_string());
    }
    let body = request_json(request, params, &["model", "messages", "stream"]);
    debug_log_json_request("chat-completions", endpoint, &body);
    let response = tokio::select! {
        result = client.post(endpoint).bearer_auth(api_key).json(&body).send() => result
            .map_err(|error| format!("Unable to connect to model API: {error}"))?,
        result = cancellation.changed() => {
            let _ = result;
            return Err(CHAT_CANCELED.to_string());
        }
    };
    state.ensure_active(stream_id)?;
    let status = response.status();
    let body = tokio::select! {
        result = response.text() => result
            .map_err(|error| format!("Unable to read API response: {error}"))?,
        result = cancellation.changed() => {
            let _ = result;
            return Err(CHAT_CANCELED.to_string());
        }
    };
    state.ensure_active(stream_id)?;
    if !status.is_success() {
        return Err(api_error(status, &body));
    }
    let data: ApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Unable to parse API response: {error}"))?;
    response_content(data).ok_or_else(|| "API returned no reply content.".to_string())
}

async fn send_responses_non_streaming(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    request: &ResponsesRequest,
    params: &serde_json::Map<String, serde_json::Value>,
    state: &ChatState,
    stream_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<String, String> {
    if *cancellation.borrow() {
        return Err(CHAT_CANCELED.to_string());
    }
    let body = request_json(
        request,
        params,
        &["model", "instructions", "input", "stream"],
    );
    debug_log_json_request("responses", endpoint, &body);
    let response = tokio::select! {
        result = client.post(endpoint).bearer_auth(api_key).json(&body).send() => result
            .map_err(|error| format!("Unable to connect to model API: {error}"))?,
        result = cancellation.changed() => {
            let _ = result;
            return Err(CHAT_CANCELED.to_string());
        }
    };
    state.ensure_active(stream_id)?;
    let status = response.status();
    let body = tokio::select! {
        result = response.text() => result
            .map_err(|error| format!("Unable to read API response: {error}"))?,
        result = cancellation.changed() => {
            let _ = result;
            return Err(CHAT_CANCELED.to_string());
        }
    };
    state.ensure_active(stream_id)?;
    if !status.is_success() {
        return Err(api_error(status, &body));
    }
    let data: ResponsesApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Unable to parse Responses API response: {error}"))?;
    responses_content(data).ok_or_else(|| "Responses API returned no reply content.".to_string())
}

async fn chat_completion_chat_completions(
    app: AppHandle,
    state: &ChatState,
    request: ChatRequest,
    config: ModelProfile,
    language: String,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<String, String> {
    let endpoint = model_endpoint(
        &config.base_url,
        &config.request_path,
        "/chat/completions",
    );
    let request_params = request.extra_params.clone();
    let stream_id = request
        .stream_id
        .unwrap_or_else(|| format!("stream-{}", chrono_like_timestamp()));
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(ChatMessage {
        role: "system".to_string(),
        content: system_prompt_for(&language).to_string(),
        images: Vec::new(),
    });
    messages.extend(request.messages);

    let client = reqwest::Client::new();
    let stream_request = ChatCompletionsRequest {
        model: config.id.clone(),
        messages: messages.iter().map(chat_completions_message).collect(),
        reasoning_effort: request.reasoning_effort.clone(),
        stream: true,
    };
    let body = request_json(
        &stream_request,
        &request_params,
        &["model", "messages", "stream"],
    );
    debug_log_json_request("chat-completions", &endpoint, &body);
    if *cancellation.borrow() {
        return Err(CHAT_CANCELED.to_string());
    }
    let response = tokio::select! {
        result = client.post(&endpoint).bearer_auth(&config.api_key).json(&body).send() => result
            .map_err(|error| format!("Unable to connect to model API: {error}"))?,
        result = cancellation.changed() => {
            let _ = result;
            return Err(CHAT_CANCELED.to_string());
        }
    };

    let status = response.status();
    if !status.is_success() {
        let fallback_request = ChatCompletionsRequest {
            stream: false,
            ..stream_request
        };
        return send_non_streaming(
            &client,
            &endpoint,
            &config.api_key,
            &fallback_request,
            &request_params,
            state,
            &stream_id,
            cancellation,
        )
        .await;
    }

    let is_event_stream = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false);

    if !is_event_stream {
        state.ensure_active(&stream_id)?;
        let body = tokio::select! {
            result = response.text() => result
                .map_err(|error| format!("Unable to read API response: {error}"))?,
            result = cancellation.changed() => {
                let _ = result;
                return Err(CHAT_CANCELED.to_string());
            }
        };
        state.ensure_active(&stream_id)?;
        let data: ApiResponse = serde_json::from_str(&body)
            .map_err(|error| format!("Unable to parse API response: {error}"))?;
        let content =
            response_content(data).ok_or_else(|| "API returned no reply content.".to_string())?;
        emit_stream_event(&app, &stream_id, content.clone(), true)?;
        return Ok(content);
    }

    let mut response = response;
    let mut pending = Vec::new();
    let mut answer = String::new();
    loop {
        let chunk = tokio::select! {
            result = response.chunk() => result
                .map_err(|error| format!("Unable to read streaming API response: {error}"))?,
            result = cancellation.changed() => {
                let _ = result;
                return Err(CHAT_CANCELED.to_string());
            }
        };
        let Some(chunk) = chunk else { break };
        state.ensure_active(&stream_id)?;
        pending.extend_from_slice(&chunk);
        while let Some((position, delimiter_length)) = sse_frame_end(&pending) {
            let frame = pending
                .drain(..position + delimiter_length)
                .collect::<Vec<_>>();
            let frame = String::from_utf8_lossy(&frame);
            for line in frame.lines() {
                let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                    continue;
                };
                if data == "[DONE]" {
                    continue;
                }
                let chunk: ApiResponse = serde_json::from_str(data)
                    .map_err(|error| format!("Unable to parse streaming API response: {error}"))?;
                if let Some(delta) = response_content(chunk) {
                    answer.push_str(&delta);
                    emit_stream_event(&app, &stream_id, delta, false)?;
                }
            }
        }
    }
    if !pending.is_empty() {
        let frame = String::from_utf8_lossy(&pending);
        for line in frame.lines() {
            let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let chunk: ApiResponse = serde_json::from_str(data)
                .map_err(|error| format!("Unable to parse streaming API response: {error}"))?;
            if let Some(delta) = response_content(chunk) {
                answer.push_str(&delta);
                emit_stream_event(&app, &stream_id, delta, false)?;
            }
        }
    }

    if answer.is_empty() {
        return Err("API returned no reply content.".to_string());
    }
    emit_stream_event(&app, &stream_id, String::new(), true)?;
    Ok(answer)
}

fn emit_responses_sse_frame(
    app: &AppHandle,
    stream_id: &str,
    frame: &str,
    answer: &mut String,
) -> Result<(), String> {
    for line in frame.lines() {
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" {
            continue;
        }
        let event = serde_json::from_str::<serde_json::Value>(data).map_err(|error| {
            format!("Unable to parse Responses streaming API response: {error}")
        })?;
        if event.get("type").and_then(serde_json::Value::as_str)
            != Some("response.output_text.delta")
        {
            continue;
        }
        let Some(delta) = event.get("delta").and_then(serde_json::Value::as_str) else {
            continue;
        };
        answer.push_str(delta);
        emit_stream_event(app, stream_id, delta.to_string(), false)?;
    }
    Ok(())
}

async fn chat_completion_responses(
    app: AppHandle,
    state: &ChatState,
    request: ChatRequest,
    config: ModelProfile,
    language: String,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<String, String> {
    let endpoint = model_endpoint(&config.base_url, &config.request_path, "/responses");
    let request_params = request.extra_params.clone();
    let stream_id = request
        .stream_id
        .unwrap_or_else(|| format!("stream-{}", chrono_like_timestamp()));
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(ChatMessage {
        role: "system".to_string(),
        content: system_prompt_for(&language).to_string(),
        images: Vec::new(),
    });
    messages.extend(request.messages);
    let instructions = messages
        .iter()
        .find(|message| message.role == "system")
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let input = messages
        .into_iter()
        .filter(|message| message.role != "system")
        .map(|message| responses_input_message(&message))
        .collect::<Vec<_>>();
    let reasoning = request
        .reasoning_effort
        .map(|effort| ResponsesReasoning { effort });
    let client = reqwest::Client::new();
    let stream_request = ResponsesRequest {
        model: config.id.clone(),
        instructions,
        input,
        reasoning,
        stream: true,
    };
    let body = request_json(
        &stream_request,
        &request_params,
        &["model", "instructions", "input", "stream"],
    );
    debug_log_json_request("responses", &endpoint, &body);
    if *cancellation.borrow() {
        return Err(CHAT_CANCELED.to_string());
    }
    let response = tokio::select! {
        result = client.post(&endpoint).bearer_auth(&config.api_key).json(&body).send() => result
            .map_err(|error| format!("Unable to connect to Responses API: {error}"))?,
        result = cancellation.changed() => {
            let _ = result;
            return Err(CHAT_CANCELED.to_string());
        }
    };
    let status = response.status();
    if !status.is_success() {
        let fallback_request = ResponsesRequest {
            stream: false,
            ..stream_request
        };
        return send_responses_non_streaming(
            &client,
            &endpoint,
            &config.api_key,
            &fallback_request,
            &request_params,
            state,
            &stream_id,
            cancellation,
        )
        .await;
    }

    let is_event_stream = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false);
    if !is_event_stream {
        state.ensure_active(&stream_id)?;
        let body = tokio::select! {
            result = response.text() => result
                .map_err(|error| format!("Unable to read Responses API response: {error}"))?,
            result = cancellation.changed() => {
                let _ = result;
                return Err(CHAT_CANCELED.to_string());
            }
        };
        state.ensure_active(&stream_id)?;
        let data: ResponsesApiResponse = serde_json::from_str(&body)
            .map_err(|error| format!("Unable to parse Responses API response: {error}"))?;
        let content = responses_content(data)
            .ok_or_else(|| "Responses API returned no reply content.".to_string())?;
        emit_stream_event(&app, &stream_id, content.clone(), true)?;
        return Ok(content);
    }

    let mut response = response;
    let mut pending = Vec::new();
    let mut answer = String::new();
    loop {
        let chunk = tokio::select! {
            result = response.chunk() => result
                .map_err(|error| format!("Unable to read Responses streaming API response: {error}"))?,
            result = cancellation.changed() => {
                let _ = result;
                return Err(CHAT_CANCELED.to_string());
            }
        };
        let Some(chunk) = chunk else { break };
        state.ensure_active(&stream_id)?;
        pending.extend_from_slice(&chunk);
        while let Some((position, delimiter_length)) = sse_frame_end(&pending) {
            let frame = pending
                .drain(..position + delimiter_length)
                .collect::<Vec<_>>();
            emit_responses_sse_frame(
                &app,
                &stream_id,
                &String::from_utf8_lossy(&frame),
                &mut answer,
            )?;
        }
    }
    if !pending.is_empty() {
        emit_responses_sse_frame(
            &app,
            &stream_id,
            &String::from_utf8_lossy(&pending),
            &mut answer,
        )?;
    }
    if answer.is_empty() {
        return Err("Responses API returned no reply content.".to_string());
    }
    emit_stream_event(&app, &stream_id, String::new(), true)?;
    Ok(answer)
}

fn anthropic_response_content(body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| format!("Unable to parse Anthropic response: {error}"))?;
    let content = value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
        .collect::<String>();
    if content.is_empty() {
        Err("Anthropic API returned no reply content.".to_string())
    } else {
        Ok(content)
    }
}

fn emit_anthropic_sse_frame(
    app: &AppHandle,
    stream_id: &str,
    frame: &str,
    answer: &mut String,
) -> Result<(), String> {
    let mut event_name = "";
    for line in frame.lines() {
        if let Some(value) = line.strip_prefix("event:").map(str::trim) {
            event_name = value;
            continue;
        }
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if event_name != "content_block_delta" {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(data)
            .map_err(|error| format!("Unable to parse Anthropic streaming response: {error}"))?;
        let Some(delta) = value
            .get("delta")
            .and_then(|delta| delta.get("text"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        answer.push_str(delta);
        emit_stream_event(app, stream_id, delta.to_string(), false)?;
    }
    Ok(())
}

async fn chat_completion_anthropic(
    app: AppHandle,
    state: &ChatState,
    request: ChatRequest,
    config: ModelProfile,
    language: String,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<String, String> {
    let endpoint = model_endpoint(&config.base_url, &config.request_path, "/v1/messages");
    let stream_id = request
        .stream_id
        .clone()
        .unwrap_or_else(|| format!("stream-{}", chrono_like_timestamp()));
    let mut messages = Vec::with_capacity(request.messages.len());
    for message in request.messages {
        if message.role != "system" {
            messages.push(anthropic_message(&message));
        }
    }
    let system = system_prompt_for(&language).to_string();
    let anthropic_request = AnthropicRequest {
        model: config.id,
        system,
        messages,
        max_tokens: 4096,
        stream: true,
    };
    let body = request_json(
        &anthropic_request,
        &request.extra_params,
        &["model", "system", "messages", "stream"],
    );
    debug_log_json_request("anthropic-messages", &endpoint, &body);
    let client = reqwest::Client::new();
    let response = tokio::select! {
        result = client
            .post(&endpoint)
            .header("x-api-key", &config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send() => result
            .map_err(|error| format!("Unable to connect to Anthropic API: {error}"))?,
        result = cancellation.changed() => {
            let _ = result;
            return Err(CHAT_CANCELED.to_string());
        }
    };
    let status = response.status();
    if !status.is_success() {
        let response_body = response
            .text()
            .await
            .map_err(|error| format!("Unable to read Anthropic response: {error}"))?;
        return Err(api_error(status, &response_body));
    }
    let is_event_stream = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false);
    if !is_event_stream {
        let response_body = response
            .text()
            .await
            .map_err(|error| format!("Unable to read Anthropic response: {error}"))?;
        let answer = anthropic_response_content(&response_body)?;
        emit_stream_event(&app, &stream_id, answer.clone(), true)?;
        return Ok(answer);
    }
    let mut response = response;
    let mut pending = Vec::new();
    let mut answer = String::new();
    loop {
        let chunk = tokio::select! {
            result = response.chunk() => result
                .map_err(|error| format!("Unable to read Anthropic streaming response: {error}"))?,
            result = cancellation.changed() => {
                let _ = result;
                return Err(CHAT_CANCELED.to_string());
            }
        };
        let Some(chunk) = chunk else { break };
        state.ensure_active(&stream_id)?;
        pending.extend_from_slice(&chunk);
        while let Some((position, delimiter_length)) = sse_frame_end(&pending) {
            let frame = pending
                .drain(..position + delimiter_length)
                .collect::<Vec<_>>();
            emit_anthropic_sse_frame(
                &app,
                &stream_id,
                &String::from_utf8_lossy(&frame),
                &mut answer,
            )?;
        }
    }
    if !pending.is_empty() {
        emit_anthropic_sse_frame(
            &app,
            &stream_id,
            &String::from_utf8_lossy(&pending),
            &mut answer,
        )?;
    }
    if answer.is_empty() {
        return Err("Anthropic API returned no reply content.".to_string());
    }
    emit_stream_event(&app, &stream_id, String::new(), true)?;
    Ok(answer)
}

#[tauri::command]
async fn chat_completion(
    app: AppHandle,
    state: State<'_, ChatState>,
    mut request: ChatRequest,
) -> Result<String, String> {
    let stream_id = request
        .stream_id
        .clone()
        .unwrap_or_else(|| format!("stream-{}", chrono_like_timestamp()));
    request.stream_id = Some(stream_id.clone());
    let mut cancellation = state.start(&stream_id);
    let (config, language) = model_config_for_id(&app, request.model_id.as_deref())?;
    if request.messages.iter().any(|message| !message.images.is_empty())
        && !model_supports_input_type(&config, "image")
    {
        state.finish(&stream_id);
        return Err("当前模型未配置图片输入".to_string());
    }
    let result = match config.api_format.trim() {
        "responses" | "responsesApi" => {
            chat_completion_responses(app, &state, request, config, language, &mut cancellation)
                .await
        }
        "anthropicMessages" | "anthropic" => {
            chat_completion_anthropic(app, &state, request, config, language, &mut cancellation)
                .await
        }
        "" | "chatCompletions" => {
            chat_completion_chat_completions(
                app,
                &state,
                request,
                config,
                language,
                &mut cancellation,
            )
            .await
        }
        format => Err(format!("暂不支持的 API 格式：{format}")),
    };
    state.finish(&stream_id);
    result
}

#[tauri::command]
fn cancel_chat(state: State<'_, ChatState>, stream_id: String) {
    state.cancel(&stream_id);
}

#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    window.show().map_err(|error| format!("显示主窗口失败：{error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("聚焦主窗口失败：{error}"))
}

fn chrono_like_timestamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(ChatState::default())
        .manage(WorkspaceState::default())
        .setup(|app| {
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出应用", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let main_window = app
                .get_webview_window("main")
                .ok_or_else(|| "找不到主窗口".to_string())?;
            let window_for_close = main_window.clone();
            main_window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_for_close.hide();
                }
            });
            TrayIconBuilder::with_id("main-tray")
                .icon(tauri::include_image!("icons/icon.png"))
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            chat_completion,
            cancel_chat,
            show_main_window,
            load_conversations,
            save_conversations,
            load_settings,
            save_settings,
            list_provider_models,
            generate_image,
            edit_image,
            clear_app_data,
            load_workspace_config,
            save_workspace_config,
            set_workspace_root,
            get_workspace_root,
            list_workspace_entries,
            read_workspace_file,
            write_workspace_file,
            create_workspace_file,
            delete_workspace_file,
            create_workspace_directory,
            rename_workspace_entry,
            paste_workspace_entry,
            open_workspace_in_explorer
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
