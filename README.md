# AI-fast

Vue 3 and Tauri 2 desktop AI chat client.

## Development

1. Install Node.js 20 LTS or newer, Rust stable, and Microsoft C++ Build Tools.
2. Run `npm install`.
3. Run `npm run tauri:dev`.
4. Open the in-app settings to configure models and application preferences.

## Windows build

Run `npm run tauri:build`. The NSIS installer will be under `src-tauri/target/release/bundle/nsis/`.

To replace the packaged application icon, prepare a square PNG (1024x1024 or larger) and run `npm run tauri -- icon path/to/app-icon.png`. This regenerates the files in `src-tauri/icons/`, which are referenced by the Tauri bundle configuration. Then run `npm run tauri:build` again. Windows may cache an old executable icon, so test the newly generated installer or clear the shortcut/icon cache if an existing shortcut still shows the old image.

Vue owns the chat UI and current session state. Conversation history is stored in `conversations.json` in the app config directory, and Rust exposes `load_conversations` and `save_conversations` commands for persistence. The Rust `chat_completion` command owns outbound HTTP requests, so the WebView does not make cross-origin calls directly.

Application settings and non-secret model metadata are stored in `settings.json`. API Keys are stored through the operating system credential store (Windows Credential Manager) and are never serialized into `settings.json` or returned to the Vue UI. The settings screen also provides a clear-data action that removes conversations, settings, saved credentials, and startup registration.

The Vue UI is split into `src/components/ConversationSidebar.vue`, `src/components/MessageList.vue`, and `src/components/Composer.vue`; `src/App.vue` coordinates state, persistence, and API calls. Assistant replies request `stream: true` and are rendered incrementally through Tauri events. If a provider rejects streaming, the client retries with a standard JSON completion and displays it through the same path. Assistant Markdown is rendered with sanitized HTML, including tables, lists, links, and fenced code blocks.

In the chat composer, the model selector changes the persisted active model. Each model profile can use either OpenAI Chat Completions or OpenAI Responses API; older settings default to Chat Completions. The reasoning selector maps to `reasoning_effort` for Chat Completions and `reasoning.effort` for Responses. `max_tokens` is sent as `max_output_tokens` for Responses. Support still depends on the selected model and API provider.

The chat composer supports uploading multiple images with text or as an image-only message. Images are stored with conversation history as data URLs and sent as `image_url` for Chat Completions or `input_image` for Responses API. Each image is limited to 10 MB; the selected model and provider must support vision input.

## Runtime configuration

The packaged app reads settings at runtime, so changing the API settings does not require rebuilding. On Windows the app data directory is normally located at:

`%APPDATA%\com.local.aidesk\`

The same location is used in development and in the packaged app. New installations should use the in-app settings instead of editing files manually.
