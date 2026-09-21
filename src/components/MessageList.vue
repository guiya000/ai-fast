<script setup>
import { computed, nextTick, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-shell";
import { useMessage } from "naive-ui";
import { copyFor } from "../i18n";
import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";
import CopyIcon from "./icons/CopyIcon.vue";
import EditIcon from "./icons/EditIcon.vue";
import ImagePreview from "./ImagePreview.vue";
import ReferenceIcon from "./icons/ReferenceIcon.vue";
import TrashIcon from "./icons/TrashIcon.vue";
const emit = defineEmits(["delete-message", "edit-message", "reference-image"]);
const copiedMessageIndex = ref(null);
const shouldAutoScroll = ref(true);
const message = useMessage();
let copiedMessageTimer = null;

const props = defineProps({
  messages: { type: Array, required: true },
  isSending: { type: Boolean, default: false },
  isGeneratingImage: { type: Boolean, default: false },
  imageEditingAvailable: { type: Boolean, default: false },
  locale: { type: String, default: "zh-CN" },
});

const messageList = ref(null);
const copy = computed(() => copyFor(props.locale));
const markdown = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: true,
});

function renderMarkdown(content) {
  return DOMPurify.sanitize(markdown.render(content || ""));
}

async function openLink(event) {
  const link = event.target.closest("a");
  if (!link) return;
  const href = link.getAttribute("href");
  if (!/^https?:\/\//i.test(href || "")) return;
  event.preventDefault();
  try {
    await open(href);
  } catch (error) {
    console.error("打开链接失败", error);
    message.error(`${copy.value.openLinkFailed}：${String(error)}`, { duration: 2000 });
  }
}

function scrollToBottom(force = false) {
  if (force) shouldAutoScroll.value = true;
  if (!force && !shouldAutoScroll.value) return;
  nextTick(() => {
    const element = messageList.value;
    if (element) element.scrollTop = element.scrollHeight;
  });
}

function handleScroll() {
  const element = messageList.value;
  if (!element) return;
  shouldAutoScroll.value = element.scrollHeight - element.scrollTop - element.clientHeight <= 48;
}

async function copyMessage(content, index) {
  try {
    await navigator.clipboard.writeText(content);
    copiedMessageIndex.value = index;
    clearTimeout(copiedMessageTimer);
    copiedMessageTimer = setTimeout(() => {
      copiedMessageIndex.value = null;
    }, 1800);
  } catch (error) {
    console.error("复制消息失败", error);
    message.error(`${copy.value.copyFailed}：${String(error)}`, { duration: 2000 });
  }
}

watch(() => [props.messages.length, props.isSending], scrollToBottom, { flush: "post" });
defineExpose({ scrollToBottom });
</script>

<template>
  <section ref="messageList" class="conversation" aria-live="polite" @scroll="handleScroll">
    <div v-if="props.messages.length === 0" class="welcome">
      <div class="welcome-mark" aria-hidden="true">AI</div>
      <h2>{{ copy.welcomeTitle }}</h2>
      <p>{{ copy.welcomeSubtitle }}</p>
    </div>
    <article v-for="(message, index) in props.messages" v-show="!(message.role === 'assistant' && props.isSending && !message.content && !message.images?.length)" :key="`${message.role}-${index}`" class="message" :class="message.role">
      <div class="message-avatar" aria-hidden="true">{{ message.role === "user" ? "我" : message.role === "assistant" ? "AI" : "!" }}</div>
      <div class="message-content">
        <div class="message-label">{{ message.role === "user" ? copy.you : message.role === "assistant" ? copy.assistant : copy.system }}</div>
        <div v-if="message.role === 'assistant'" class="message-body">
          <div v-if="message.images?.length" class="message-image-list">
            <ImagePreview v-for="image in message.images" :key="image.dataUrl" :image="image" :locale="props.locale" />
          </div>
          <div v-if="message.content" class="markdown-body" v-html="renderMarkdown(message.content)" @click="openLink"></div>
        </div>
        <div v-else class="message-body">
          <div v-if="message.images?.length" class="message-image-list">
            <ImagePreview v-for="image in message.images" :key="image.dataUrl" :image="image" :locale="props.locale" />
          </div>
          <p v-if="message.content" class="message-text">{{ message.content }}</p>
        </div>
        <div v-if="(message.content || message.images?.length) && message.role !== 'error'" class="message-actions">
          <button v-if="message.content" type="button" :title="copiedMessageIndex === index ? copy.copied : copy.copyMessage" :aria-label="copiedMessageIndex === index ? copy.copied : copy.copyMessage" @click="copyMessage(message.content, index)"><CopyIcon /></button>
          <button v-if="message.role === 'user'" type="button" :title="copy.editMessage" :aria-label="copy.editMessage" @click="emit('edit-message', index)"><EditIcon /></button>
          <button v-if="props.imageEditingAvailable && message.role === 'assistant' && message.images?.length" type="button" :title="copy.referenceImage" :aria-label="copy.referenceImage" @click="emit('reference-image', { image: message.images[0] })"><ReferenceIcon /></button>
          <span v-if="copiedMessageIndex === index" class="copy-success" role="status">{{ copy.copied }}</span>
          <button type="button" :disabled="props.isSending" :title="copy.deleteMessage" :aria-label="copy.deleteMessage" @click="emit('delete-message', index)"><TrashIcon /></button>
        </div>
      </div>
    </article>
    <article v-if="props.isSending && (props.messages[props.messages.length - 1]?.role !== 'assistant' || (!props.messages[props.messages.length - 1]?.content && !props.messages[props.messages.length - 1]?.images?.length))" class="message assistant typing-message" :aria-label="props.isGeneratingImage ? copy.imageGenerationStatus : copy.assistantTyping" aria-live="polite">
      <div class="message-avatar" aria-hidden="true">AI</div>
      <div class="message-content">
        <div class="message-label">{{ props.isGeneratingImage ? copy.imageGenerationStatus : copy.assistant }}</div>
        <div class="message-body typing-body">
          <span v-if="props.isGeneratingImage" class="typing-status">{{ copy.imageGenerationStatus }}</span>
          <span class="typing-dot" aria-hidden="true"></span>
          <span class="typing-dot" aria-hidden="true"></span>
          <span class="typing-dot" aria-hidden="true"></span>
        </div>
      </div>
    </article>
  </section>
</template>

<style scoped>
.conversation {
  flex: 1;
  width: min(1200px, 100%);
  min-height: 0;
  margin: 0 auto;
  padding: 34px 24px;
  overflow-y: auto;
}

.welcome {
  display: grid;
  place-items: center;
  align-content: center;
  min-height: 100%;
  color: var(--text-muted);
  text-align: center;
}

.welcome-mark {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  margin-bottom: 18px;
  border-radius: 7px;
  background: var(--accent);
  color: #fff;
  font-size: 15px;
  font-weight: 800;
}

.welcome h2 {
  margin: 0;
  color: var(--text);
  font-size: 24px;
  font-weight: 700;
}

.welcome p {
  margin: 9px 0 0;
  font-size: 14px;
}

.message {
  display: flex;
  align-items: flex-start;
  gap: 18px;
  max-width: 1100px;
  margin: 0 0 30px;
}

.message.user {
  flex-direction: row-reverse;
  margin-left: auto;
}

.message-avatar {
  display: grid;
  flex: 0 0 52px;
  place-items: center;
  width: 52px;
  height: 52px;
  margin-top: 2px;
  border-radius: 50%;
  background: #dceafe;
  color: #1765b0;
  font-size: 21px;
  font-weight: 700;
}

.message.user .message-avatar {
  background: #1677ff;
  color: #fff;
}

.message.error .message-avatar {
  background: #fde4e4;
  color: var(--danger);
}

.message-content {
  min-width: 0;
  max-width: calc(100% - 70px);
}

.message.user .message-content {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.message-label {
  margin: 0 0 8px;
  color: #7a8799;
  font-size: 16px;
  font-weight: 500;
}

.message-body {
  width: fit-content;
  max-width: 100%;
  min-width: 0;
  overflow: hidden;
  padding: 13px 18px;
  overflow-wrap: anywhere;
  border: 1px solid #dce3eb;
  border-radius: 12px;
  background: var(--surface-muted);
  color: var(--text);
  font-size: 20px;
  white-space: pre-wrap;
  line-height: 1.65;
}

.message-image-list {
  display: flex;
  max-width: 100%;
  flex-wrap: wrap;
  gap: 8px;
}

.message-image-list :deep(.image-preview-trigger) {
  display: block;
  width: min(360px, 100%);
  max-width: 100%;
  border-radius: 7px;
  overflow: hidden;
}

.message-image-list :deep(.image-preview-trigger img) {
  display: block;
  width: 100%;
  max-width: 100%;
  max-height: 300px;
  object-fit: cover;
}

.message-text {
  margin: 8px 0 0;
}

.message-image-list + .message-text {
  margin-top: 10px;
}

.message-actions {
  display: flex;
  gap: 8px;
  margin-top: 7px;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.message:hover .message-actions,
.message:focus-within .message-actions {
  opacity: 1;
}

.message-actions button {
  display: grid;
  place-items: center;
  width: 30px;
  height: 28px;
  padding: 0;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: #7a8799;
  font-size: 18px;
}

.message-actions button:hover:not(:disabled) {
  background: #eaf1fb;
  color: var(--accent-dark);
}

.copy-success {
  align-self: center;
  color: var(--success);
  font-size: 12px;
}

.markdown-body {
  width: min(100%, 1000px);
  white-space: normal;
}

:deep(.markdown-body > :first-child) {
  margin-top: 0;
}

:deep(.markdown-body > :last-child) {
  margin-bottom: 0;
}

:deep(.markdown-body p) {
  margin: 0 0 12px;
}

:deep(.markdown-body h1),
:deep(.markdown-body h2),
:deep(.markdown-body h3) {
  margin: 18px 0 10px;
  line-height: 1.35;
}

:deep(.markdown-body h1) {
  font-size: 1.35em;
}

:deep(.markdown-body h2) {
  font-size: 1.2em;
}

:deep(.markdown-body h3) {
  font-size: 1.08em;
}

:deep(.markdown-body ul),
:deep(.markdown-body ol) {
  margin: 8px 0 14px;
  padding-left: 1.5em;
}

:deep(.markdown-body li + li) {
  margin-top: 5px;
}

:deep(.markdown-body blockquote) {
  margin: 12px 0;
  padding-left: 14px;
  border-left: 3px solid #9abbe9;
  color: var(--text-secondary);
}

:deep(.markdown-body a) {
  color: var(--accent-dark);
  text-decoration: underline;
}

:deep(.markdown-body code) {
  padding: 2px 5px;
  border-radius: 4px;
  background: rgb(31 111 235 / 9%);
  font-size: 0.86em;
}

:deep(.markdown-body pre) {
  margin: 12px 0;
  padding: 12px 14px;
  overflow-x: auto;
  border-radius: 7px;
  background: var(--code-surface);
  color: #e5e7eb;
  line-height: 1.5;
}

:deep(.markdown-body pre code) {
  padding: 0;
  background: transparent;
  color: inherit;
  font-size: 0.78em;
  white-space: pre;
}

:deep(.markdown-body table) {
  display: block;
  width: max-content;
  min-width: 100%;
  max-width: 100%;
  margin: 14px 0;
  overflow-x: auto;
  border-collapse: collapse;
  font-size: 0.78em;
  line-height: 1.45;
}

:deep(.markdown-body th),
:deep(.markdown-body td) {
  min-width: 90px;
  padding: 8px 10px;
  border: 1px solid var(--border-input);
  text-align: left;
  vertical-align: top;
}

:deep(.markdown-body th) {
  background: var(--accent-soft);
  font-weight: 700;
}

:deep(.markdown-body tr:nth-child(even) td) {
  background: rgb(234 241 251 / 42%);
}

:deep(.markdown-body hr) {
  margin: 16px 0;
  border: 0;
  border-top: 1px solid var(--border);
}

.user .message-body {
  border-color: #1677ff;
  background: #1677ff;
  color: #fff;
}

.error .message-body {
  border-color: #edc2c2;
  background: #fff6f6;
  color: var(--danger);
}

.typing-body {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 68px;
  min-height: 50px;
  padding: 0 17px;
}

.typing-status {
  margin-right: 4px;
  color: #66758a;
  font-size: 15px;
  white-space: nowrap;
}

.typing-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #7d8a9b;
  animation: typing-bounce 1.1s infinite ease-in-out;
}

.typing-dot:nth-child(2) {
  animation-delay: 0.14s;
}

.typing-dot:nth-child(3) {
  animation-delay: 0.28s;
}

@keyframes typing-bounce {
  0%,
  60%,
  100% {
    transform: translateY(0);
    opacity: 0.45;
  }

  30% {
    transform: translateY(-4px);
    opacity: 1;
  }
}

@media (max-width: 720px) {
  .conversation {
    padding: 22px 16px;
  }

  .welcome h2 {
    font-size: 21px;
  }

  .message {
    gap: 12px;
  }

  .message-avatar {
    flex-basis: 42px;
    width: 42px;
    height: 42px;
    font-size: 17px;
  }

  .message-content {
    max-width: calc(100% - 54px);
  }

  .message-label {
    font-size: 14px;
  }

  .message-body {
    font-size: 17px;
  }
}
</style>