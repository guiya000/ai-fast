<script setup>
import { computed, nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { modelConfig } from "./config";

const input = ref("");
const messages = ref([]);
const isSending = ref(false);
const status = ref("");
const statusType = ref("idle");
const messageList = ref(null);
const canSend = computed(() => input.value.trim() && !isSending.value);

function scrollToBottom() {
  nextTick(() => {
    const element = messageList.value;
    if (element) element.scrollTop = element.scrollHeight;
  });
}

function onKeydown(event) {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    sendMessage();
  }
}

async function sendMessage() {
  const question = input.value.trim();
  if (!question || isSending.value) return;

  if (!modelConfig.baseUrl || !modelConfig.apiKey || !modelConfig.model) {
    status.value = "请先在 src/config.js 填写接口地址、API Key 和模型名称。";
    statusType.value = "error";
    return;
  }

  const history = messages.value.filter(
    (message) => message.role === "user" || message.role === "assistant",
  );
  messages.value.push({ role: "user", content: question });
  input.value = "";
  isSending.value = true;
  status.value = "正在生成回复";
  statusType.value = "working";
  scrollToBottom();

  try {
    const answer = await invoke("chat_completion", {
      request: {
        baseUrl: modelConfig.baseUrl,
        apiKey: modelConfig.apiKey,
        model: modelConfig.model,
        systemPrompt: modelConfig.systemPrompt,
        temperature: modelConfig.temperature,
        maxTokens: modelConfig.maxTokens,
        messages: [...history, { role: "user", content: question }],
      },
    });
    messages.value.push({ role: "assistant", content: answer });
    status.value = "回复完成";
    statusType.value = "success";
  } catch (error) {
    messages.value.push({ role: "error", content: `请求失败：${String(error)}` });
    status.value = "请求失败";
    statusType.value = "error";
  } finally {
    isSending.value = false;
    scrollToBottom();
  }
}

function clearConversation() {
  messages.value = [];
  status.value = "";
  statusType.value = "idle";
}
</script>

<template>
  <main class="app-shell">
    <header class="topbar" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region>
        <div class="brand-mark" aria-hidden="true">AI</div>
        <div data-tauri-drag-region>
          <h1>AI Desk</h1>
          <p>新建对话</p>
        </div>
      </div>
      <button class="icon-button" type="button" title="清空对话" aria-label="清空对话" :disabled="messages.length === 0 || isSending" @click="clearConversation"><span aria-hidden="true">⌫</span></button>
    </header>

    <section ref="messageList" class="conversation" aria-live="polite">
      <div v-if="messages.length === 0" class="welcome">
        <div class="welcome-mark" aria-hidden="true">AI</div>
        <h2>今天想解决什么问题？</h2>
        <p>开始输入，当前会话会保留上下文。</p>
      </div>
      <article v-for="(message, index) in messages" :key="index" class="message" :class="message.role">
        <div class="message-label">{{ message.role === "user" ? "你" : message.role === "assistant" ? "AI 助手" : "系统" }}</div>
        <div class="message-body">{{ message.content }}</div>
      </article>
    </section>

    <footer class="composer-wrap">
      <form class="composer" @submit.prevent="sendMessage">
        <textarea v-model="input" :disabled="isSending" rows="1" placeholder="输入消息" aria-label="输入消息" @keydown="onKeydown" />
        <button class="send-button" type="submit" :disabled="!canSend" aria-label="发送消息" title="发送消息"><span aria-hidden="true">↑</span></button>
      </form>
      <p class="status" :class="statusType">{{ status || "Enter 发送，Shift + Enter 换行" }}</p>
    </footer>
  </main>
</template>
