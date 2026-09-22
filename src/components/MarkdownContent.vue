<script setup>
import { computed } from "vue";
import { open } from "@tauri-apps/plugin-shell";
import { useMessage } from "naive-ui";
import DOMPurify from "dompurify";
import MarkdownIt from "markdown-it";
import { copyFor } from "../i18n";

const props = defineProps({
  content: { type: String, default: "" },
  locale: { type: String, default: "zh-CN" },
});

const message = useMessage();
const copy = computed(() => copyFor(props.locale));
const markdown = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: true,
});

markdown.renderer.rules.fence = (tokens, index) => {
  const token = tokens[index];
  const encodedContent = encodeURIComponent(token.content);
  const copyIcon = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="10" height="10" rx="2"></rect><path d="M5 15V5a2 2 0 0 1 2-2h10"></path></svg>';
  return `<div class="markdown-code-block"><button type="button" class="markdown-code-copy" data-code-copy="${encodedContent}" title="${copy.value.copyCode}" aria-label="${copy.value.copyCode}">${copyIcon}</button><pre><code>${markdown.utils.escapeHtml(token.content)}</code></pre></div>`;
};

function renderMarkdown(content) {
  return DOMPurify.sanitize(markdown.render(content || ""));
}

async function handleClick(event) {
  const copyButton = event.target.closest("[data-code-copy]");
  if (copyButton) {
    try {
      await navigator.clipboard.writeText(decodeURIComponent(copyButton.dataset.codeCopy || ""));
      message.success(copy.value.copySuccess, { duration: 1800 });
    } catch (error) {
      message.error(`${copy.value.copyFailed}：${String(error)}`, { duration: 2000 });
    }
    return;
  }
  const link = event.target.closest("a");
  if (!link) return;
  const href = link.getAttribute("href");
  if (!/^https?:\/\//i.test(href || "")) return;
  event.preventDefault();
  try {
    await open(href);
  } catch (error) {
    message.error(`${copy.value.openLinkFailed}：${String(error)}`, { duration: 2000 });
  }
}
</script>

<template>
  <div class="markdown-body" v-html="renderMarkdown(props.content)" @click="handleClick"></div>
</template>

<style scoped>
.markdown-body { min-width: 0; line-height: 1.65; overflow-wrap: anywhere; }
.markdown-body :deep(p) { margin: 0 0 12px; }
.markdown-body :deep(p:last-child) { margin-bottom: 0; }
.markdown-body :deep(h1), .markdown-body :deep(h2), .markdown-body :deep(h3) { margin: 18px 0 10px; line-height: 1.35; }
.markdown-body :deep(h1) { font-size: 1.35em; }
.markdown-body :deep(h2) { font-size: 1.2em; }
.markdown-body :deep(h3) { font-size: 1.08em; }
.markdown-body :deep(ul), .markdown-body :deep(ol) { margin: 8px 0 14px; padding-left: 1.5em; }
.markdown-body :deep(li + li) { margin-top: 5px; }
.markdown-body :deep(blockquote) { margin: 12px 0; padding-left: 14px; border-left: 3px solid #9abbe9; color: var(--text-secondary); }
.markdown-body :deep(a) { color: var(--accent-dark); text-decoration: underline; }
.markdown-body :deep(code) { padding: 2px 5px; border-radius: 4px; background: rgb(31 111 235 / 9%); font-size: 0.86em; }
.markdown-body :deep(.markdown-code-block) { position: relative; margin: 12px 0; overflow: hidden; border-radius: 7px; background: var(--code-surface); color: #e5e7eb; }
.markdown-body :deep(.markdown-code-copy) { position: absolute; z-index: 1; top: 8px; right: 10px; display: grid; width: 30px; height: 30px; place-items: center; padding: 0; border: 1px solid rgb(255 255 255 / 34%); border-radius: 5px; background: rgb(17 24 39 / 72%); color: #d7dee8; cursor: pointer; }
.markdown-body :deep(.markdown-code-copy:hover) { border-color: rgb(255 255 255 / 72%); background: rgb(17 24 39 / 92%); color: #fff; }
.markdown-body :deep(.markdown-code-copy svg) { display: block; width: 17px; height: 17px; }
.markdown-body :deep(.markdown-code-block pre) { margin: 0; padding: 48px 12px 12px; overflow-x: auto; line-height: 1.5; }
.markdown-body :deep(.markdown-code-block pre code) { padding: 0; background: transparent; color: inherit; font-size: 0.78em; white-space: pre; }
.markdown-body :deep(table) { display: block; width: max-content; min-width: 100%; max-width: 100%; margin: 14px 0; overflow-x: auto; border-collapse: collapse; font-size: 0.78em; line-height: 1.45; }
.markdown-body :deep(th), .markdown-body :deep(td) { min-width: 90px; padding: 8px 10px; border: 1px solid var(--border-input); text-align: left; vertical-align: top; }
.markdown-body :deep(th) { background: var(--accent-soft); font-weight: 700; }
.markdown-body :deep(tr:nth-child(even) td) { background: rgb(234 241 251 / 42%); }
.markdown-body :deep(hr) { margin: 16px 0; border: 0; border-top: 1px solid var(--border); }
</style>
