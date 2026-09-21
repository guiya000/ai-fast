<script setup>
import CloseIcon from "./icons/CloseIcon.vue";

defineProps({
  visible: { type: Boolean, default: false },
  title: { type: String, required: true },
  message: { type: String, required: true },
  confirmLabel: { type: String, required: true },
  cancelLabel: { type: String, required: true },
});

defineEmits(["confirm", "cancel"]);
</script>

<template>
  <Transition name="confirm" appear>
    <div v-if="visible" class="confirm-backdrop" @click.self="$emit('cancel')">
      <section class="confirm-dialog" role="alertdialog" aria-modal="true" :aria-label="title">
        <header class="confirm-header">
          <h2>{{ title }}</h2>
          <button class="confirm-close-button" type="button" :title="cancelLabel" :aria-label="cancelLabel" @click="$emit('cancel')"><CloseIcon /></button>
        </header>
        <p class="confirm-message">{{ message }}</p>
        <footer class="confirm-actions">
          <button class="secondary-button" type="button" @click="$emit('cancel')">{{ cancelLabel }}</button>
          <button class="danger-button" type="button" @click="$emit('confirm')">{{ confirmLabel }}</button>
        </footer>
      </section>
    </div>
  </Transition>
</template>

<style scoped>
.confirm-backdrop {
  position: fixed;
  z-index: 1000;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgb(15 23 42 / 42%);
}

.confirm-dialog {
  width: min(420px, 100%);
  padding: 22px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface);
  color: var(--text);
  box-shadow: 0 18px 50px rgb(15 23 42 / 20%);
}

.confirm-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.confirm-header h2 {
  margin: 0;
  font-size: 17px;
}

.confirm-close-button {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
}

.confirm-close-button:hover {
  background: var(--surface-muted);
  color: var(--text);
}

.confirm-message {
  margin: 18px 0 0;
  color: var(--text-secondary);
  font-size: 14px;
  line-height: 1.6;
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 9px;
  margin-top: 24px;
}

.secondary-button,
.danger-button {
  padding: 8px 14px;
  border-radius: 6px;
  font-size: 13px;
}

.secondary-button {
  border: 1px solid var(--border-input);
  background: var(--surface);
  color: var(--text-secondary);
}

.secondary-button:hover {
  background: var(--surface-muted);
}

.danger-button {
  border: 1px solid var(--danger);
  background: var(--danger);
  color: #fff;
}

.danger-button:hover {
  filter: brightness(0.92);
}

.confirm-enter-active,
.confirm-leave-active {
  transition: opacity 0.16s ease;
}

.confirm-enter-active .confirm-dialog,
.confirm-leave-active .confirm-dialog {
  transition: transform 0.16s ease, opacity 0.16s ease;
}

.confirm-enter-from,
.confirm-leave-to {
  opacity: 0;
}

.confirm-enter-from .confirm-dialog,
.confirm-leave-to .confirm-dialog {
  transform: translateY(8px);
  opacity: 0;
}
</style>
