<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { NButton, NIcon, useMessage } from "naive-ui";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Composer from "../components/Composer.vue";
import ConfirmDialog from "../components/ConfirmDialog.vue";
import ConversationSidebar from "../components/ConversationSidebar.vue";
import MessageList from "../components/MessageList.vue";
import TrashIcon from "../components/icons/TrashIcon.vue";
import PanelLeftIcon from "../components/icons/PanelLeftIcon.vue";
import { copyFor } from "../i18n";

const input = ref("");
const inputImages = ref([]);
const conversationSearch = ref("");
const editingMessageIndex = ref(null);
const conversations = ref([]);
const activeConversationId = ref(null);
const isSending = ref(false);
const isGeneratingImage = ref(false);
const isEditingImage = ref(false);
const isCanceling = ref(false);
const messageList = ref(null);
const confirmation = ref(null);
let confirmationResolver = null;
const settings = ref({
  theme: "system",
  language: "zh-CN",
  autostart: false,
  activeModelId: null,
  suppliers: [],
  models: [],
});
const router = useRouter();
const message = useMessage();
const sidebarCollapsed = ref(false);
const sidebarWidth = ref(250);
const isResizingSidebar = ref(false);
let sidebarResizeStartX = 0;
let sidebarResizeStartWidth = 250;

function isImagesProtocol(apiFormat) {
  return apiFormat === "images" || apiFormat === "imagesApi";
}

function modelRequestParams(params = []) {
  const overrides = arguments[1] || {};
  return params.reduce((result, param) => {
    const key = String(param?.key || "").trim();
    if (key) result[key] = Object.prototype.hasOwnProperty.call(overrides, key) ? overrides[key] : param.value;
    return result;
  }, {});
}

const activeConversation = computed(() => conversations.value.find(
  (conversation) => conversation.id === activeConversationId.value,
));
const imageGenerationAvailable = computed(() => isImagesProtocol(activeModel.value?.apiFormat));
const activeModelIsImage = computed(() => activeModel.value?.functionType === "image" || imageGenerationAvailable.value);
const functionMode = ref("chat");
function isImageModel(model) {
  return model?.functionType === "image" || isImagesProtocol(model?.apiFormat);
}
const visibleConversations = computed(() => {
  const query = conversationSearch.value.trim().toLocaleLowerCase();
  if (!query) return conversations.value;
  return conversations.value.filter((conversation) => {
    const searchable = [
      conversation.title,
      ...conversation.messages.flatMap((message) => [
        message.content,
        ...(message.images || []).map((image) => image.name),
      ]),
    ].filter(Boolean).join("\n").toLocaleLowerCase();
    return searchable.includes(query);
  });
});
const messages = computed(() => activeConversation.value?.messages || []);
const isActiveConversationSending = computed(() => isSending.value && (
  activeConversationId.value === activeStreamConversationId.value
  || activeConversationId.value === activeImageGenerationConversationId.value
));
const copy = computed(() => copyFor(settings.value.language));
const themeClass = computed(() => `theme-${settings.value.theme || "system"}`);
const activeModel = computed(() => settings.value.models.find(
  (model) => (model.configId || model.id) === settings.value.activeModelId
    && (functionMode.value === "image" ? isImageModel(model) : !isImageModel(model)),
  ) || settings.value.models.find((model) => functionMode.value === "image" ? isImageModel(model) : !isImageModel(model)) || null);
const imageParamValues = ref({});
const imageParams = computed(() => (activeModel.value?.params || [])
  .filter((param) => param.key && param.valueType === "string" && Array.isArray(param.options) && param.options.length)
  .map((param) => ({
    ...param,
    value: Object.prototype.hasOwnProperty.call(imageParamValues.value, param.key)
      ? imageParamValues.value[param.key]
      : param.value,
  })));

watch(activeModel, (model) => {
  const nextValues = {};
  for (const param of model?.params || []) {
    if (param.key && param.valueType === "string" && Array.isArray(param.options) && param.options.length) {
      nextValues[param.key] = param.value;
    }
  }
  imageParamValues.value = nextValues;
}, { immediate: true });

function updateImageParam(key, value) {
  imageParamValues.value = { ...imageParamValues.value, [key]: value };
}

const reasoningEffort = ref("medium");
const activeStreamId = ref(null);
const activeStreamConversationId = ref(null);
const activeImageGenerationConversationId = ref(null);
const activeAssistantMessage = ref(null);
let stopStreamListener = null;

const reasoningPresets = [
  { value: "low", key: "light" },
  { value: "medium", key: "medium" },
  { value: "high", key: "high" },
  { value: "xhigh", key: "veryHigh" },
];

function createId() {
  return globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function now() {
  return new Date().toISOString();
}

async function persistConversations() {
  try {
    await invoke("save_conversations", { conversations: conversations.value });
  } catch (error) {
    console.error("历史记录保存失败", error);
    message.error(`${copy.value.conversationSaveFailed}：${String(error)}`, { duration: 2000 });
  }
}

async function loadConversations() {
  try {
    const stored = await invoke("load_conversations");
    conversations.value = stored.sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
    activeConversationId.value = conversations.value[0]?.id || null;
  } catch (error) {
    console.error("历史记录读取失败", error);
    message.error(`${copy.value.conversationLoadFailed}：${String(error)}`, { duration: 2000 });
  }
}

async function loadSettings() {
  try {
    settings.value = await invoke("load_settings");
    const selectedModel = settings.value.models.find(
      (model) => (model.configId || model.id) === settings.value.activeModelId,
    );
    functionMode.value = isImageModel(selectedModel) ? "image" : "chat";
    if (!settings.value.activeModelId && settings.value.models.length > 0) {
      await selectModel(settings.value.models[0].configId || settings.value.models[0].id);
    }
  } catch (error) {
    message.error(`${copy.value.settingsLoadFailed}：${String(error)}`, { duration: 2000 });
  }
}

async function updateSettings(nextSettings) {
  const settingsToSave = {
    theme: nextSettings.theme,
    language: nextSettings.language,
    autostart: nextSettings.autostart,
    activeModelId: nextSettings.activeModelId,
    suppliers: (nextSettings.suppliers || []).map((supplier) => ({
      name: supplier.name,
      config: {
        officialAddress: supplier.config?.officialAddress || "",
        baseURL: supplier.config?.baseURL || supplier.config?.baseUrl || "",
      },
      models: (supplier.models || []).map((model) => ({
        id: model.id,
        name: model.name,
        functionType: model.functionType,
        apiFormat: model.apiFormat,
        requestPath: model.requestPath || "",
        apiKey: model.apiKey,
        temperature: model.temperature,
        maxTokens: model.maxTokens,
        params: model.params || [],
      })),
    })),
  };
  settings.value = await invoke("save_settings", { settings: settingsToSave });
}

async function selectModel(id) {
  if (isSending.value || !id) return;
  try {
    const selectedModel = settings.value.models.find((model) => (model.configId || model.id) === id);
    if (selectedModel) functionMode.value = isImageModel(selectedModel) ? "image" : "chat";
    await updateSettings({ ...settings.value, activeModelId: id });
  } catch (error) {
    message.error(String(error), { duration: 2000 });
  }
}

async function selectFunctionMode(mode) {
  if (isSending.value || functionMode.value === mode) return;
  const nextModel = settings.value.models.find((model) => mode === "image" ? isImageModel(model) : !isImageModel(model));
  if (!nextModel) return;
  functionMode.value = mode;
  await selectModel(nextModel.configId || nextModel.id);
}

function updateReasoningEffort(value) {
  if (!isSending.value) reasoningEffort.value = value;
}

function openSettings() {
  router.push("/settings");
}

function askForConfirmation(message) {
  return new Promise((resolve) => {
    confirmationResolver = resolve;
    confirmation.value = { message };
  });
}

function resolveConfirmation(result) {
  const resolve = confirmationResolver;
  confirmationResolver = null;
  confirmation.value = null;
  resolve?.(result);
}

function resetComposer() {
  input.value = "";
  inputImages.value = [];
  editingMessageIndex.value = null;
}

function startImageReference(payload) {
  if (!activeModelIsImage.value || !payload?.image?.dataUrl) return;
  input.value = "";
  inputImages.value = [{
    name: payload.image.name || "image.png",
    dataUrl: payload.image.dataUrl,
  }];
  editingMessageIndex.value = null;
}

async function editImage(prompt, images) {
  const sourceImages = (images || []).filter((image) => image?.dataUrl);
  if (!prompt || sourceImages.length === 0 || isEditingImage.value) return;
  isEditingImage.value = true;
  isSending.value = true;
  isGeneratingImage.value = true;
  const conversation = activeConversation.value || createConversation(prompt);
  const userMessage = { role: "user", content: prompt, images: sourceImages };
  const assistantMessage = { role: "assistant", content: "", images: [] };
  if (conversation.title === "新建对话") conversation.title = titleFromQuestion(prompt);
  conversation.messages.push(userMessage, assistantMessage);
  touchConversation(conversation);
  input.value = "";
  inputImages.value = [];
  isGeneratingImage.value = true;
  activeImageGenerationConversationId.value = conversation.id;
  await persistConversations();
  messageList.value?.scrollToBottom(true);
  try {
    const result = await invoke("edit_image", {
      request: {
        ...modelRequestParams(activeModel.value?.params, imageParamValues.value),
        imageDataUrls: sourceImages.map((image) => image.dataUrl),
        prompt,
      },
    });
    assistantMessage.images = (result.images || []).map((image, index) => ({
      name: `edited-image-${index + 1}.png`,
      dataUrl: image.dataUrl,
    }));
    touchConversation(conversation);
    await persistConversations();
  } catch (error) {
    const assistantIndex = conversation.messages.indexOf(assistantMessage);
    if (assistantIndex !== -1) conversation.messages.splice(assistantIndex, 1);
    conversation.messages.push({ role: "error", content: `图片编辑失败：${String(error)}` });
    await persistConversations();
    message.error(`${copy.value.imageEditFailed}：${String(error)}`, { duration: 2000 });
  } finally {
    isEditingImage.value = false;
    isGeneratingImage.value = false;
    activeImageGenerationConversationId.value = null;
    isSending.value = false;
    messageList.value?.scrollToBottom();
  }
}

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value;
}

function startSidebarResize(event) {
  if (sidebarCollapsed.value) return;
  isResizingSidebar.value = true;
  sidebarResizeStartX = event.clientX;
  sidebarResizeStartWidth = sidebarWidth.value;
  window.addEventListener("mousemove", resizeSidebar);
  window.addEventListener("mouseup", stopSidebarResize, { once: true });
}

function resizeSidebar(event) {
  if (!isResizingSidebar.value) return;
  sidebarWidth.value = Math.min(380, Math.max(190, sidebarResizeStartWidth + event.clientX - sidebarResizeStartX));
}

function stopSidebarResize() {
  if (!isResizingSidebar.value) return;
  isResizingSidebar.value = false;
  window.removeEventListener("mousemove", resizeSidebar);
}

function startNewConversation() {
  activeConversationId.value = null;
  resetComposer();
}

function selectConversation(id) {
  activeConversationId.value = id;
  resetComposer();
}

function titleFromQuestion(question) {
  const compact = question.replace(/\s+/g, " ").trim();
  return compact.length > 28 ? `${compact.slice(0, 28)}...` : compact;
}

function createConversation(question) {
  const timestamp = now();
  const conversation = {
    id: createId(),
    title: titleFromQuestion(question),
    messages: [],
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  conversations.value.unshift(conversation);
  activeConversationId.value = conversation.id;
  return conversation;
}

function touchConversation(conversation) {
  conversation.updatedAt = now();
  conversations.value = [...conversations.value].sort(
    (left, right) => right.updatedAt.localeCompare(left.updatedAt),
  );
}

async function sendMessage(payload, options = {}) {
  const messagePayload = typeof payload === "string"
    ? { content: payload, images: [] }
    : payload;
  const retryIndex = Number.isInteger(options.retryIndex) ? options.retryIndex : null;
  const content = messagePayload.content.trim();
  const images = (messagePayload.images || []).map((image) => ({
    name: image.name,
    dataUrl: image.dataUrl,
  }));
  if ((!content && images.length === 0) || isSending.value) return;

  if (activeModelIsImage.value) {
    if (!content) {
      message.error(copy.value.imagePromptRequired, { duration: 2000 });
      return;
    }
    editingMessageIndex.value = null;
    if (images.length) {
      await editImage(content, images);
    } else {
      await generateImage(content);
    }
    return;
  }
  const conversation = activeConversation.value || createConversation(content || copy.value.uploadedImage);
  if (conversation.title === "新建对话") {
    conversation.title = titleFromQuestion(content);
  }
  const replaceIndex = retryIndex ?? editingMessageIndex.value;
  const history = conversation.messages.slice(0, replaceIndex ?? conversation.messages.length).filter(
    (message) => message.role === "user" || message.role === "assistant",
  );
  const requestMessage = {
    role: "user",
    content,
    ...(images.length ? { images } : {}),
  };
  if (replaceIndex !== null) conversation.messages.splice(replaceIndex);
  conversation.messages.push({ role: "user", content, images });
  conversation.messages.push({ role: "assistant", content: "" });
  const assistantMessage = conversation.messages[conversation.messages.length - 1];
  const streamId = createId();
  activeStreamId.value = streamId;
  activeStreamConversationId.value = conversation.id;
  activeAssistantMessage.value = assistantMessage;
  touchConversation(conversation);
  input.value = "";
  inputImages.value = [];
  editingMessageIndex.value = null;
  isSending.value = true;
  await persistConversations();
  messageList.value?.scrollToBottom(true);

  try {
    const answer = await invoke("chat_completion", {
      request: {
        ...modelRequestParams(activeModel.value?.params),
        messages: [...history, requestMessage],
        reasoningEffort: reasoningEffort.value,
        streamId,
      },
    });
    assistantMessage.content = answer;
  } catch (error) {
    if (String(error) === "CHAT_CANCELED") {
      if (!assistantMessage.content) {
        const assistantIndex = conversation.messages.indexOf(assistantMessage);
        if (assistantIndex !== -1) conversation.messages.splice(assistantIndex, 1);
      }
    } else {
      const assistantIndex = conversation.messages.indexOf(assistantMessage);
      if (assistantIndex !== -1) conversation.messages.splice(assistantIndex, 1);
      conversation.messages.push({ role: "error", content: `请求失败：${String(error)}` });
    }
  } finally {
    activeStreamId.value = null;
    activeStreamConversationId.value = null;
    activeAssistantMessage.value = null;
    touchConversation(conversation);
    await persistConversations();
    isSending.value = false;
    isCanceling.value = false;
    messageList.value?.scrollToBottom();
  }
}

async function generateImage(prompt) {
  const trimmedPrompt = prompt.trim();
  if (!trimmedPrompt || isSending.value || !activeModelIsImage.value) return;
  const conversation = activeConversation.value || createConversation(trimmedPrompt);
  const userMessage = { role: "user", content: trimmedPrompt, images: [] };
  const assistantMessage = { role: "assistant", content: "", images: [] };
  conversation.messages.push(userMessage, assistantMessage);
  touchConversation(conversation);
  input.value = "";
  isSending.value = true;
  isGeneratingImage.value = true;
  activeImageGenerationConversationId.value = conversation.id;
  await persistConversations();
  try {
    const result = await invoke("generate_image", {
      request: { ...modelRequestParams(activeModel.value?.params, imageParamValues.value), prompt: trimmedPrompt },
    });
    assistantMessage.images = (result.images || []).map((image, index) => ({
      name: `generated-image-${index + 1}.png`,
      dataUrl: image.dataUrl,
    }));
  } catch (error) {
    conversation.messages.splice(conversation.messages.indexOf(assistantMessage), 1);
    conversation.messages.push({ role: "error", content: `图片生成失败：${String(error)}` });
    message.error(`${copy.value.imageGenerationFailed}：${String(error)}`, { duration: 3000 });
  } finally {
    touchConversation(conversation);
    await persistConversations();
    isGeneratingImage.value = false;
    activeImageGenerationConversationId.value = null;
    isSending.value = false;
    messageList.value?.scrollToBottom();
  }
}

function editMessage(index) {
  if (isSending.value || !activeConversation.value) return;
  const message = activeConversation.value.messages[index];
  if (!message || message.role !== "user") return;
  input.value = message.content || "";
  inputImages.value = [...(message.images || [])];
  editingMessageIndex.value = index;
}

function cancelEditMessage() {
  editingMessageIndex.value = null;
  resetComposer();
}

async function cancelGeneration() {
  if (!isSending.value || isCanceling.value || !activeStreamId.value) return;
  isCanceling.value = true;
  try {
    await invoke("cancel_chat", { streamId: activeStreamId.value });
  } catch (error) {
    isCanceling.value = false;
    console.error("停止生成失败", error);
    message.error(`${copy.value.stopGenerationFailed}：${String(error)}`, { duration: 2000 });
  }
}

async function deleteMessage(index) {
  if (isActiveConversationSending.value || !activeConversation.value) return;
  if (!(await askForConfirmation(copy.value.deleteMessageConfirm))) return;
  activeConversation.value.messages.splice(index, 1);
  if (activeConversation.value.messages.length === 0) activeConversation.value.title = "新建对话";
  touchConversation(activeConversation.value);
  await persistConversations();
}

async function deleteConversation(id) {
  if (isSending.value && id === activeStreamConversationId.value) return;
  const index = conversations.value.findIndex((conversation) => conversation.id === id);
  if (index === -1) return;
  if (!(await askForConfirmation(copy.value.deleteConversationConfirm))) return;
  conversations.value.splice(index, 1);
  if (activeConversationId.value === id) {
    activeConversationId.value = conversations.value[0]?.id || null;
    resetComposer();
  }
  await persistConversations();
}

async function clearConversation() {
  if (!activeConversation.value || isActiveConversationSending.value) return;
  if (!(await askForConfirmation(copy.value.clearConversationConfirm))) return;
  activeConversation.value.messages = [];
  activeConversation.value.title = "新建对话";
  touchConversation(activeConversation.value);
  resetComposer();
  await persistConversations();
}

onMounted(async () => {
  if (window.innerWidth <= 720) sidebarCollapsed.value = true;
  stopStreamListener = await listen("chat-stream", ({ payload }) => {
    if (!payload || payload.streamId !== activeStreamId.value || !payload.delta) return;
    const assistantMessage = activeAssistantMessage.value;
    if (!assistantMessage || assistantMessage.role !== "assistant") return;
    assistantMessage.content += payload.delta;
    if (activeConversationId.value === activeStreamConversationId.value) {
      messageList.value?.scrollToBottom();
    }
  });
  await Promise.all([loadConversations(), loadSettings()]);
});

onUnmounted(() => {
  stopStreamListener?.();
  stopSidebarResize();
});
</script>

<template>
  <main class="app-shell" :class="[themeClass, { 'sidebar-collapsed': sidebarCollapsed, 'is-resizing-sidebar': isResizingSidebar } ]" :style="{ '--sidebar-width': `${sidebarWidth}px` }">
    <ConversationSidebar
      :conversations="visibleConversations"
      v-model:search="conversationSearch"
      :active-id="activeConversationId"
      :locale="settings.language"
      :collapsed="sidebarCollapsed"
      @new="startNewConversation"
      @select="selectConversation"
      @delete="deleteConversation"
      @settings="openSettings"
      @toggle-collapse="toggleSidebar"
      @resize-start="startSidebarResize"
    />

    <section class="chat-shell">
      <header class="topbar" data-tauri-drag-region>
        <n-button v-if="sidebarCollapsed" class="icon-button sidebar-open-button" quaternary circle :title="copy.expandSidebar" :aria-label="copy.expandSidebar" @click="toggleSidebar">
          <template #icon><n-icon><PanelLeftIcon /></n-icon></template>
        </n-button>
        <div data-tauri-drag-region>
          <h1>{{ activeConversation?.title || copy.newConversation }}</h1>
          <!-- <p>{{ activeConversation ? copy.currentConversation : copy.startConversation }} · {{ activeModel?.model || copy.noModelSelected }}</p> -->
        </div>
        <n-button class="icon-button" quaternary circle :title="copy.clearConversation" :aria-label="copy.clearConversation" :disabled="!activeConversation || messages.length === 0 || isActiveConversationSending" @click="clearConversation">
          <template #icon><n-icon><TrashIcon /></n-icon></template>
        </n-button>
      </header>

      <MessageList ref="messageList" :messages="messages" :is-sending="isActiveConversationSending" :is-generating-image="isGeneratingImage" :image-editing-available="activeModelIsImage" :locale="settings.language" @delete-message="deleteMessage" @edit-message="editMessage" @reference-image="startImageReference" />
      <Composer
        v-model="input"
        :initial-images="inputImages"
        :is-editing="editingMessageIndex !== null"
        :is-sending="isSending"
        :is-canceling="isCanceling"
        :locale="settings.language"
        :models="settings.models"
        :active-model-id="settings.activeModelId"
        :function-mode="functionMode"
        :reasoning-effort="reasoningEffort"
        :reasoning-presets="reasoningPresets"
        :image-model="activeModelIsImage"
        :image-params="imageParams"
        @select-model="selectModel"
        @select-function-mode="selectFunctionMode"
        @cancel-generation="cancelGeneration"
        @update-reasoning-effort="updateReasoningEffort"
        @update-image-param="updateImageParam"
        @cancel-edit="cancelEditMessage"
        @send="sendMessage"
      />
    </section>

    <ConfirmDialog
      :visible="Boolean(confirmation)"
      :title="copy.confirmTitle"
      :message="confirmation?.message || ''"
      :confirm-label="copy.confirm"
      :cancel-label="copy.cancel"
      @confirm="resolveConfirmation(true)"
      @cancel="resolveConfirmation(false)"
    />
  </main>
</template>

<style scoped>
.app-shell {
  display: grid;
  grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
  width: 100%;
  height: 100vh;
  min-height: 100vh;
  overflow: hidden;
  background: var(--app-bg);
  color: var(--text);
}

.app-shell.sidebar-collapsed {
  grid-template-columns: 0 minmax(0, 1fr);
}

.chat-shell {
  display: flex;
  min-width: 0;
  min-height: 100vh;
  flex-direction: column;
  overflow: hidden;
}

.topbar {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 66px;
  padding: 10px 24px;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}

.topbar > div {
  min-width: 0;
  flex: 1;
}

.topbar h1 {
  margin: 0;
  overflow: hidden;
  color: var(--text);
  font-size: 16px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.icon-button {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  padding: 0;
  color: var(--text-secondary);
}

.sidebar-open-button {
  flex: 0 0 auto;
}

@media (max-width: 720px) {
  .app-shell,
  .app-shell.sidebar-collapsed {
    position: relative;
    grid-template-columns: minmax(0, 1fr);
  }

  .app-shell :deep(.sidebar) {
    position: absolute;
    z-index: 10;
    top: 0;
    bottom: 0;
    left: 0;
    width: min(var(--sidebar-width), calc(100vw - 24px));
    box-shadow: 8px 0 24px rgb(15 23 42 / 14%);
  }

  .app-shell.sidebar-collapsed :deep(.sidebar) {
    width: 0;
  }

  .topbar {
    min-height: 58px;
    padding: 8px 14px;
  }

}
</style>
