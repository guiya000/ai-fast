<script setup>
import { ref } from "vue";
import MessageList from "../MessageList.vue";

const props = defineProps({
  messages: { type: Array, required: true },
  isSending: { type: Boolean, default: false },
  isGeneratingImage: { type: Boolean, default: false },
  imageEditingAvailable: { type: Boolean, default: false },
  locale: { type: String, default: "zh-CN" },
  variant: { type: String, default: "default" },
  emptyTitle: { type: String, default: "" },
  emptySubtitle: { type: String, default: "" },
  allowCopy: { type: Boolean, default: true },
  allowEdit: { type: Boolean, default: true },
  allowDelete: { type: Boolean, default: true },
});

const emit = defineEmits(["delete-message", "edit-message", "reference-image"]);
const messageList = ref(null);

defineExpose({
  scrollToBottom: (force = false) => messageList.value?.scrollToBottom(force),
});
</script>

<template>
  <MessageList
    ref="messageList"
    :messages="props.messages"
    :is-sending="props.isSending"
    :is-generating-image="props.isGeneratingImage"
    :image-editing-available="props.imageEditingAvailable"
    :locale="props.locale"
    :variant="props.variant"
    :empty-title="props.emptyTitle"
    :empty-subtitle="props.emptySubtitle"
    :allow-copy="props.allowCopy"
    :allow-edit="props.allowEdit"
    :allow-delete="props.allowDelete"
    @delete-message="emit('delete-message', $event)"
    @edit-message="emit('edit-message', $event)"
    @reference-image="emit('reference-image', $event)"
  />
</template>