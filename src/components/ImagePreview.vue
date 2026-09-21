<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { useMessage } from "naive-ui";
import CloseIcon from "./icons/CloseIcon.vue";
import DownloadIcon from "./icons/DownloadIcon.vue";
import { copyFor } from "../i18n";

const props = defineProps({
  image: { type: Object, required: true },
  locale: { type: String, default: "zh-CN" },
});

const message = useMessage();
const visible = ref(false);
const scale = ref(1);
const copy = computed(() => copyFor(props.locale));

function imageFileInfo(dataUrl, fallbackName) {
  const metadata = dataUrl.match(/^data:image\/([a-z0-9.+-]+);/i);
  const remoteExtension = !metadata && dataUrl.match(/\.([a-z0-9]+)(?:[?#]|$)/i)?.[1];
  const format = metadata?.[1]?.toLowerCase() || remoteExtension?.toLowerCase() || "png";
  const extension = format === "jpeg" ? "jpg" : format === "svg+xml" ? "svg" : format;
  const baseName = (fallbackName || "image").replace(/\.[^.]+$/, "").replace(/[\\/:*?"<>|]+/g, "-") || "image";
  return { extension, name: `${baseName}.${extension}` };
}

async function imageBytes(dataUrl) {
  if (/^https?:\/\//i.test(dataUrl)) {
    const response = await fetch(dataUrl);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
  }
  const [, encoded = ""] = dataUrl.split(",", 2);
  if (dataUrl.includes(";base64,")) {
    const binary = atob(encoded);
    return Uint8Array.from(binary, (character) => character.charCodeAt(0));
  }
  return new TextEncoder().encode(decodeURIComponent(encoded));
}

async function downloadImage() {
  if (!props.image?.dataUrl) return;
  try {
    const { extension, name } = imageFileInfo(props.image.dataUrl, props.image.name);
    const path = await save({
      defaultPath: name,
      filters: [{ name: `${extension.toUpperCase()} image`, extensions: [extension] }],
    });
    if (!path) return;
    await writeFile(path, await imageBytes(props.image.dataUrl));
    message.success(copy.value.saveImage, { duration: 1800 });
  } catch (error) {
    console.error("保存图片失败", error);
    message.error(`${copy.value.imageSaveFailed}：${String(error)}`, { duration: 2500 });
  }
}

function open() {
  scale.value = 1;
  visible.value = true;
}

function close() {
  visible.value = false;
}

function zoom(delta) {
  scale.value = Math.min(4, Math.max(0.5, Number((scale.value + delta).toFixed(2))));
}

function handleKeydown(event) {
  if (!visible.value) return;
  if (event.key === "Escape") close();
  else if (event.key === "+" || event.key === "=") zoom(0.25);
  else if (event.key === "-") zoom(-0.25);
}

onMounted(() => window.addEventListener("keydown", handleKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", handleKeydown));

defineExpose({ open, close });
</script>

<template>
  <button
    type="button"
    class="image-preview-trigger"
    :title="image.name || copy.generatedImage"
    @click="open"
  >
    <img :src="image.dataUrl" :alt="image.name || copy.generatedImage" />
  </button>
  <Teleport to="body">
    <div v-if="visible" class="image-preview-overlay" role="dialog" aria-modal="true" @click="close">
      <div class="image-preview-stage">
        <img
          class="image-preview-image"
          :src="image.dataUrl"
          :alt="image.name || copy.generatedImage"
          :style="{ transform: `scale(${scale})` }"
          @click.stop
        />
        <div class="image-preview-toolbar" role="toolbar" :aria-label="copy.imagePreviewToolbar" @click.stop>
          <button type="button" :title="copy.zoomOut" :aria-label="copy.zoomOut" :disabled="scale <= 0.5" @click="zoom(-0.25)">−</button>
          <span class="image-preview-scale">{{ Math.round(scale * 100) }}%</span>
          <button type="button" :title="copy.zoomIn" :aria-label="copy.zoomIn" :disabled="scale >= 4" @click="zoom(0.25)">+</button>
          <button type="button" class="image-preview-download" :title="copy.saveImage" :aria-label="copy.saveImage" @click="downloadImage"><DownloadIcon /></button>
          <button type="button" class="image-preview-close" :title="copy.close" :aria-label="copy.close" @click="close"><CloseIcon /></button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.image-preview-trigger {
  display: block;
  width: 100%;
  max-width: 320px;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  overflow: hidden;
  cursor: zoom-in;
}

.image-preview-trigger img {
  display: block;
  width: 100%;
  max-height: 320px;
  object-fit: contain;
  background: var(--surface-subtle);
}

.image-preview-overlay {
  position: fixed;
  z-index: 3000;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 32px;
  background: rgb(0 0 0 / 78%);
}

.image-preview-stage {
  position: relative;
  z-index: 1;
  display: flex;
  width: min(92vw, 1100px);
  height: 100%;
  max-height: 100%;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.image-preview-image {
  display: block;
  max-width: 88vw;
  max-height: calc(100vh - 120px);
  object-fit: contain;
  transition: transform 0.15s ease;
  user-select: none;
  pointer-events: auto;
}

.image-preview-toolbar {
  position: fixed;
  z-index: 2;
  bottom: 24px;
  left: 50%;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 48px;
  padding: 6px 10px;
  border: 1px solid rgb(255 255 255 / 18%);
  border-radius: 8px;
  background: rgb(20 24 30 / 92%);
  color: #fff;
  transform: translateX(-50%);
  pointer-events: auto;
}

.image-preview-toolbar button {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  padding: 0;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: #fff;
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
}

.image-preview-toolbar button:hover:not(:disabled),
.image-preview-toolbar button:focus-visible {
  background: rgb(255 255 255 / 16%);
  outline: none;
}

.image-preview-toolbar button:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.image-preview-toolbar .image-preview-download :deep(svg),
.image-preview-toolbar .image-preview-close :deep(svg) {
  width: 18px;
  height: 18px;
}

.image-preview-scale {
  min-width: 48px;
  color: rgb(255 255 255 / 78%);
  font-size: 12px;
  text-align: center;
}

.image-preview-close {
  margin-left: 4px;
}

@media (max-width: 640px) {
  .image-preview-overlay { padding: 16px; }
  .image-preview-image { max-width: 96vw; max-height: calc(100vh - 108px); }
  .image-preview-toolbar { bottom: 16px; }
}
</style>
