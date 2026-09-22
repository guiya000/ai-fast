<script setup>
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { NButton, NIcon, NSelect, NTreeSelect, useMessage } from "naive-ui";
import { copyFor } from "../i18n";
import CloseIcon from "./icons/CloseIcon.vue";
import ImageIcon from "./icons/ImageIcon.vue";
import InfoIcon from "./icons/InfoIcon.vue";
import SendIcon from "./icons/SendIcon.vue";
import StopIcon from "./icons/StopIcon.vue";

const props = defineProps({
  modelValue: { type: String, default: "" },
  isSending: { type: Boolean, default: false },
  locale: { type: String, default: "zh-CN" },
  models: { type: Array, default: () => [] },
  activeModelId: { type: String, default: null },
  functionMode: { type: String, default: "chat" },
  isCanceling: { type: Boolean, default: false },
  reasoningEffort: { type: String, default: "medium" },
  reasoningPresets: { type: Array, default: () => [] },
  imageModel: { type: Boolean, default: false },
  imageParams: { type: Array, default: () => [] },
  initialImages: { type: Array, default: () => [] },
  isEditing: { type: Boolean, default: false },
  variant: { type: String, default: "default" },
  enableImageInput: { type: Boolean, default: true },
  showFunctionMode: { type: Boolean, default: true },
  showModelSelector: { type: Boolean, default: true },
  showReasoning: { type: Boolean, default: true },
  showStopGeneration: { type: Boolean, default: true },
});

const message = useMessage();

const emit = defineEmits(["update:modelValue", "send", "cancel-generation", "select-model", "select-function-mode", "update-reasoning-effort", "update-image-param", "cancel-edit"]);
const draft = ref(props.modelValue);
const imageInput = ref(null);
const textarea = ref(null);
const attachments = ref([]);
const pendingImages = ref(0);
const isDragActive = ref(false);
const dragLeaveTimer = ref(null);
const copy = computed(() => copyFor(props.locale));
const canSubmit = computed(() => !pendingImages.value && Boolean(draft.value.trim() || (props.enableImageInput && attachments.value.length)));
function isImageModel(model) {
  return model?.functionType === "image" || model?.apiFormat === "images" || model?.apiFormat === "imagesApi";
}
const supplierGroups = computed(() => {
  const groups = new Map();
  for (const model of props.models.filter((item) => (props.functionMode === "image" ? isImageModel(item) : !isImageModel(item)))) {
    const supplierName = model.supplierName || copy.value.unnamedModel;
    const group = groups.get(supplierName) || [];
    group.push(model);
    groups.set(supplierName, group);
  }
  return [...groups.entries()].map(([name, models]) => ({ name, models }));
});
const modelOptions = computed(() => supplierGroups.value.map((supplier) => ({
  key: `supplier:${supplier.name}`,
  label: supplier.name,
  disabled: true,
  children: supplier.models.map((model) => ({
    key: model.configId || model.id,
    label: model.name || copy.value.unnamedModel,
    value: model.configId || model.id,
  })),
})));
const functionModes = computed(() => [
  { value: "chat", label: copy.value.chatFunction },
  { value: "image", label: copy.value.imageFunction },
].filter((mode) => props.models.some((model) => mode.value === "image" ? isImageModel(model) : !isImageModel(model))));
const reasoningOptions = computed(() => props.reasoningPresets.map((preset) => ({
  label: copy.value[preset.key],
  value: preset.value,
})));

const maxImageSize = 10 * 1024 * 1024;
const maxImages = 3;
const maxTextFileSize = 2 * 1024 * 1024;

watch(() => props.initialImages, (images) => {
  attachments.value = [...(images || [])].slice(0, maxImages).map((image, index) => ({
    ...image,
    id: image.id || `${image.name || "image"}-${index}-${image.dataUrl?.slice(-12) || ""}`,
  }));
}, { immediate: true });

watch(() => props.enableImageInput, (enabled) => {
  if (!enabled) attachments.value = [];
});

watch(() => props.modelValue, (value) => {
  if (value !== draft.value) {
    draft.value = value;
    nextTick(resizeTextarea);
  }
});

function resizeTextarea() {
  const element = textarea.value;
  if (!element) return;
  element.style.height = "auto";
  const maxHeight = 150;
  element.style.height = `${Math.min(element.scrollHeight, maxHeight)}px`;
  element.style.overflowY = element.scrollHeight > maxHeight ? "auto" : "hidden";
}

function updateDraft(event) {
  draft.value = event.target.value;
  emit("update:modelValue", draft.value);
  resizeTextarea();
}

function onKeydown(event) {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    submit();
  }
}

function submit() {
  const images = props.enableImageInput ? attachments.value : [];
  if ((!draft.value.trim() && images.length === 0) || props.isSending) return;
  emit("send", { content: draft.value, images });
  attachments.value = [];
}

function openImagePicker() {
  imageInput.value?.click();
}

function addImageFiles(files) {
  const availableSlots = maxImages - attachments.value.length - pendingImages.value;
  const acceptedFiles = files
    .filter((file) => file.type.startsWith("image/") && file.size <= maxImageSize)
    .slice(0, Math.max(0, availableSlots));
  pendingImages.value += acceptedFiles.length;
  for (const file of acceptedFiles) {
    const reader = new FileReader();
    reader.onload = () => {
      pendingImages.value -= 1;
      attachments.value.push({
        id: `${file.name}-${file.size}-${file.lastModified}-${Math.random()}`,
        name: file.name,
        dataUrl: reader.result,
      });
    };
    reader.onerror = () => {
      pendingImages.value -= 1;
    };
    reader.readAsDataURL(file);
  }
}

function addImages(event) {
  addImageFiles(Array.from(event.target.files || []));
  event.target.value = "";
}

function handleDragOver(event) {
  if (props.isSending || !props.enableImageInput) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = "copy";
  isDragActive.value = true;
}

function handleDragLeave(event) {
  if (event.currentTarget.contains(event.relatedTarget)) return;
  clearTimeout(dragLeaveTimer.value);
  dragLeaveTimer.value = setTimeout(() => {
    isDragActive.value = false;
  }, 40);
}

function handleDrop(event) {
  if (props.isSending || !props.enableImageInput) return;
  event.preventDefault();
  isDragActive.value = false;
  const files = Array.from(event.dataTransfer?.files || []);
  addImageFiles(files);
  const textFiles = files.filter((file) => !file.type.startsWith("image/"));
  for (const file of textFiles) addTextFile(file);
}

function addTextFile(file) {
  const extension = file.name.split(".").pop()?.toLowerCase();
  const knownTextType = file.type.startsWith("text/")
    || ["csv", "css", "html", "js", "json", "jsx", "md", "py", "rs", "ts", "tsx", "vue", "xml", "yaml", "yml"].includes(extension);
  if (!knownTextType || file.size > maxTextFileSize) {
    message.error(copy.value.unsupportedFile, { duration: 2000 });
    return;
  }
  const reader = new FileReader();
  reader.onload = () => {
    const content = String(reader.result || "");
    const language = extension || "text";
    const separator = draft.value.trim() ? "\n\n" : "";
    draft.value = `${draft.value}${separator}\`\`\`${language}\n${content}\n\`\`\``;
    emit("update:modelValue", draft.value);
    nextTick(resizeTextarea);
  };
  reader.onerror = () => message.error(copy.value.fileReadFailed, { duration: 2000 });
  reader.readAsText(file);
}

function handlePaste(event) {
  if (props.isSending || !props.enableImageInput) return;
  const files = Array.from(event.clipboardData?.items || [])
    .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
    .map((item) => item.getAsFile())
    .filter(Boolean);
  if (files.length === 0) return;
  event.preventDefault();
  addImageFiles(files);
}

function removeImage(id) {
  attachments.value = attachments.value.filter((item) => item.id !== id);
}

onMounted(resizeTextarea);
</script>

<template>
  <footer class="composer-wrap">
    <div v-if="attachments.length" class="composer-attachments" :aria-label="copy.selectedImages">
      <div v-for="image in attachments" :key="image.id" class="composer-attachment">
        <img :src="image.dataUrl" :alt="image.name" />
        <n-button circle size="tiny" type="button" :title="copy.removeImage" :aria-label="`${copy.removeImage}: ${image.name}`" @click="removeImage(image.id)">
          <template #icon><n-icon><CloseIcon /></n-icon></template>
        </n-button>
      </div>
    </div>
    <form class="composer" :class="{ compact: props.variant === 'compact', 'drag-active': isDragActive, 'is-sending': props.isSending }" @submit.prevent="submit" @dragover="handleDragOver" @dragleave="handleDragLeave" @drop="handleDrop">
      <div v-if="props.isEditing" class="composer-editing">
        {{ copy.editingMessage }}
        <n-button text type="primary" size="small" @click="emit('cancel-edit')">{{ copy.cancel }}</n-button>
      </div>
      <div v-if="isDragActive && props.enableImageInput" class="drop-hint">{{ copy.dropFiles }}</div>
      <textarea ref="textarea" :value="draft" :disabled="props.isSending" rows="1" :placeholder="copy.inputPlaceholder" :aria-label="copy.inputPlaceholder" @input="updateDraft" @keydown="onKeydown" @paste="handlePaste" />
      <input v-if="props.enableImageInput" ref="imageInput" class="image-input" type="file" accept="image/*" multiple @change="addImages" />
      <div class="composer-inline-tools" :class="{ 'has-image-input': props.enableImageInput }">
        <div v-if="props.showModelSelector" class="composer-select-wrap composer-model-select">
          <span>{{ copy.currentModel }}</span>
          <n-tree-select :value="props.activeModelId || null" :options="modelOptions" :placeholder="copy.noModelSelected" :disabled="props.isSending || props.models.length === 0" :aria-label="copy.currentModel" :show-path="false" :filterable="true" :default-expand-all="true" :indent="14" size="small" @update:value="$emit('select-model', $event)" />
        </div>
        <div v-if="props.showFunctionMode" class="function-mode-switch" role="group" :aria-label="copy.functionMode">
          <button
            v-for="mode in functionModes"
            :key="mode.value"
            type="button"
            :class="{ active: props.functionMode === mode.value }"
            :disabled="props.isSending"
            @click="emit('select-function-mode', mode.value)"
          >
            {{ mode.label }}
          </button>
        </div>
        <n-button v-if="props.enableImageInput" class="image-button" quaternary circle type="button" :disabled="props.isSending || attachments.length + pendingImages >= maxImages" :title="copy.imageLimit" :aria-label="copy.imageLimit" @click="openImagePicker">
          <template #icon><n-icon><ImageIcon /></n-icon></template>
        </n-button>
        <n-button v-if="!props.isSending" class="send-button" type="primary" circle attr-type="submit" :disabled="!canSubmit" :aria-label="copy.sendMessage" :title="copy.sendMessage">
          <template #icon><n-icon><SendIcon /></n-icon></template>
        </n-button>
        <n-button v-else-if="props.showStopGeneration" class="stop-button" type="error" circle :disabled="props.isCanceling" :aria-label="copy.stopGenerating" :title="copy.stopGenerating" @click="$emit('cancel-generation')">
          <template #icon><n-icon><StopIcon /></n-icon></template>
        </n-button>
      </div>
    </form>
    <div class="composer-tools">
      <div v-if="props.showReasoning && !props.imageModel" class="composer-select-wrap output-size-select">
          <span>{{ copy.reasoningEffort }} <small :title="copy.reasoningEffortHint"><InfoIcon /></small></span>
          <n-select :value="props.reasoningEffort" :options="reasoningOptions" :disabled="props.isSending || props.models.length === 0" :aria-label="copy.reasoningEffort" size="small" @update:value="$emit('update-reasoning-effort', $event)" />
      </div>
      <div v-for="param in props.imageParams" :key="param.key" class="composer-select-wrap image-param-select">
        <span>{{ param.description || param.key }}</span>
        <n-select
          :value="param.value"
          :options="param.options.map((option) => ({ label: option, value: option }))"
          :disabled="props.isSending"
          :aria-label="param.description || param.key"
          size="small"
          @update:value="$emit('update-image-param', param.key, $event)"
        />
      </div>
    </div>
    <p class="status">{{ copy.warning }}</p>
  </footer>
</template>

<style scoped>
.composer-wrap {
  position: relative;
  flex: 0 0 auto;
  padding: 14px 24px 22px;
  border-top: 1px solid var(--border);
  background: var(--surface);
}

.drop-hint {
  position: absolute;
  z-index: 3;
  inset: 10px 24px;
  display: grid;
  place-items: center;
  border: 2px dashed var(--accent);
  border-radius: 9px;
  background: rgb(244 248 255 / 94%);
  color: var(--accent-dark);
  font-size: 14px;
  font-weight: 600;
  pointer-events: none;
}

.composer.drag-active {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgb(31 111 235 / 10%);
}

.composer {
  position: relative;
  isolation: isolate;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 9px;
  width: min(1200px, 100%);
  padding: 9px 10px 9px 14px;
  border: 1px solid var(--border-input);
  border-radius: 9px;
  background: var(--surface);
  margin: 0 auto;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.composer.compact {
  width: 100%;
  padding: 8px 10px;
  border-radius: 7px;
}

.composer.compact .composer-inline-tools {
  gap: 5px;
}

.composer.compact .composer-select-wrap :deep(.n-select) {
  min-width: 0;
}

.composer > * {
  position: relative;
  z-index: 2;
}

.composer.is-sending {
  border-color: transparent;
  box-shadow: 0 0 18px rgb(31 111 235 / 18%);
}

.composer.is-sending::before,
.composer.is-sending::after {
  position: absolute;
  content: "";
  pointer-events: none;
}

.composer.is-sending::before {
  z-index: 0;
  inset: -1px;
  border-radius: 10px;
  background: linear-gradient(
    110deg,
    transparent 10%,
    rgb(31 111 235 / 35%) 28%,
    var(--accent) 44%,
    #8b5cf6 52%,
    rgb(31 111 235 / 35%) 66%,
    transparent 86%
  );
  background-size: 240% 100%;
  filter: blur(1px);
  animation: composer-border-flow 2.4s linear infinite;
}

.composer.is-sending::after {
  z-index: 1;
  inset: 1px;
  border-radius: 8px;
  background: var(--surface);
}

@keyframes composer-border-flow {
  from { background-position: 120% 0; }
  to { background-position: -120% 0; }
}

@media (prefers-reduced-motion: reduce) {
  .composer.is-sending::before {
    animation: none;
    background: var(--accent);
  }
}

.composer-inline-tools {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
}

.composer-model-select {
  flex: 0 1 270px;
}

.composer-model-select :deep(.n-tree-select) {
  min-width: 150px;
  flex: 1;
}

.composer-inline-tools .image-button {
  margin-left: auto;
}

.composer-inline-tools:not(.has-image-input) .send-button,
.composer-inline-tools:not(.has-image-input) .stop-button {
  margin-left: auto;
}

.composer:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgb(31 111 235 / 10%);
}

.composer-attachments {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 10px;
}

.composer-attachment {
  position: relative;
  width: 72px;
  height: 72px;
  overflow: hidden;
  border: 1px solid var(--border-input);
  border-radius: 7px;
  background: var(--surface-muted);
}

.composer-attachment img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.composer-attachment .n-button {
  position: absolute;
  top: 3px;
  right: 3px;
  z-index: 1;
}

.composer textarea {
  flex: 1;
  min-width: 0;
  min-height: 34px;
  max-height: 150px;
  padding: 4px 0;
  resize: none;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text);
  font-size: 15px;
  line-height: 1.5;
}

.composer textarea::placeholder {
  color: var(--text-muted);
}

.composer-tools {
  display: flex;
  align-items: center;
  gap: 7px;
  width: min(1200px, 100%);
  margin: 10px auto 0;
}

.function-mode-switch {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
  background: var(--surface-muted);
}

.function-mode-switch button {
  min-width: 46px;
  height: 28px;
  padding: 0 10px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
}

.function-mode-switch button.active {
  background: var(--surface);
  color: var(--accent-dark);
  box-shadow: 0 1px 3px rgb(0 0 0 / 10%);
}

.function-mode-switch button:disabled {
  cursor: not-allowed;
}

.image-button,
.send-button,
.stop-button {
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  width: 34px;
  height: 34px;
  padding: 0;
  border-radius: 7px;
}

.image-button {
  color: var(--text-secondary);
}

.image-button:hover:not(:disabled) {
  background: var(--accent-soft);
  color: var(--accent-dark);
}

.send-button,
.stop-button {
  margin-left: 0;
}

.image-input {
  display: none;
}

.composer-editing {
  position: absolute;
  right: 54px;
  bottom: calc(100% + 8px);
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-secondary);
  font-size: 12px;
}

.composer-select-wrap {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.composer-select-wrap > span,
.composer-toggle > span {
  flex: 0 0 auto;
  color: var(--text-secondary);
  font-size: 12px;
}

.composer-select-wrap :deep(.n-select) {
  min-width: 150px;
  --n-height: 32px;
}

.composer-toggle {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-left: auto;
}

.output-size-select :deep(.n-select) {
  min-width: 145px;
}

.composer-tools small {
  display: inline-flex;
  width: 14px;
  height: 14px;
  vertical-align: -2px;
}

.composer-tools small .ui-icon {
  width: 14px;
  height: 14px;
}

.status {
  width: min(1200px, 100%);
  margin: 10px auto 0;
  color: var(--text-muted);
  font-size: 12px;
}

@media (max-width: 720px) {
  .composer-wrap {
    padding: 10px 12px 15px;
  }

  .drop-hint {
    inset: 8px 12px;
  }

  .composer-tools {
    align-items: stretch;
    flex-wrap: wrap;
  }

  .composer-select-wrap {
    flex: 1 1 180px;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
  }

  .composer-toggle {
    margin-left: 0;
  }

  .status {
    width: 100%;
  }
}
</style>
