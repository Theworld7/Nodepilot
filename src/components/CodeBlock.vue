<script setup lang="ts">
import { ref, computed } from 'vue'
import { CheckIcon, CopyIcon } from 'tdesign-icons-vue-next'

interface Props {
  code: string
  language?: string
}

const props = defineProps<Props>()
const copied = ref(false)

const copyIcon = computed(() => (copied.value ? CheckIcon : CopyIcon))

async function copyToClipboard() {
  try {
    const text = props.code ?? ''
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.setAttribute('readonly', '')
      ta.style.position = 'absolute'
      ta.style.left = '-9999px'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
  } catch (e) {
    console.error('copy failed', e)
  }
}

const displayCode = computed(() => props.code ?? '')
</script>

<template>
  <div class="code-block" role="region" aria-label="代码块">
    <t-button
      class="copy-btn"
      variant="text"
      shape="circle"
      size="extra-small"
      @click="copyToClipboard"
    >
        <template #icon>
            <component :is="copyIcon" />
        </template>
    </t-button>

    <pre class="code-pre"><code>{{ displayCode }}</code></pre>
  </div>
</template>

<style scoped>
.code-block {
  position: relative;
  background: var(--code-bg);
  border-radius: var(--radius-sm);
  padding: 12px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, "Roboto Mono", "Courier New", monospace;
  font-size: 12px;
  color: var(--text);
}

.copy-btn {
  position: absolute;
  top: -12px;
  right: -12px;
}

.copy-btn :deep(.t-button__icon) {
  font-size: 18px;
}

.code-pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.code-pre code {
  display: block;
  font-family: inherit;
  font-size: inherit;
}
</style>
