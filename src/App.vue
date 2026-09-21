<script setup>
import { nextTick, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NMessageProvider } from "naive-ui";

const isLaunching = ref(true);

onMounted(async () => {
  await nextTick();
  await new Promise((resolve) => requestAnimationFrame(resolve));
  try {
    await invoke("show_main_window");
  } catch (error) {
    console.error("显示主窗口失败", error);
  }
  window.setTimeout(() => {
    isLaunching.value = false;
  }, 700);
});
</script>

<template>
  <n-message-provider>
    <router-view />
    <Transition name="launch-screen">
      <div v-if="isLaunching" class="launch-screen" aria-live="polite" aria-label="AI-fast 正在启动">
        <div class="launch-grid"></div>
        <div class="launch-content">
          <div class="launch-mark" aria-hidden="true">
            <span></span><span></span><span></span>
          </div>
          <div class="launch-brand">AI-fast</div>
          <p>正在准备你的工作空间</p>
          <div class="launch-progress" aria-hidden="true"><span></span></div>
        </div>
      </div>
    </Transition>
  </n-message-provider>
</template>

<style scoped>
.launch-screen {
  position: fixed;
  z-index: 1000;
  inset: 0;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: #f6f7fb;
  color: #263244;
}

.launch-grid {
  position: absolute;
  inset: 0;
  opacity: 0.55;
  background-image: linear-gradient(#dfe4ec 1px, transparent 1px), linear-gradient(90deg, #dfe4ec 1px, transparent 1px);
  background-size: 36px 36px;
  mask-image: radial-gradient(ellipse 72% 58% at center, #000 10%, transparent 78%);
}

.launch-content {
  position: relative;
  display: flex;
  align-items: center;
  flex-direction: column;
  width: min(280px, calc(100vw - 48px));
  animation: launch-enter 520ms cubic-bezier(0.2, 0.8, 0.2, 1) both;
}

.launch-mark {
  display: grid;
  grid-template-columns: repeat(3, 12px);
  gap: 6px;
  margin-bottom: 18px;
}

.launch-mark span {
  width: 12px;
  height: 34px;
  border-radius: 3px;
  background: #1f6feb;
  animation: launch-pulse 1100ms ease-in-out infinite;
}

.launch-mark span:nth-child(2) {
  height: 46px;
  background: #258343;
  animation-delay: 120ms;
}

.launch-mark span:nth-child(3) {
  height: 26px;
  align-self: end;
  background: #f0a119;
  animation-delay: 240ms;
}

.launch-brand {
  font-size: 28px;
  font-weight: 700;
  letter-spacing: 0;
}

.launch-content p {
  margin: 8px 0 22px;
  color: #8792a3;
  font-size: 14px;
}

.launch-progress {
  width: 100%;
  height: 3px;
  overflow: hidden;
  border-radius: 2px;
  background: #dfe4ec;
}

.launch-progress span {
  display: block;
  width: 42%;
  height: 100%;
  border-radius: inherit;
  background: #1f6feb;
  animation: launch-progress 1100ms ease-in-out infinite;
}

.launch-screen-leave-active {
  transition: opacity 260ms ease;
}

.launch-screen-leave-to {
  opacity: 0;
}

@keyframes launch-enter {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes launch-pulse {
  0%, 100% { transform: scaleY(0.82); opacity: 0.7; }
  50% { transform: scaleY(1); opacity: 1; }
}

@keyframes launch-progress {
  from { transform: translateX(-110%); }
  to { transform: translateX(250%); }
}

@media (prefers-reduced-motion: reduce) {
  .launch-content,
  .launch-mark span,
  .launch-progress span {
    animation: none;
  }
}
</style>
