<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as monaco from "monaco-editor";
import editorWorker from "../../node_modules/monaco-editor/esm/vs/editor/editor.worker.js?worker";
import jsonWorker from "../../node_modules/monaco-editor/esm/vs/language/json/json.worker.js?worker";
import { NButton, NIcon, NSplit, NTree, useMessage } from "naive-ui";
import ChevronLeftIcon from "../components/icons/ChevronLeftIcon.vue";
import ConfirmDialog from "../components/ConfirmDialog.vue";
import DownloadIcon from "../components/icons/DownloadIcon.vue";
import PanelLeftIcon from "../components/icons/PanelLeftIcon.vue";
import PanelRightIcon from "../components/icons/PanelRightIcon.vue";
import PencilIcon from "../components/icons/PencilIcon.vue";
import RefreshIcon from "../components/icons/RefreshIcon.vue";
import ChatSession from "../components/chat/ChatSession.vue";
import { copyFor } from "../i18n";

if (!globalThis.MonacoEnvironment) {
  globalThis.MonacoEnvironment = {
    getWorker(_, label) {
      return label === "json" ? new jsonWorker() : new editorWorker();
    },
  };
}

const router = useRouter();
const message = useMessage();
const tree = ref([]);
const expandedDirectories = ref(new Set());
const rootPath = ref("");
const tabs = ref([]);
const recentlyClosedTabs = ref([]);
const activePath = ref("");
const editorHost = ref(null);
const diffHost = ref(null);
const prompt = ref("");
const isGenerating = ref(false);
const isSaving = ref(false);
const diffPreview = ref(null);
const statusText = ref("请选择一个工作区");
const appLanguage = ref("zh-CN");
const editorLanguage = ref("zh-CN");
const leftPanelVisible = ref(true);
const rightPanelVisible = ref(true);
const leftPanelWidth = ref(250);
const editorPaneSize = ref(0.68);
const contextMenu = ref({ visible: false, x: 0, y: 0, entry: null });
const clipboardEntry = ref(null);
const pendingDeletion = ref(null);
const chatInput = ref("");
const chatMessages = ref([]);
const chatModels = ref([]);
const chatModelId = ref("");
const chatEditingIndex = ref(null);
const chatEditImages = ref([]);
const chatSession = ref(null);
const workspaceReasoningEffort = ref("medium");
const workspaceParamValues = ref({});
const isChatSending = ref(false);
const workspaceConfig = ref({ root: null, chats: {} });
const activeChatStreamId = ref(null);
const activeChatAssistant = ref(null);
const copy = computed(() => copyFor(appLanguage.value));
const shortcutConfig = ref({
  closeTab: "Ctrl+W",
  reopenTab: "Ctrl+Shift+T",
  switchTab: "Ctrl+Tab",
  nextTab: "Ctrl+PageDown",
  previousTab: "Ctrl+PageUp",
  quickOpen: "Ctrl+P",
  commandPalette: "Ctrl+Shift+P",
  saveFile: "Ctrl+S",
  aiEdit: "Ctrl+Enter",
  formatDocument: "Shift+Alt+F",
  toggleComment: "Ctrl+/",
  goToDefinition: "F12",
  renameSymbol: "F2",
});
let editor = null;
let diffEditor = null;
const models = new Map();
let diffModels = [];
let monacoChineseMessages = null;
let stopChatStreamListener = null;
let suppressWorkspaceCache = false;

const activeTab = computed(() => tabs.value.find((tab) => tab.path === activePath.value) || null);
const activeChatModel = computed(() => chatModels.value.find((model) => (model.configId || model.id) === chatModelId.value) || chatModels.value[0] || null);
const activeChatModelSupportsImage = computed(() => activeChatModel.value?.inputTypes?.includes("image") === true);
const chatModelTreeOptions = computed(() => {
  const groups = new Map();
  for (const model of chatModels.value) {
    const supplierName = model.supplierName || copy.value.unnamedModel;
    const group = groups.get(supplierName) || [];
    group.push(model);
    groups.set(supplierName, group);
  }
  return [...groups.entries()].map(([supplierName, models]) => ({
    key: `supplier:${supplierName}`,
    label: supplierName,
    disabled: true,
    children: models.map((model) => ({
      key: model.configId || model.id,
      label: model.name || copy.value.unnamedModel,
      value: model.configId || model.id,
    })),
  }));
});
const workspaceReasoningOptions = computed(() => [
  { label: copy.value.light, value: "low" },
  { label: copy.value.medium, value: "medium" },
  { label: copy.value.high, value: "high" },
  { label: copy.value.veryHigh, value: "xhigh" },
]);
const workspaceParams = computed(() => (activeChatModel.value?.params || [])
  .filter((param) => param.key && param.valueType === "string" && Array.isArray(param.options) && param.options.length)
  .map((param) => ({
    ...param,
    value: Object.prototype.hasOwnProperty.call(workspaceParamValues.value, param.key)
      ? workspaceParamValues.value[param.key]
      : param.value,
  })));
const treeNodes = computed(() => treeNodesFor(tree.value));

const legacyWorkspaceCachePrefix = "ai-fast.workspace-chat";
let workspaceConfigReady = false;
let workspaceSavePromise = Promise.resolve();

function legacyWorkspaceCacheKey(type) {
  const workspace = rootPath.value || "__default__";
  return `${legacyWorkspaceCachePrefix}.${type}.${encodeURIComponent(workspace)}`;
}

async function loadWorkspaceConfig() {
  try {
    const config = await invoke("load_workspace_config");
    workspaceConfig.value = {
      root: config.root || null,
      chats: config.chats || {},
    };
    workspaceConfigReady = true;
  } catch (error) {
    console.error("读取工作区配置失败", error);
  }
}

async function restoreWorkspaceChatCache() {
  if (!rootPath.value) return;
  try {
    const storedChat = workspaceConfig.value.chats?.[rootPath.value];
    const storedMessages = storedChat?.messages;
    chatMessages.value = Array.isArray(storedMessages) ? storedMessages : [];
    const storedModelId = storedChat?.modelId;
    if (storedModelId && chatModels.value.some((model) => (model.configId || model.id) === storedModelId)) {
      chatModelId.value = storedModelId;
    }
    if (storedChat?.reasoningEffort) workspaceReasoningEffort.value = storedChat.reasoningEffort;
    if (storedChat?.params && typeof storedChat.params === "object") {
      workspaceParamValues.value = { ...workspaceParamValues.value, ...storedChat.params };
    }
    if (!storedChat) {
      const legacyMessages = JSON.parse(localStorage.getItem(legacyWorkspaceCacheKey("messages")) || "[]");
      const legacyModelId = localStorage.getItem(legacyWorkspaceCacheKey("model"));
      if (Array.isArray(legacyMessages)) chatMessages.value = legacyMessages;
      if (legacyModelId && chatModels.value.some((model) => (model.configId || model.id) === legacyModelId)) {
        chatModelId.value = legacyModelId;
      }
      if (Array.isArray(legacyMessages) || legacyModelId) {
        localStorage.removeItem(legacyWorkspaceCacheKey("messages"));
        localStorage.removeItem(legacyWorkspaceCacheKey("model"));
        await persistWorkspaceChatCache();
      }
    }
  } catch (error) {
    console.error("读取工作区聊天缓存失败", error);
    chatMessages.value = [];
  }
}

function persistWorkspaceChatCache() {
  if (suppressWorkspaceCache || !workspaceConfigReady || !rootPath.value) return workspaceSavePromise;
  workspaceConfig.value = {
    ...workspaceConfig.value,
    root: rootPath.value,
    chats: {
      ...workspaceConfig.value.chats,
      [rootPath.value]: {
        messages: chatMessages.value,
        modelId: chatModelId.value || null,
        reasoningEffort: workspaceReasoningEffort.value,
        params: workspaceParamValues.value,
      },
    },
  };
  const config = JSON.parse(JSON.stringify(workspaceConfig.value));
  workspaceSavePromise = workspaceSavePromise
    .catch(() => {})
    .then(() => invoke("save_workspace_config", { config }));
  return workspaceSavePromise;
}

watch(chatMessages, persistWorkspaceChatCache, { deep: true, flush: "sync" });
watch(chatModelId, persistWorkspaceChatCache);
watch(workspaceReasoningEffort, persistWorkspaceChatCache);
watch(workspaceParamValues, persistWorkspaceChatCache, { deep: true });

watch(activeChatModel, (model) => {
  const nextValues = { ...workspaceParamValues.value };
  for (const param of model?.params || []) {
    if (param.key && param.valueType === "string" && Array.isArray(param.options) && param.options.length) {
      if (!param.options.includes(nextValues[param.key])) nextValues[param.key] = param.value || param.options[0];
    }
  }
  workspaceParamValues.value = nextValues;
}, { immediate: true });

function treeNodesFor(entries) {
  return entries.map((entry) => ({
    key: entry.path,
    label: entry.name,
    isLeaf: !entry.isDirectory,
    entry,
    children: entry.isDirectory ? treeNodesFor(entry.children || []) : undefined,
  }));
}

function flattenEntries(entries, result = []) {
  for (const entry of entries) {
    if (entry.isDirectory) flattenEntries(entry.children || [], result);
    else result.push(entry);
  }
  return result;
}

function isImageModel(model) {
  return model?.functionType === "image" || ["images", "imagesApi"].includes(model?.apiFormat);
}

function findEntry(entries, path) {
  for (const entry of entries) {
    if (entry.path === path) return entry;
    if (entry.isDirectory) {
      const found = findEntry(entry.children || [], path);
      if (found) return found;
    }
  }
  return null;
}

function parentPath(path) {
  const parts = String(path || "").split("/");
  parts.pop();
  return parts.join("/");
}

function entryTargetDirectory(entry) {
  return entry?.isDirectory ? entry.path : parentPath(entry?.path);
}

function updateOpenPaths(oldPath, newPath) {
  for (const tab of tabs.value) {
    if (tab.path === oldPath || tab.path.startsWith(`${oldPath}/`)) {
      tab.path = `${newPath}${tab.path.slice(oldPath.length)}`;
      tab.name = tab.path.split("/").pop();
    }
  }
  for (const tab of recentlyClosedTabs.value) {
    if (tab.path === oldPath || tab.path.startsWith(`${oldPath}/`)) {
      tab.path = `${newPath}${tab.path.slice(oldPath.length)}`;
      tab.name = tab.path.split("/").pop();
    }
  }
  const nextModels = new Map();
  for (const [path, model] of models) {
    nextModels.set(path === oldPath || path.startsWith(`${oldPath}/`) ? `${newPath}${path.slice(oldPath.length)}` : path, model);
  }
  models.clear();
  nextModels.forEach((model, path) => models.set(path, model));
  if (activePath.value === oldPath || activePath.value.startsWith(`${oldPath}/`)) {
    activePath.value = `${newPath}${activePath.value.slice(oldPath.length)}`;
  }
}

async function loadMonacoLocale(language) {
  globalThis._VSCODE_NLS_MESSAGES = undefined;
  globalThis._VSCODE_NLS_LANGUAGE = language === "zh-CN" ? "zh-cn" : "en";
  if (language === "zh-CN") {
    await import("../../node_modules/monaco-editor/esm/vs/nls/lang/zh-cn.js");
    monacoChineseMessages ||= globalThis._VSCODE_NLS_MESSAGES;
    globalThis._VSCODE_NLS_MESSAGES = monacoChineseMessages;
  }
}

function parseKeybinding(value) {
  if (!monaco || !String(value || "").trim()) return 0;
  const tokens = String(value).split("+").map((token) => token.trim()).filter(Boolean);
  const key = tokens.pop()?.toUpperCase();
  if (!key) return 0;
  let binding = 0;
  for (const token of tokens) {
    if (["CTRL", "CMD", "COMMAND"].includes(token.toUpperCase())) binding |= monaco.KeyMod.CtrlCmd;
    else if (token.toUpperCase() === "SHIFT") binding |= monaco.KeyMod.Shift;
    else if (token.toUpperCase() === "ALT" || token.toUpperCase() === "OPTION") binding |= monaco.KeyMod.Alt;
    else if (["WIN", "META"].includes(token.toUpperCase())) binding |= monaco.KeyMod.WinCtrl;
  }
  const namedKeys = {
    ENTER: monaco.KeyCode.Enter,
    TAB: monaco.KeyCode.Tab,
    PAGEUP: monaco.KeyCode.PageUp,
    PAGEDOWN: monaco.KeyCode.PageDown,
    SLASH: monaco.KeyCode.Slash,
    "/": monaco.KeyCode.Slash,
    F2: monaco.KeyCode.F2,
    F12: monaco.KeyCode.F12,
  };
  const keyCode = namedKeys[key] || (key.length === 1 && /[A-Z]/.test(key) ? monaco.KeyCode[`Key${key}`] : 0);
  return keyCode ? binding | keyCode : 0;
}

function registerShortcut(name, handler) {
  const keybinding = parseKeybinding(shortcutConfig.value[name]);
  if (keybinding && editor) editor.addCommand(keybinding, handler);
}

function languageForPath(path) {
  const extension = path.split(".").pop()?.toLowerCase();
  return {
    js: "javascript", jsx: "javascript", ts: "typescript", tsx: "typescript",
    vue: "html", html: "html", css: "css", scss: "scss", json: "json",
    md: "markdown", py: "python", rs: "rust", go: "go", java: "java",
    c: "c", cpp: "cpp", h: "cpp", sql: "sql", xml: "xml", yaml: "yaml", yml: "yaml",
  }[extension] || "plaintext";
}

function flattenPromptText(value) {
  return String(value || "").replace(/\r\n/g, "\n");
}

async function loadTree() {
  tree.value = await invoke("list_workspace_entries");
  const directories = flattenTree(tree.value).filter((entry) => entry.isDirectory).map((entry) => entry.path);
  if (expandedDirectories.value.size === 0) expandedDirectories.value = new Set(directories.slice(0, 1));
}

function flattenTree(entries, result = []) {
  for (const entry of entries) {
    result.push(entry);
    if (entry.isDirectory) flattenTree(entry.children || [], result);
  }
  return result;
}

async function openWorkspace() {
  const selected = await open({ directory: true, multiple: false, title: "打开代码工作区" });
  if (!selected || Array.isArray(selected)) return;
  try {
    rootPath.value = await invoke("set_workspace_root", { path: selected });
    suppressWorkspaceCache = true;
    workspaceConfig.value = { ...workspaceConfig.value, root: rootPath.value };
    tabs.value = [];
    recentlyClosedTabs.value = [];
    activePath.value = "";
    chatInput.value = "";
    chatEditingIndex.value = null;
    chatEditImages.value = [];
    chatMessages.value = [];
    workspaceReasoningEffort.value = "medium";
    workspaceParamValues.value = {};
    models.forEach((model) => model.dispose());
    models.clear();
    editor?.setModel(null);
    await loadTree();
    await restoreWorkspaceChatCache();
    suppressWorkspaceCache = false;
    await persistWorkspaceChatCache();
    statusText.value = "工作区已打开";
  } catch (error) {
    suppressWorkspaceCache = false;
    message.error(String(error), { duration: 2500 });
  }
}

function toggleDirectory(path) {
  const next = new Set(expandedDirectories.value);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  expandedDirectories.value = next;
}

function updateExpandedDirectories(keys) {
  expandedDirectories.value = new Set(keys);
}

function updateSelectedFile(keys) {
  const path = keys[0];
  const entry = path ? findEntry(tree.value, path) : null;
  if (entry) openFile(entry);
}

function treeNodeProps(node) {
  return {
    onContextmenu: (event) => showContextMenu(node.entry, event),
  };
}

function updateLeftPanelSize(size) {
  const nextSize = Number.parseFloat(size);
  if (Number.isFinite(nextSize)) leftPanelWidth.value = Math.min(420, Math.max(180, nextSize));
}

function updateEditorPaneSize(size) {
  const nextSize = Number(size);
  if (Number.isFinite(nextSize)) editorPaneSize.value = Math.min(0.85, Math.max(0.35, nextSize));
}

function closeContextMenu() {
  contextMenu.value = { ...contextMenu.value, visible: false };
}

function showContextMenu(entry, event) {
  event.preventDefault();
  contextMenu.value = {
    visible: true,
    x: Math.min(event.clientX, window.innerWidth - 230),
    y: Math.min(event.clientY, window.innerHeight - 330),
    entry,
  };
}

function onWorkspaceContextMenu() {
  closeContextMenu();
}

function contextNewPath(name) {
  const value = String(name || "").trim().replaceAll("\\", "/").replace(/^\/+|\/+$/g, "");
  if (!value) return "";
  const directory = entryTargetDirectory(contextMenu.value.entry);
  return directory ? `${directory}/${value}` : value;
}

async function createEntry(kind) {
  if (!rootPath.value) return message.warning("请先打开工作区");
  const name = window.prompt(kind === "directory" ? copy.value.createFolderPrompt : copy.value.createPathPrompt);
  const path = contextNewPath(name);
  if (!path) return;
  try {
    await invoke(kind === "directory" ? "create_workspace_directory" : "create_workspace_file", { path });
    const parent = parentPath(path);
    if (parent) expandedDirectories.value = new Set([...expandedDirectories.value, parent]);
    closeContextMenu();
    await loadTree();
    if (kind === "file") {
      const entry = findEntry(tree.value, path);
      if (entry) await openFile(entry);
    }
  } catch (error) {
    message.error(String(error), { duration: 2500 });
  }
}

async function renameEntry(entry = contextMenu.value.entry) {
  if (!entry) return;
  const name = window.prompt(copy.value.renamePrompt, entry.name);
  if (!name?.trim() || name.trim() === entry.name) return closeContextMenu();
  try {
    const nextPath = `${parentPath(entry.path) ? `${parentPath(entry.path)}/` : ""}${name.trim()}`;
    await invoke("rename_workspace_entry", { path: entry.path, name: name.trim() });
    updateOpenPaths(entry.path, nextPath);
    closeContextMenu();
    await loadTree();
    statusText.value = `已重命名 ${nextPath}`;
  } catch (error) {
    message.error(String(error), { duration: 2500 });
  }
}

function copyEntry(cut = false) {
  if (!contextMenu.value.entry) return;
  clipboardEntry.value = { path: contextMenu.value.entry.path, cut };
  closeContextMenu();
  message.success(cut ? copy.value.cutFile : copy.value.copiedFile, { duration: 1800 });
}

async function pasteEntry(entry = contextMenu.value.entry) {
  if (!clipboardEntry.value) return;
  const destination = entryTargetDirectory(entry);
  try {
    await invoke("paste_workspace_entry", {
      source: clipboardEntry.value.path,
      destination,
      cut: clipboardEntry.value.cut,
    });
    if (clipboardEntry.value.cut) clipboardEntry.value = null;
    closeContextMenu();
    await loadTree();
  } catch (error) {
    message.error(String(error), { duration: 2500 });
  }
}

async function revealEntry(entry = contextMenu.value.entry) {
  try {
    await invoke("open_workspace_in_explorer", { path: entry?.path || null });
    closeContextMenu();
  } catch (error) {
    message.error(String(error), { duration: 2500 });
  }
}

function closeModelsUnder(path) {
  recentlyClosedTabs.value = recentlyClosedTabs.value.filter((tab) => tab.path !== path && !tab.path.startsWith(`${path}/`));
  for (const tab of [...tabs.value]) {
    if (tab.path === path || tab.path.startsWith(`${path}/`)) {
      const model = models.get(tab.path);
      model?.dispose();
      models.delete(tab.path);
    }
  }
  tabs.value = tabs.value.filter((tab) => tab.path !== path && !tab.path.startsWith(`${path}/`));
  if (activePath.value === path || activePath.value.startsWith(`${path}/`)) {
    const next = tabs.value[0];
    activePath.value = next?.path || "";
    editor?.setModel(next ? models.get(next.path) : null);
  }
}

function requestDeleteEntry(entry = contextMenu.value.entry) {
  if (!entry) return;
  pendingDeletion.value = entry;
  closeContextMenu();
}

async function confirmDeleteEntry() {
  const entry = pendingDeletion.value;
  pendingDeletion.value = null;
  if (!entry) return;
  try {
    await invoke("delete_workspace_file", { path: entry.path });
    closeModelsUnder(entry.path);
    closeContextMenu();
    await loadTree();
  } catch (error) {
    message.error(String(error), { duration: 2500 });
  }
}

function cancelDeleteEntry() {
  pendingDeletion.value = null;
}

function modelRequestParams(params = []) {
  const overrides = arguments[1] || {};
  return params.reduce((result, param) => {
    if (param?.key) result[param.key] = Object.prototype.hasOwnProperty.call(overrides, param.key) ? overrides[param.key] : param.value;
    return result;
  }, {});
}

async function selectChatModel(id) {
  if (!id || !chatModels.value.some((model) => (model.configId || model.id) === id)) return;
  chatModelId.value = id;
  persistWorkspaceChatCache();
}

function clearChat() {
  if (!chatMessages.value.length || window.confirm("确定清空并新建会话吗？")) {
    chatMessages.value = [];
    chatInput.value = "";
    chatEditingIndex.value = null;
    chatEditImages.value = [];
  }
}

function updateWorkspaceParam(key, value) {
  workspaceParamValues.value = { ...workspaceParamValues.value, [key]: value };
}

function editChatMessage(index) {
  if (isChatSending.value) return;
  const item = chatMessages.value[index];
  if (!item || item.role !== "user") return;
  chatEditingIndex.value = index;
  chatInput.value = item.content || "";
  chatEditImages.value = [...(item.images || [])];
}

function cancelChatEdit() {
  chatEditingIndex.value = null;
  chatInput.value = "";
  chatEditImages.value = [];
}

async function deleteChatMessage(index) {
  if (isChatSending.value || !chatMessages.value[index]) return;
  if (!window.confirm(copy.value.deleteMessageConfirm)) return;
  chatMessages.value.splice(index, 1);
  if (chatMessages.value.length === 0) chatInput.value = "";
  if (chatEditingIndex.value === index) cancelChatEdit();
  else if (chatEditingIndex.value !== null && index < chatEditingIndex.value) chatEditingIndex.value -= 1;
}

async function sendChatMessage(messagePayload = {}) {
  const content = String(messagePayload.content || "").trim();
  const images = (messagePayload.images || []).map(({ name, dataUrl }) => ({ name, dataUrl }));
  if ((!content && !images.length) || isChatSending.value || !activeChatModel.value) return;
  const replaceIndex = chatEditingIndex.value;
  const history = chatMessages.value.slice(0, replaceIndex ?? chatMessages.value.length)
    .filter((item) => item.role === "user" || item.role === "assistant");
  if (replaceIndex !== null) chatMessages.value.splice(replaceIndex);
  chatInput.value = "";
  chatEditingIndex.value = null;
  chatEditImages.value = [];
  chatMessages.value.push({ role: "user", content, images });
  const assistant = { role: "assistant", content: "" };
  chatMessages.value.push(assistant);
  const streamId = `workspace-chat-${Date.now()}`;
  activeChatStreamId.value = streamId;
  activeChatAssistant.value = assistant;
  isChatSending.value = true;
  chatSession.value?.scrollToBottom(true);
  try {
    const answer = await invoke("chat_completion", {
      request: {
        ...modelRequestParams(activeChatModel.value.params, workspaceParamValues.value),
        modelId: activeChatModel.value.configId || activeChatModel.value.id,
        messages: [...history, { role: "user", content, images }],
        reasoningEffort: workspaceReasoningEffort.value,
        streamId,
      },
    });
    if (!assistant.content) assistant.content = answer;
  } catch (error) {
    if (String(error) === "CHAT_CANCELED") {
      if (!assistant.content) chatMessages.value.splice(chatMessages.value.indexOf(assistant), 1);
    } else {
      chatMessages.value.splice(chatMessages.value.indexOf(assistant), 1);
      chatMessages.value.push({ role: "error", content: `请求失败：${String(error)}` });
    }
  } finally {
    activeChatStreamId.value = null;
    activeChatAssistant.value = null;
    isChatSending.value = false;
  }
}

async function cancelChatMessage() {
  if (!isChatSending.value || !activeChatStreamId.value) return;
  try {
    await invoke("cancel_chat", { streamId: activeChatStreamId.value });
  } catch (error) {
    message.error(`${copy.value.stopGenerationFailed}：${String(error)}`, { duration: 2000 });
  }
}

async function openFile(entry) {
  if (entry.isDirectory) {
    toggleDirectory(entry.path);
    return;
  }
  if (diffPreview.value) rejectDiff();
  let model = models.get(entry.path);
  if (!model) {
    try {
      const content = await invoke("read_workspace_file", { path: entry.path });
      model = monaco.editor.createModel(content, languageForPath(entry.path), monaco.Uri.parse(`inmemory://workspace/${encodeURIComponent(entry.path)}`));
      models.set(entry.path, model);
      tabs.value.push({ path: entry.path, name: entry.name, dirty: false });
    } catch (error) {
      message.error(String(error), { duration: 2500 });
      return;
    }
  }
  activePath.value = entry.path;
  editor?.setModel(model);
  await nextTick();
  editor?.layout();
  statusText.value = entry.path;
}

function closeTab(path) {
  const tab = tabs.value.find((item) => item.path === path);
  if (tab?.dirty && !window.confirm("文件尚未保存，确定关闭吗？")) return;
  if (tab) recentlyClosedTabs.value.push({ ...tab });
  const model = models.get(path);
  model?.dispose();
  models.delete(path);
  const index = tabs.value.findIndex((item) => item.path === path);
  tabs.value.splice(index, 1);
  if (activePath.value === path) {
    const next = tabs.value[index] || tabs.value[index - 1];
    activePath.value = next?.path || "";
    editor?.setModel(next ? models.get(next.path) : null);
  }
}

async function reopenClosedTab() {
  while (recentlyClosedTabs.value.length) {
    const closedTab = recentlyClosedTabs.value.pop();
    const entry = findEntry(tree.value, closedTab.path);
    if (entry) {
      await openFile(entry);
      return;
    }
  }
}

function switchTab(direction = 1) {
  if (tabs.value.length < 2) return;
  const currentIndex = tabs.value.findIndex((tab) => tab.path === activePath.value);
  const nextIndex = (currentIndex + direction + tabs.value.length) % tabs.value.length;
  const next = tabs.value[nextIndex];
  activePath.value = next.path;
  editor?.setModel(models.get(next.path));
}

function quickOpen() {
  const entries = flattenEntries(tree.value);
  if (!entries.length) return message.info(copy.value.workspaceEmptyDescription, { duration: 2000 });
  const available = entries.map((entry) => entry.path).join("\n");
  const target = window.prompt(`${copy.value.quickOpenPrompt}\n\n${available}`);
  if (!target?.trim()) return;
  const normalized = target.trim().replaceAll("\\", "/").toLocaleLowerCase();
  const entry = entries.find((item) => item.path.toLocaleLowerCase() === normalized)
    || entries.find((item) => item.name.toLocaleLowerCase() === normalized)
    || entries.find((item) => item.path.toLocaleLowerCase().includes(normalized));
  if (entry) openFile(entry);
  else message.warning(`${copy.value.quickOpenPrompt}: ${target}`, { duration: 2000 });
}

function registerEditorShortcuts() {
  registerShortcut("closeTab", () => {
    if (activePath.value) closeTab(activePath.value);
  });
  registerShortcut("reopenTab", reopenClosedTab);
  registerShortcut("switchTab", () => switchTab(1));
  registerShortcut("nextTab", () => switchTab(1));
  registerShortcut("previousTab", () => switchTab(-1));
  registerShortcut("quickOpen", quickOpen);
  registerShortcut("commandPalette", () => editor?.trigger("keyboard", "editor.action.quickCommand", null));
  registerShortcut("saveFile", saveFile);
  registerShortcut("aiEdit", requestAssist);
  registerShortcut("formatDocument", () => editor?.trigger("keyboard", "editor.action.formatDocument", null));
  registerShortcut("toggleComment", () => editor?.trigger("keyboard", "editor.action.commentLine", null));
  registerShortcut("goToDefinition", () => editor?.trigger("keyboard", "editor.action.revealDefinition", null));
  registerShortcut("renameSymbol", () => editor?.trigger("keyboard", "editor.action.rename", null));
}

async function saveFile() {
  if (!activeTab.value || !editor || isSaving.value) return;
  isSaving.value = true;
  try {
    await invoke("write_workspace_file", { path: activeTab.value.path, content: editor.getValue() });
    activeTab.value.dirty = false;
    statusText.value = `已保存 ${activeTab.value.path}`;
  } catch (error) {
    message.error(String(error), { duration: 2500 });
  } finally {
    isSaving.value = false;
  }
}

function selectedRange() {
  const model = editor?.getModel();
  const selection = editor?.getSelection();
  if (!model || !selection) return null;
  const hasSelection = !selection.isEmpty();
  const start = hasSelection ? model.getOffsetAt(selection.getStartPosition()) : 0;
  const end = hasSelection ? model.getOffsetAt(selection.getEndPosition()) : model.getValueLength();
  return { start, end, text: model.getValue().slice(start, end), hasSelection };
}

function parseEditResponse(text) {
  const fenced = String(text || "").match(/```(?:json)?\s*([\s\S]*?)```/i);
  const candidate = fenced?.[1] || String(text || "").slice(String(text || "").indexOf("{"), String(text || "").lastIndexOf("}") + 1);
  if (!candidate) throw new Error("AI 没有返回结构化修改结果");
  const result = JSON.parse(candidate);
  const replacement = result.replacement ?? result.edits?.[0]?.replacement;
  if (typeof replacement !== "string") throw new Error("AI 返回结果缺少 replacement 字段");
  return { summary: String(result.summary || "已生成修改"), replacement };
}

async function requestAssist() {
  if (!activeTab.value || !editor || isGenerating.value || !prompt.value.trim()) return;
  const range = selectedRange();
  if (!range) return;
  const source = range.text;
  if (source.length > 30000) {
    message.warning("当前代码片段超过 30000 个字符，请缩小选择范围");
    return;
  }
  isGenerating.value = true;
  try {
    const requestContent = [
      "你是一个严谨的代码编辑助手。",
      "根据用户要求修改代码，只返回一个 JSON 对象，不要 Markdown，不要解释。",
      'JSON 格式必须是 {"summary":"简短说明","replacement":"替换后的完整代码片段"}。',
      range.hasSelection ? "replacement 只替换用户选中的代码。" : "replacement 必须是整个文件的新内容。",
      `文件路径：${activeTab.value.path}`,
      `用户要求：${flattenPromptText(prompt.value)}`,
      "原始代码：",
      source,
    ].join("\n\n");
    const answer = await invoke("chat_completion", {
      request: {
        messages: [{ role: "user", content: requestContent, images: [] }],
        reasoningEffort: "medium",
        streamId: `code-${Date.now()}`,
      },
    });
    const result = parseEditResponse(answer);
    const original = editor.getValue();
    const modified = original.slice(0, range.start) + result.replacement + original.slice(range.end);
    diffPreview.value = { path: activeTab.value.path, original, modified, summary: result.summary };
    await nextTick();
    showDiffPreview();
    statusText.value = result.summary;
  } catch (error) {
    message.error(`AI 编辑失败：${String(error)}`, { duration: 3000 });
  } finally {
    isGenerating.value = false;
  }
}

function showDiffPreview() {
  if (!diffHost.value || !diffPreview.value) return;
  diffEditor?.dispose();
  diffModels.forEach((model) => model.dispose());
  const language = languageForPath(diffPreview.value.path);
  const original = monaco.editor.createModel(diffPreview.value.original, language);
  const modified = monaco.editor.createModel(diffPreview.value.modified, language);
  diffModels = [original, modified];
  diffEditor = monaco.editor.createDiffEditor(diffHost.value, {
    automaticLayout: true,
    readOnly: true,
    minimap: { enabled: false },
    renderSideBySide: false,
    theme: "vs-dark",
  });
  diffEditor.setModel({ original, modified });
}

function rejectDiff() {
  diffEditor?.dispose();
  diffEditor = null;
  diffModels.forEach((model) => model.dispose());
  diffModels = [];
  diffPreview.value = null;
}

async function acceptDiff() {
  if (!diffPreview.value) return;
  try {
    await invoke("write_workspace_file", { path: diffPreview.value.path, content: diffPreview.value.modified });
    const model = models.get(diffPreview.value.path);
    model?.setValue(diffPreview.value.modified);
    const tab = tabs.value.find((item) => item.path === diffPreview.value.path);
    if (tab) tab.dirty = false;
    rejectDiff();
    statusText.value = `已应用 AI 修改 ${activePath.value}`;
  } catch (error) {
    message.error(String(error), { duration: 2500 });
  }
}

function onEditorChange() {
  const tab = activeTab.value;
  if (tab) tab.dirty = true;
}

onMounted(async () => {
  stopChatStreamListener = await listen("chat-stream", ({ payload }) => {
    if (!payload || payload.streamId !== activeChatStreamId.value) return;
    const assistant = activeChatAssistant.value;
    if (assistant && payload.delta) {
      assistant.content += payload.delta;
      chatSession.value?.scrollToBottom();
    }
  });
  await loadWorkspaceConfig();
  try {
    const settings = await invoke("load_settings");
    chatModels.value = (settings.models || []).filter((model) => !isImageModel(model));
    chatModelId.value = chatModels.value[0]?.configId || chatModels.value[0]?.id || "";
    appLanguage.value = settings.language || "zh-CN";
    editorLanguage.value = settings.editorLanguage || "zh-CN";
    shortcutConfig.value = { ...shortcutConfig.value, ...(settings.shortcuts || {}) };
  } catch (error) {
    console.error("读取编辑器设置失败", error);
  }
  await loadMonacoLocale(editorLanguage.value);
  editor = monaco.editor.create(editorHost.value, {
    automaticLayout: true,
    theme: "vs-dark",
    minimap: { enabled: false },
    fontSize: 13,
    tabSize: 2,
    wordWrap: "off",
  });
  await nextTick();
  editor.layout();
  editor.onDidChangeModelContent(onEditorChange);
  registerEditorShortcuts();
  const existing = await invoke("get_workspace_root");
  if (existing) {
    try {
      rootPath.value = existing;
      await loadTree();
      statusText.value = existing;
    } catch (error) {
      statusText.value = String(error);
    }
  }
  await restoreWorkspaceChatCache();
  await persistWorkspaceChatCache();
  window.addEventListener("click", closeContextMenu);
});

onUnmounted(() => {
  stopChatStreamListener?.();
  window.removeEventListener("click", closeContextMenu);
  editor?.dispose();
  diffEditor?.dispose();
  models.forEach((model) => model.dispose());
  diffModels.forEach((model) => model.dispose());
});
</script>

<template>
  <main class="workspace-page" @contextmenu.prevent="onWorkspaceContextMenu">
    <header class="workspace-topbar">
      <n-button quaternary circle title="返回聊天" aria-label="返回聊天" @click="router.push('/')">
        <template #icon><n-icon><ChevronLeftIcon /></n-icon></template>
      </n-button>
      <div class="workspace-title">
        <strong>{{ copy.codeWorkspace }}</strong>
        <span>{{ rootPath || copy.workspaceEmptyDescription }}</span>
      </div>
      <n-button class="workspace-open-button" secondary @click="openWorkspace">{{ copy.workspaceOpenFolder }}</n-button>
      <n-button quaternary circle :title="leftPanelVisible ? copy.hideFilePanel : copy.showFilePanel" @click="leftPanelVisible = !leftPanelVisible">
        <template #icon><n-icon><PanelLeftIcon /></n-icon></template>
      </n-button>
      <n-button quaternary circle :title="copy.toggleChatPanel" @click="rightPanelVisible = !rightPanelVisible">
        <template #icon><n-icon><PanelRightIcon /></n-icon></template>
      </n-button>
      <n-button quaternary circle :title="copy.saveFile" :aria-label="copy.saveFile" :disabled="!activeTab" :loading="isSaving" @click="saveFile">
        <template #icon><n-icon><DownloadIcon /></n-icon></template>
      </n-button>
    </header>

    <section class="workspace-layout">
      <NSplit
        class="workspace-split"
        direction="horizontal"
        :size="leftPanelVisible ? `${leftPanelWidth}px` : '0px'"
        min="180px"
        max="420px"
        :disabled="!leftPanelVisible"
        :resize-trigger-size="5"
        @update:size="updateLeftPanelSize"
      >
        <template #1>
          <aside v-if="leftPanelVisible" class="file-panel">
        <div class="panel-heading">
          <span>{{ copy.workspaceExplorer }}</span>
          <button type="button" class="panel-action" title="在资源管理器中显示工作区" @click="revealEntry()">...</button>
        </div>
        <div v-if="!rootPath" class="empty-panel">打开一个文件夹开始编辑</div>
        <div v-else class="file-tree" @contextmenu.stop.prevent="showContextMenu(null, $event)">
          <n-tree
            block-line
            selectable
            :indent="18"
            :data="treeNodes"
            :expanded-keys="[...expandedDirectories]"
            :selected-keys="activePath ? [activePath] : []"
            :node-props="treeNodeProps"
            @update:expanded-keys="updateExpandedDirectories"
            @update:selected-keys="updateSelectedFile"
          />
        </div>
          </aside>
        </template>
        <template #2>
          <NSplit
            class="workspace-inner-split"
            direction="horizontal"
            :size="rightPanelVisible ? editorPaneSize : '100%'"
            :min="0.35"
            :max="0.85"
            :disabled="!rightPanelVisible"
            :resize-trigger-size="5"
            @update:size="updateEditorPaneSize"
          >
            <template #1>
              <section class="editor-panel">
        <div class="tabs-bar">
          <button v-for="tab in tabs" :key="tab.path" type="button" class="editor-tab" :class="{ active: tab.path === activePath }" @click="openFile({ path: tab.path, name: tab.name, isDirectory: false })">
            <span>{{ tab.name }}{{ tab.dirty ? " *" : "" }}</span>
            <span class="tab-close" @click.stop="closeTab(tab.path)">x</span>
          </button>
          <span v-if="!tabs.length" class="tabs-empty">{{ copy.workspaceChooseFile }}</span>
        </div>
        <div v-show="!diffPreview" class="editor-stage">
          <div ref="editorHost" class="monaco-host"></div>
          <div v-if="!activeTab" class="editor-empty-state">
            <div class="editor-empty-logo" aria-hidden="true"><span></span><span></span><span></span></div>
            <h2>{{ copy.workspaceEmptyTitle }}</h2>
            <p>{{ copy.workspaceEmptyDescription }}</p>
            <n-button v-if="!rootPath" secondary @click="openWorkspace">{{ copy.workspaceOpenFolder }}
            </n-button>
            <span v-else class="editor-empty-hint">{{ copy.workspaceChooseFile }}</span>
          </div>
        </div>
        <div v-if="diffPreview" class="diff-wrapper">
          <div class="diff-heading">
            <span>{{ diffPreview.summary }}</span>
            <div class="diff-actions">
              <n-button size="small" @click="rejectDiff">拒绝</n-button>
              <n-button size="small" type="primary" @click="acceptDiff">接受并保存</n-button>
            </div>
          </div>
          <div ref="diffHost" class="monaco-host diff-host"></div>
        </div>
              </section>
            </template>
            <template #2>
              <aside v-if="rightPanelVisible" class="chat-panel">
        <div class="chat-panel-heading">
          <div><strong>{{ copy.workspaceChat }}</strong><span>{{ copy.workspaceChatHint }}</span></div>
          <button type="button" class="panel-action" :title="copy.clearChat" @click="clearChat"><n-icon><RefreshIcon /></n-icon></button>
        </div>
        <ChatSession
          ref="chatSession"
          v-model="chatInput"
          :messages="chatMessages"
          :models="chatModels"
          :active-model-id="chatModelId"
          :is-sending="isChatSending"
          :is-editing="chatEditingIndex !== null"
          :reasoning-effort="workspaceReasoningEffort"
          :reasoning-presets="[
            { value: 'low', key: 'light' },
            { value: 'medium', key: 'medium' },
            { value: 'high', key: 'high' },
            { value: 'xhigh', key: 'veryHigh' },
          ]"
          :image-params="workspaceParams"
          :initial-images="chatEditImages"
          :locale="appLanguage"
          :capabilities="{ imageInput: activeChatModelSupportsImage, functionMode: false, imageEditing: false }"
          @select-model="selectChatModel"
          @update-reasoning-effort="workspaceReasoningEffort = $event"
          @update-image-param="updateWorkspaceParam"
          @cancel-generation="cancelChatMessage"
          @cancel-edit="cancelChatEdit"
          @edit-message="editChatMessage"
          @delete-message="deleteChatMessage"
          @send="sendChatMessage"
        />
              </aside>
            </template>
          </NSplit>
        </template>
      </NSplit>
    </section>
    <footer class="workspace-statusbar"><span>{{ statusText }}</span><span v-if="activeTab">{{ languageForPath(activeTab.path) }}</span></footer>
    <div v-if="contextMenu.visible" class="workspace-context-menu" :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }" @click.stop>
      <button type="button" @click="createEntry('file')">{{ copy.createFile }}</button>
      <button type="button" @click="createEntry('directory')">{{ copy.createFolder }}</button>
      <div class="context-divider"></div>
      <button type="button" :disabled="!contextMenu.entry" @click="revealEntry()">{{ copy.revealInExplorer }}</button>
      <button type="button" :disabled="!contextMenu.entry" @click="copyEntry(true)">{{ copy.cut }}</button>
      <button type="button" :disabled="!contextMenu.entry" @click="copyEntry(false)">{{ copy.copy }}</button>
      <button type="button" :disabled="!clipboardEntry" @click="pasteEntry()">{{ copy.paste }}</button>
      <button type="button" :disabled="!contextMenu.entry" @click="renameEntry()">{{ copy.rename }}</button>
      <button type="button" class="danger-action" :disabled="!contextMenu.entry" @click="requestDeleteEntry()">{{ copy.deleteEntry }}</button>
    </div>
    <ConfirmDialog
      :visible="Boolean(pendingDeletion)"
      :title="copy.confirmTitle"
      :message="pendingDeletion ? copy.deleteEntryConfirm.replace('{name}', pendingDeletion.name) : ''"
      :confirm-label="copy.confirm"
      :cancel-label="copy.cancel"
      @confirm="confirmDeleteEntry"
      @cancel="cancelDeleteEntry"
    />
  </main>
</template>

<style scoped>
.workspace-page {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  background: #f4f6f8;
  color: #263244;
}
.workspace-topbar { display: flex; align-items: center; gap: 8px; min-height: 54px; padding: 8px 14px; border-bottom: 1px solid #dfe4ec; background: #fff; }
.workspace-topbar :deep(.n-button) { color: #4c5a6e; }
.workspace-topbar :deep(.n-button:hover), .workspace-topbar :deep(.n-button:focus-visible) { color: #1759c2; background: #eef5ff; }
.workspace-topbar :deep(.n-button.n-button--disabled) { color: #aeb8c5; opacity: 1; }
.workspace-topbar :deep(.workspace-open-button) { border-color: #cdd6e2; background: #fff; color: #263244; }
.workspace-topbar :deep(.workspace-open-button:hover), .workspace-topbar :deep(.workspace-open-button:focus-visible) { border-color: #1f6feb; background: #f4f8ff; color: #1759c2; }
.workspace-title { display: flex; flex: 1; flex-direction: column; min-width: 0; margin: 0 12px 0 4px; }
.workspace-title strong { color: #263244; font-size: 14px; }
.workspace-title span { overflow: hidden; color: #8792a3; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.workspace-layout { display: flex; flex: 1; min-width: 0; min-height: 0; overflow: hidden; }
.workspace-split, .workspace-inner-split { width: 100%; height: 100%; min-width: 0; min-height: 0; }
.workspace-split :deep(.n-split-pane-1), .workspace-split :deep(.n-split-pane-2), .workspace-inner-split :deep(.n-split-pane-1), .workspace-inner-split :deep(.n-split-pane-2) { height: 100%; min-width: 0; min-height: 0; overflow: hidden; }
.file-panel { height: 100%; overflow: auto; border-right: 1px solid #dfe4ec; background: #f8fafc; }
.panel-heading { display: flex; align-items: center; justify-content: space-between; padding: 14px 12px 10px; color: #4c5a6e; font-size: 11px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; }
.panel-action { display: inline-grid; width: 24px; height: 24px; place-items: center; border: 0; background: transparent; color: #8792a3; }
.panel-action:hover { background: #e8f1ff; color: #1759c2; }
.empty-panel { padding: 16px 12px; color: #8792a3; font-size: 12px; line-height: 1.6; }
.file-tree { padding: 4px 0 20px; }
.file-tree :deep(.n-tree-node-content) { min-height: 28px; color: #4c5a6e; font-size: 12px; }
.file-tree :deep(.n-tree-node-content:hover), .file-tree :deep(.n-tree-node-content--selected) { background: #e8f1ff; color: #1759c2; }
.file-tree :deep(.n-tree-node-content__text) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.editor-panel { display: flex; height: 100%; flex: 1 1 0; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; background: #1e1e1e; }
.tabs-bar { display: flex; min-height: 36px; overflow-x: auto; border-bottom: 1px solid #dfe4ec; background: #f8fafc; }
.editor-tab { display: flex; align-items: center; gap: 14px; max-width: 220px; padding: 0 12px; border: 0; border-right: 1px solid #dfe4ec; background: #eef1f5; color: #68768a; font-size: 12px; }
.editor-tab.active { background: #1e1e1e; color: #fff; }
.editor-tab span:first-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tab-close { color: #8792a3; font-size: 12px; }
.tabs-empty { padding: 10px 14px; color: #8792a3; font-size: 12px; }
.editor-stage { position: relative; display: flex; flex: 1; min-width: 0; min-height: 0; overflow: hidden; }
.monaco-host { flex: 1; min-width: 0; min-height: 0; overflow: hidden; }
.editor-empty-state { position: absolute; inset: 0; display: flex; align-items: center; flex-direction: column; justify-content: center; padding: 32px; background: #1e1e1e; color: #d4d4d4; text-align: center; }
.editor-empty-logo { display: grid; grid-template-columns: repeat(3, 10px); align-items: end; gap: 5px; height: 44px; margin-bottom: 18px; }
.editor-empty-logo span { display: block; width: 10px; height: 28px; border-radius: 3px; background: #1f6feb; }
.editor-empty-logo span:nth-child(2) { height: 42px; background: #258343; }
.editor-empty-logo span:nth-child(3) { height: 22px; background: #f0a119; }
.editor-empty-state h2 { margin: 0; color: #f2f2f2; font-size: 18px; font-weight: 600; }
.editor-empty-state p { max-width: 360px; margin: 10px 0 18px; color: #aeb8c5; font-size: 13px; line-height: 1.6; }
.editor-empty-hint { color: #8792a3; font-size: 12px; }
.diff-wrapper { display: flex; flex: 1; min-width: 0; min-height: 0; flex-direction: column; overflow: hidden; }
.diff-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 12px; border-bottom: 1px solid #dfe4ec; background: #fff; color: #4c5a6e; font-size: 12px; }
.diff-actions { display: flex; gap: 8px; }
.diff-host { min-width: 0; min-height: 0; }
.chat-panel { display: flex; height: 100%; flex: 1; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; background: #f8fafc; }
.chat-panel-heading { display: flex; align-items: center; justify-content: space-between; min-height: 58px; padding: 10px 12px; border-bottom: 1px solid #dfe4ec; background: #fff; }
.chat-panel-heading div { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.chat-panel-heading strong { color: #263244; font-size: 13px; }
.chat-panel-heading span { color: #8792a3; font-size: 11px; }
.workspace-context-menu { position: fixed; z-index: 20; width: 218px; padding: 5px; border: 1px solid #cdd6e2; border-radius: 5px; box-shadow: 0 12px 28px rgb(25 42 70 / 16%); background: #fff; }
.workspace-context-menu button { display: block; width: 100%; padding: 7px 9px; border: 0; background: transparent; color: #3d4b5f; font-size: 12px; text-align: left; }
.workspace-context-menu button:hover:not(:disabled) { background: #eef5ff; color: #1759c2; }
.workspace-context-menu button:disabled { color: #b2bcc9; cursor: not-allowed; }
.workspace-context-menu .danger-action { color: #b42318; }
.context-divider { height: 1px; margin: 5px 0; background: #eef1f5; }
.workspace-statusbar { display: flex; justify-content: space-between; min-height: 24px; padding: 4px 12px; background: #176b87; color: #fff; font-size: 11px; }
@media (max-width: 720px) {
  .workspace-topbar .n-button:not(:first-child) { padding-inline: 8px; }
  .workspace-topbar .n-button:nth-of-type(2) { display: none; }
}
</style>
