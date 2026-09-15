use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    base_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ApiResponse {
    choices: Option<Vec<ApiChoice>>,
}

#[derive(Deserialize)]
struct ApiChoice {
    message: Option<ApiMessage>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiMessage {
    content: Option<String>,
}

#[tauri::command]
async fn chat_completion(request: ChatRequest) -> Result<String, String> {
    let endpoint = format!("{}/chat/completions", request.base_url.trim_end_matches('/'));
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    if !request.system_prompt.trim().is_empty() {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: request.system_prompt,
        });
    }
    messages.extend(request.messages);

    let response = reqwest::Client::new()
        .post(endpoint)
        .bearer_auth(request.api_key)
        .json(&ApiRequest {
            model: request.model,
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        })
        .send()
        .await
        .map_err(|error| format!("Unable to connect to model API: {error}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Unable to read API response: {error}"))?;

    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|data| {
                data.pointer("/error/message")
                    .or_else(|| data.get("message"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or(body);
        return Err(format!("API returned {status}: {message}"));
    }

    let data: ApiResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Unable to parse API response: {error}"))?;

    data.choices
        .and_then(|mut choices| choices.drain(..).next())
        .and_then(|choice| choice.message.and_then(|message| message.content).or(choice.text))
        .filter(|content| !content.is_empty())
        .ok_or_else(|| "API returned no reply content.".to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![chat_completion])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
