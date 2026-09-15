# AI Desk

Vue 3 and Tauri 2 desktop AI chat client.

## Development

1. Install Node.js 20 LTS or newer, Rust stable, and Microsoft C++ Build Tools.
2. Run `npm install`.
3. Edit `src/config.js` with your API endpoint, key, and model name.
4. Run `npm run tauri:dev`.

## Windows build

Run `npm run tauri:build`. The NSIS installer will be under `src-tauri/target/release/bundle/nsis/`.

Vue owns the chat UI and message history. The Rust `chat_completion` command owns outbound HTTP requests, so the WebView does not make cross-origin calls directly.
