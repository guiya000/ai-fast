<script setup>
import { computed } from "vue";
import { NButton, NIcon, NInput } from "naive-ui";
import { copyFor } from "../i18n";
import PlusIcon from "./icons/PlusIcon.vue";
import SettingsIcon from "./icons/SettingsIcon.vue";
import TrashIcon from "./icons/TrashIcon.vue";
import ChevronLeftIcon from "./icons/ChevronLeftIcon.vue";
import SearchIcon from "./icons/SearchIcon.vue";
import PencilIcon from "./icons/PencilIcon.vue";

const props = defineProps({
  conversations: { type: Array, required: true },
  activeId: { type: String, default: null },
  disabled: { type: Boolean, default: false },
  locale: { type: String, default: "zh-CN" },
  collapsed: { type: Boolean, default: false },
  search: { type: String, default: "" },
});
const copy = computed(() => copyFor(props.locale));
</script>

<template>
  <aside class="sidebar" :class="{ collapsed: props.collapsed }">
    <header class="sidebar-header">
      <div class="brand" data-tauri-drag-region>
        <div class="brand-mark" aria-hidden="true">AI</div>
        <div data-tauri-drag-region>
          <h1>AI-fast</h1>
          <p>{{ copy.appSubtitle }}</p>
        </div>
      </div>
      <n-button class="sidebar-new-button" secondary :disabled="props.disabled" @click="$emit('new')">
        <template #icon><n-icon><PlusIcon /></n-icon></template>
        {{ copy.newConversation }}
      </n-button>
      <n-button class="sidebar-collapse-button" circle quaternary :title="copy.collapseSidebar" :aria-label="copy.collapseSidebar" @click="$emit('toggle-collapse')">
        <template #icon><n-icon><ChevronLeftIcon /></n-icon></template>
      </n-button>
    </header>
    <div class="history-heading">{{ copy.recentConversations }}</div>
    <n-input class="conversation-search" :value="props.search" type="text" clearable :placeholder="copy.searchConversations" :aria-label="copy.searchConversations" @update:value="$emit('update:search', $event)">
      <template #prefix><n-icon><SearchIcon /></n-icon></template>
    </n-input>
    <nav class="conversation-list" :aria-label="copy.recentConversations">
      <p v-if="props.conversations.length === 0" class="empty-history">{{ copy.emptyHistory }}</p>
      <div v-for="conversation in props.conversations" :key="conversation.id" class="conversation-item" :class="{ active: conversation.id === props.activeId }">
        <n-button class="conversation-select" quaternary :disabled="props.disabled" @click="$emit('select', conversation.id)">
          <strong>{{ conversation.title }}</strong>
        </n-button>
        <n-button class="conversation-delete" quaternary circle :disabled="props.disabled" :title="copy.deleteConversation" :aria-label="`${copy.deleteConversation}: ${conversation.title}`" @click.stop="$emit('delete', conversation.id)">
          <template #icon><n-icon><TrashIcon /></n-icon></template>
        </n-button>
      </div>
    </nav>
    <n-button class="sidebar-workspace-button" quaternary :disabled="props.disabled" @click="$emit('workspace')">
      <template #icon><n-icon><PencilIcon /></n-icon></template>
      {{ copy.codeWorkspace }}
    </n-button>
    <n-button class="sidebar-settings-button" quaternary :disabled="props.disabled" @click="$emit('settings')">
      <template #icon><n-icon><SettingsIcon /></n-icon></template>
      {{ copy.settings }}
    </n-button>
  </aside>
</template>

<style scoped>
.sidebar { position: relative; display: flex; height: 100%; flex-direction: column; min-width: 0; border-right: 1px solid var(--border); background: var(--surface); transition: width 0.18s ease, opacity 0.18s ease; }
.sidebar.collapsed { width: 0; overflow: hidden; border-right: 0; opacity: 0; pointer-events: none; }
.sidebar-header { padding: 22px 16px 18px; border-bottom: 1px solid var(--border-subtle); }
.brand { display: flex; align-items: center; gap: 11px; }
.brand-mark { display: grid; place-items: center; width: 34px; height: 34px; border-radius: 7px; background: var(--accent); color: #fff; font-size: 12px; font-weight: 800; }
.brand h1 { margin: 0; color: var(--text); font-size: 16px; font-weight: 700; }
.brand p { margin: 2px 0 0; color: var(--text-muted); font-size: 12px; }
.sidebar-new-button { display: flex; justify-content: center; width: 100%; margin-top: 20px; padding: 10px 12px; border: 1px solid var(--border-input); border-radius: 7px; background: var(--surface); color: var(--text); font-size: 13px; font-weight: 600; }
.sidebar-collapse-button { position: absolute; z-index: 1; top: 18px; right: 12px; display: grid; place-items: center; width: 30px; height: 30px; padding: 0; border: 1px solid var(--border-input); border-radius: 6px; background: var(--surface); color: var(--text-secondary); }
.history-heading { padding: 20px 16px 8px; color: var(--text-muted); font-size: 12px; font-weight: 700; }
.conversation-search { width: calc(100% - 24px); margin: 0 12px 10px; }
.conversation-list { flex: 1; min-height: 0; padding: 0 8px 16px; overflow-y: auto; }
.conversation-item { display: flex; align-items: stretch; margin-bottom: 3px; border-radius: 7px; }
.conversation-item.active { background: var(--surface-subtle); }
.conversation-select { min-width: 0; flex: 1; padding: 10px 8px; color: var(--text); text-align: left; }
.conversation-select strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
.conversation-delete { display: grid; place-items: center; width: 30px; padding: 0; color: var(--text-muted); }
.empty-history { margin: 12px 8px; color: var(--text-muted); font-size: 12px; }
.sidebar-workspace-button, .sidebar-settings-button { display: flex; align-items: center; gap: 8px; margin: 0 12px 8px; padding: 9px 12px; border: 1px solid var(--border-input); border-radius: 7px; background: var(--surface); color: var(--text-secondary); text-align: left; }
.sidebar-settings-button { margin-bottom: 14px; }
@media (max-width: 720px) { .sidebar-header { padding: 16px 12px; } .conversation-list { padding-right: 5px; padding-left: 5px; } }
</style>
