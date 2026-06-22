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
let pollTimer: ReturnType<typeof setInterval> | null = null

onMounted(async () => {
  // 先注册事件监听，避免 get_logs 和 listen 之间的窗口期丢失事件
  try {
    unlisten = await listen<{ path: string; line: string }>("dev_server_log", (event) => {
      if (event.payload.path === projectPath) {
        logs.value.push(event.payload.line)
      }
    })
  } catch (e) {
    console.error("listen dev_server_log failed, falling back to polling:", e)
    unlisten = null
    // 降级：轮询 get_dev_server_logs（2s 间隔）
    pollTimer = setInterval(async () => {
      try {
        const current = await invoke<string[]>("get_dev_server_logs", { path: projectPath })
        const seen = new Set(logs.value)
        for (const line of current) {
          if (!seen.has(line)) {
            logs.value.push(line)
          }
        }
      } catch (_) { /* ignore poll errors */ }
    }, 2000)
  }

  // 再拉取已缓冲的历史日志
  try {
    const initial = await invoke<string[]>("get_dev_server_logs", { path: projectPath })
    logs.value = initial
  } catch (e) {
    console.error("load logs failed:", e)
  }
})

onUnmounted(() => {
  if (unlisten) unlisten()
  if (pollTimer) clearInterval(pollTimer)
})

watch(logs, () => {
  nextTick(() => {
    if (logContainer.value) {
      logContainer.value.scrollTop = logContainer.value.scrollHeight
    }
  })
}, { deep: true })
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
