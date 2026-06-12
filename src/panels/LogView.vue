<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import type { UnlistenFn } from "@tauri-apps/api/event"

const params = new URLSearchParams(window.location.search)
const projectPath = params.get("path") || ""
const projectName = params.get("name") || "项目"

const logs = ref<string[]>([])
const logContainer = ref<HTMLElement | null>(null)
let unlisten: UnlistenFn | null = null

onMounted(async () => {
  try {
    const initial = await invoke<string[]>("get_dev_server_logs", { path: projectPath })
    logs.value = initial
  } catch (e) {
    console.error("load logs failed:", e)
  }

  unlisten = await listen<{ path: string; line: string }>("dev_server_log", (event) => {
    if (event.payload.path === projectPath) {
      logs.value.push(event.payload.line)
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

watch(logs, () => {
  nextTick(() => {
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight
    }
  })
})
</script>

<template>
  <div class="log-view">
    <div class="log-header">
      <span class="log-title">{{ projectName }}</span>
      <span class="log-path">{{ projectPath }}</span>
    </div>
    <div ref="logContainer" class="log-body">
      <div v-if="logs.length === 0" class="log-empty">等待日志...</div>
      <div v-for="(line, i) in logs" :key="i" class="log-line">
        <span class="log-num">{{ i + 1 }}</span>
        <pre class="log-text">{{ line }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.log-view {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #1e1e1e;
  color: #d4d4d4;
  font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
  font-size: 13px;
}

.log-header {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 16px;
  background: #252526;
  border-bottom: 1px solid #333;
  flex-shrink: 0;
}

.log-title {
  font-size: 14px;
  font-weight: 600;
  color: #e0e0e0;
}

.log-path {
  font-size: 11px;
  color: #888;
}

.log-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
}

.log-empty {
  padding: 16px;
  color: #666;
  text-align: center;
}

.log-line {
  display: flex;
  padding: 0 16px;
  line-height: 1.6;
}

.log-line:hover {
  background: #2a2a2a;
}

.log-num {
  min-width: 3em;
  text-align: right;
  padding-right: 12px;
  color: #555;
  user-select: none;
  flex-shrink: 0;
}

.log-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
  color: #d4d4d4;
}
</style>
