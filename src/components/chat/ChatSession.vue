<script setup>
import { computed, ref } from "vue";
import ChatComposer from "./ChatComposer.vue";
import ChatMessageList from "./ChatMessageList.vue";

const props = defineProps({
  modelValue: { type: String, default: "" },
  messages: { type: Array, default: () => [] },
  models: { type: Array, default: () => [] },
  activeModelId: { type: String, default: null },
  functionMode: { type: String, default: "chat" },
  isSending: { type: Boolean, default: false },
  isCanceling: { type: Boolean, default: false },
  isGeneratingImage: { type: Boolean, default: false },
  isEditing: { type: Boolean, default: false },
  reasoningEffort: { type: String, default: "medium" },
  reasoningPresets: { type: Array, default: () => [] },
  imageModel: { type: Boolean, default: false },
  imageParams: { type: Array, default: () => [] },
  initialImages: { type: Array, default: () => [] },
  locale: { type: String, default: "zh-CN" },
  variant: { type: String, default: "default" },
  emptyTitle: { type: String, default: "" },
  emptySubtitle: { type: String, default: "" },
  capabilities: { type: Object, default: () => ({}) },
});

const emit = defineEmits([
  "update:modelValue",
  "send",
  "cancel-generation",
  "select-model",
  "select-function-mode",
  "update-reasoning-effort",
  "update-image-param",
  "cancel-edit",
  "delete-message",
  "edit-message",
  "reference-image",
]);

const messageList = ref(null);
const capabilities = computed(() => ({
  imageInput: true,
  modelSelector: true,
  functionMode: true,
  reasoningEffort: true,
  customParams: true,
  stopGeneration: true,
  messageCopy: true,
  messageEdit: true,
  messageDelete: true,
  imageEditing: false,
  ...props.capabilities,
}));

defineExpose({
  scrollToBottom: (force = false) => messageList.value?.scrollToBottom(force),
});
</script>

<template>
  <section class="chat-session" :class="`chat-session-${props.variant}`">
    <slot name="header" />
    <ChatMessageList
      ref="messageList"
      :messages="props.messages"
      :is-sending="props.isSending"
      :is-generating-image="props.isGeneratingImage"
      :image-editing-available="capabilities.imageEditing"
      :locale="props.locale"
      :variant="props.variant"
      :empty-title="props.emptyTitle"
      :empty-subtitle="props.emptySubtitle"
      :allow-copy="capabilities.messageCopy"
      :allow-edit="capabilities.messageEdit"
      :allow-delete="capabilities.messageDelete"
      @delete-message="emit('delete-message', $event)"
      @edit-message="emit('edit-message', $event)"
      @reference-image="emit('reference-image', $event)"
    />
    <ChatComposer
      :model-value="props.modelValue"
      :is-sending="props.isSending"
      :is-canceling="props.isCanceling"
      :locale="props.locale"
      :models="props.models"
      :active-model-id="props.activeModelId"
      :function-mode="props.functionMode"
      :reasoning-effort="props.reasoningEffort"
      :reasoning-presets="capabilities.reasoningEffort ? props.reasoningPresets : []"
      :image-model="props.imageModel"
      :image-params="capabilities.customParams ? props.imageParams : []"
      :initial-images="props.initialImages"
      :is-editing="props.isEditing"
      :variant="props.variant"
      :enable-image-input="capabilities.imageInput"
      :show-function-mode="capabilities.functionMode"
      :show-model-selector="capabilities.modelSelector"
      :show-reasoning="capabilities.reasoningEffort"
      :show-stop-generation="capabilities.stopGeneration"
      @update:model-value="emit('update:modelValue', $event)"
      @send="emit('send', $event)"
      @cancel-generation="emit('cancel-generation')"
      @select-model="emit('select-model', $event)"
      @select-function-mode="emit('select-function-mode', $event)"
      @update-reasoning-effort="emit('update-reasoning-effort', $event)"
      @update-image-param="(...args) => emit('update-image-param', ...args)"
      @cancel-edit="emit('cancel-edit')"
    />
  </section>
</template>

<style scoped>
.chat-session {
  display: flex;
  height: 100%;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
}

.chat-session-compact :deep(.composer-wrap) {
  padding: 0 10px 10px;
  border-top: 0;
  background: transparent;
}

.chat-session-compact :deep(.composer-tools) {
  width: 100%;
  margin: 8px 0 0;
  gap: 6px;
}

.chat-session-compact :deep(.composer-select-wrap) {
  min-width: 0;
}

.chat-session-compact :deep(.composer-select-wrap > span) {
  font-size: 11px;
}

.chat-session-compact :deep(.composer-select-wrap :deep(.n-select)) {
  min-width: 0;
}
</style>