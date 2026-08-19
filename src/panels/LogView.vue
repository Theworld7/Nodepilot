<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { MessagePlugin } from "tdesign-vue-next"
import { openUrl } from "@tauri-apps/plugin-opener"

const params = new URLSearchParams(window.location.search)
const projectPath = params.get("path") || ""
const projectName = params.get("name") || "项目"

const logs = ref<string[]>([])
const logContainer = ref<HTMLElement | null>(null)
let unlisten: UnlistenFn | null = null
let pollTimer: ReturnType<typeof setInterval> | null = null

// --- 多选状态 ---
const selectionMode = ref(false)
const selectedIndices = ref<number[]>([])
const lastClickedIndex = ref<number | null>(null)

// --- ANSI 转 HTML ---

/** HTML 转义，防止 XSS */
function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
}

/** 剥离 ANSI 转义码，保留纯文本（供复制到剪贴板使用） */
function stripAnsi(raw: string): string {
  let text = raw
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1a\x1c-\x1f]/g, "")
    .replace(/^\^D/, "")
  // 移除 ANSI CSI 序列
  return text.replace(/\x1b\[[\d;]*[a-zA-Z]/g, "")
}

/**
 * 将含 ANSI 转义码的日志文本转换为带颜色样式的 HTML。
 */
function ansiToHtml(raw: string): string {
  let text = raw
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1a\x1c-\x1f]/g, "")
    .replace(/^\^D/, "")

  const tokens = text.split(/(\x1b\[[\d;]*[a-zA-Z])/)
  let html = ""
  const stack: string[] = []

  for (const token of tokens) {
    if (token.startsWith("\x1b[")) {
      const code = token.slice(2, -1)
      if (code === "0" || code === "" || code === "39") {
        for (let i = stack.length - 1; i >= 0; i--) html += stack[i]
        stack.length = 0
      } else {
        const params = code.split(";").map(Number)
        const styles: string[] = []
        for (const p of params) {
          if (p === 1) styles.push("font-weight:700")
          else if (p === 22) styles.push("font-weight:400")
          else if (p === 2) styles.push("opacity:0.6")
          else if (p === 3) styles.push("font-style:italic")
          else if (p === 4) styles.push("text-decoration:underline")
          else if (p === 24) styles.push("text-decoration:none")
          else if (p >= 30 && p <= 37) styles.push(`color:var(--ansi-${p - 30})`)
          else if (p >= 90 && p <= 97) styles.push(`color:var(--ansi-bright-${p - 90})`)
          else if (p >= 40 && p <= 47) styles.push(`background:var(--ansi-${p - 40})`)
          else if (p >= 100 && p <= 107) styles.push(`background:var(--ansi-bright-${p - 100})`)
        }
        if (styles.length) {
          html += `<span style="${styles.join(";")}">`
          stack.push("</span>")
        }
      }
    } else {
      html += escapeTextPreservingLinks(token)
    }
  }

  for (let i = stack.length - 1; i >= 0; i--) html += stack[i]
  return html
}

/** 链接注入的 <a> 标签不带真实 href（避免日志窗口被导航走），仅通过 data-href 携带目标。 */

/**
 * 在原始日志文本（含 ANSI 转义码）中识别 http/https 链接，并在链接首尾注入
 * <a class="log-link" data-href="...">…</a>。
 * 识别基于「剥离 ANSI 后的纯文本」再映射回原始文本位置，从而容忍链接内部夹杂
 * ANSI 转义码（如 Vite 的彩色/加粗 Local 行）。
 */
function linkifyRaw(raw: string): string {
  const plain: string[] = []
  const rawPos: number[] = []
  let i = 0
  while (i < raw.length) {
    if (raw[i] === "\x1b" && raw[i + 1] === "[") {
      let j = i + 2
      while (j < raw.length && !/[a-zA-Z]/.test(raw[j])) j++
      i = j < raw.length ? j + 1 : raw.length
      continue
    }
    plain.push(raw[i])
    rawPos.push(i)
    i++
  }
  const plainStr = plain.join("")
  if (!plainStr) return raw

  const urlRe = /https?:\/\/[^\s"'<>()[\]{}]+/gi
  const injections: { at: number; tag: string }[] = []
  let m: RegExpExecArray | null
  while ((m = urlRe.exec(plainStr))) {
    let end = m.index + m[0].length
    while (end > m.index && /[.,;:!?]/.test(plainStr[end - 1])) end--
    if (end <= m.index) continue
    const href = plainStr.slice(m.index, end)
    const startRaw = rawPos[m.index]
    const endRaw = rawPos[end - 1] + 1
    injections.push({
      at: startRaw,
      tag: `<a class="log-link" data-href="${escapeHtml(href)}">`,
    })
    injections.push({ at: endRaw, tag: "</a>" })
  }
  if (injections.length === 0) return raw

  injections.sort((a, b) => a.at - b.at)
  let out = ""
  let cursor = 0
  for (const inj of injections) {
    out += raw.slice(cursor, inj.at)
    out += inj.tag
    cursor = inj.at
  }
  out += raw.slice(cursor)
  return out
}

/** 转义普通文本，但保留注入的链接标签（<a class="log-link" …> 与 </a>）。 */
function escapeTextPreservingLinks(s: string): string {
  const re = /<a class="log-link" data-href="[^"]*">|<\/a>/g
  let out = ""
  let last = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(s))) {
    out += escapeHtml(s.slice(last, m.index))
    out += m[0]
    last = m.index + m[0].length
  }
  out += escapeHtml(s.slice(last))
  return out
}

const renderedLogs = computed(() => logs.value.map((line) => ansiToHtml(linkifyRaw(line))))

// --- 多选交互 ---

/** 将 Set 去重的索引排序后写回 selectedIndices，触发 checkbox-group 响应 */
function commitIndices(indices: Set<number>) {
  selectedIndices.value = [...indices].sort((a, b) => a - b)
}

function enterSelectionMode(index: number) {
  selectionMode.value = true
  selectedIndices.value = [index]
  lastClickedIndex.value = index
}

/** 从头部「选择」按钮进入选择模式，不预选任何行 */
function enterSelectionModeFromHeader() {
  selectionMode.value = true
  selectedIndices.value = []
  lastClickedIndex.value = null
}

function removeIndex(index: number) {
  selectedIndices.value = selectedIndices.value.filter((i) => i !== index)
  lastClickedIndex.value = index
}

function addIndex(index: number) {
  const set = new Set(selectedIndices.value)
  set.add(index)
  commitIndices(set)
  lastClickedIndex.value = index
}

function rangeSelect(from: number, to: number) {
  const set = new Set(selectedIndices.value)
  const [start, end] = from < to ? [from, to] : [to, from]
  for (let i = start; i <= end; i++) set.add(i)
  commitIndices(set)
}

function handleLineNumClick(index: number, event: MouseEvent) {
  if (!selectionMode.value) {
    enterSelectionMode(index)
    return
  }

  if (event.shiftKey && lastClickedIndex.value !== null) {
    rangeSelect(lastClickedIndex.value, index)
  } else if (selectedIndices.value.includes(index)) {
    removeIndex(index)
  } else {
    addIndex(index)
  }
}

function cancelSelection() {
  selectionMode.value = false
  selectedIndices.value = []
  lastClickedIndex.value = null
}

async function copySelected() {
  const sorted = [...selectedIndices.value].sort((a, b) => a - b)
  const lines = sorted.map((i) => stripAnsi(logs.value[i] ?? ""))
  try {
    await navigator.clipboard.writeText(lines.join("\n"))
  } catch (_) {
    // 降级：Tauri 桌面环境 clipboard API 不可用时无提示，不退出选择模式
    return
  }

  MessagePlugin.success("复制成功")

  // 短暂延迟让用户看到提示，再退出选择模式
  setTimeout(() => {
    selectionMode.value = false
    selectedIndices.value = []
    lastClickedIndex.value = null
  }, 200)
}

const allSelected = computed(() => {
  return logs.value.length > 0 && selectedIndices.value.length === logs.value.length
})

function toggleSelectAll() {
  if (allSelected.value) {
    selectedIndices.value = []
  } else {
    selectedIndices.value = logs.value.map((_, i) => i)
  }
  lastClickedIndex.value = null
}

/** 清空日志：清后端缓冲（避免轮询回退时被重新拉回）+ 清前端显示。 */
async function clearLogs() {
  try {
    await invoke("clear_dev_server_logs", { path: projectPath })
    logs.value = []
    MessagePlugin.success("日志已清空")
  } catch (e) {
    console.warn("clear logs failed:", e)
    MessagePlugin.error("清理日志失败")
  }
}

/** 点击日志中的链接：在系统默认浏览器打开，不触发日志窗口导航。 */
function handleLogClick(e: MouseEvent) {
  const target = e.target as Element | null
  if (!target || typeof target.closest !== "function") return
  const link = target.closest("a.log-link") as HTMLElement | null
  const href = link?.dataset?.href
  if (!href) return
  e.preventDefault()
  openUrl(href).catch((err) => console.warn("打开链接失败:", err))
}

onMounted(async () => {
  try {
    unlisten = await listen<{ path: string; line: string }>("dev_server_log", (event) => {
      if (event.payload.path === projectPath) {
        logs.value.push(event.payload.line)
      }
    })
  } catch (e) {
    console.error("listen dev_server_log failed, falling back to polling:", e)
    unlisten = null
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
      <div class="log-header-row">
        <span class="log-path">{{ projectPath }}</span>
        <div class="log-header-actions">
          <template v-if="selectionMode">
            <span class="log-select-count">已选 {{ selectedIndices.length }} 行</span>
            <t-button size="small" variant="text" @click="toggleSelectAll">
              {{ allSelected ? '取消全选' : '全选' }}
            </t-button>
            <t-button size="small" variant="text" @click="copySelected">复制</t-button>
            <t-button size="small" variant="text" @click="cancelSelection">取消</t-button>
          </template>
          <template v-else>
            <t-button size="small" variant="text" @click="enterSelectionModeFromHeader">
              选择
            </t-button>
            <t-button
              size="small"
              variant="text"
              :disabled="logs.length === 0"
              @click="clearLogs"
            >
              清理
            </t-button>
          </template>
        </div>
      </div>
    </div>
    <div ref="logContainer" class="log-body" :class="{ 'selection-mode': selectionMode }" @click="handleLogClick">
      <div v-if="logs.length === 0" class="log-empty">等待日志...</div>
      <t-checkbox-group v-model="selectedIndices" class="log-checkbox-group">
        <div
          v-for="(html, i) in renderedLogs"
          :key="i"
          class="log-line"
          :class="{ 'log-line-selected': selectedIndices.includes(i) }"
        >
          <!-- 复选框槽位：始终渲染，固定 20px 宽。进入选择模式时内部放置 checkbox -->
          <span class="log-checkbox-slot">
            <t-checkbox
              v-if="selectionMode"
              :value="i"
              :checked="selectedIndices.includes(i)"
              class="log-checkbox"
            />
          </span>
          <span
            class="log-num"
            :class="{ 'log-num-clickable': !selectionMode }"
            @click="handleLineNumClick(i, $event)"
          >{{ i + 1 }}</span>
          <pre class="log-text" v-html="html" />
        </div>
      </t-checkbox-group>
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

  /* ANSI 标准 16 色（针对暗色终端调优） */
  --ansi-0: #000000;
  --ansi-1: #cd3131;  /* red   — error */
  --ansi-2: #0dbc79;  /* green — info/success */
  --ansi-3: #e5e510;  /* yellow — warn */
  --ansi-4: #2472c8;  /* blue */
  --ansi-5: #bc3fbc;  /* magenta */
  --ansi-6: #11a8cd;  /* cyan */
  --ansi-7: #e5e5e5;  /* white — default text */

  /* 高亮变体（90-97） */
  --ansi-bright-0: #666666;
  --ansi-bright-1: #f14c4c;
  --ansi-bright-2: #23d18b;
  --ansi-bright-3: #f5f543;
  --ansi-bright-4: #3b8eea;
  --ansi-bright-5: #d670d6;
  --ansi-bright-6: #29b8db;
  --ansi-bright-7: #ffffff;
}

.log-header {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 16px;
  height: 56px;
  background: #252526;
  border-bottom: 1px solid #333;
  flex-shrink: 0;
  box-sizing: border-box;
}

.log-title {
  font-size: 14px;
  font-weight: 600;
  line-height: 18px;
  color: #e0e0e0;
}

.log-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 20px;
}

.log-path {
  font-size: 11px;
  line-height: 20px;
  color: #888;
  /* 始终占据空间，避免按钮出现时路径被挤宽导致视觉抖动 */
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.log-header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  /* 按钮区域固定宽度避免显隐时路径挤压 */
  white-space: nowrap;
}

.log-select-count {
  font-size: 11px;
  color: #aaa;
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

/* checkbox-group 不产生额外布局影响 */
.log-checkbox-group {
  display: contents;
}

.log-line {
  display: flex;
  align-items: center;
  padding: 0 16px 0 8px;
  line-height: 1.6;
}

.log-line:hover {
  background: #2a2a2a;
}

.log-line-selected {
  background: #094771;
}

.log-line-selected:hover {
  background: #0b5590;
}

/* checkbox 槽位：始终渲染，固定 20px 宽，checkbox 右对齐，margin-right 独立控制与行号的间距 */
.log-checkbox-slot {
  width: 20px;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  margin-right: 6px;
}

/* checkbox 本身锁定 16px，隐藏内部 label 文字 */
.log-checkbox {
  width: 16px;
  height: 16px;
  line-height: 1;
}

.log-checkbox :deep(.t-checkbox__label) {
  display: none;
}

.log-checkbox :deep(.t-checkbox__input) {
  width: 16px;
  height: 16px;
}

.log-num {
  min-width: 1.5em;
  text-align: right;
  padding-right: 12px;
  color: #555;
  user-select: none;
  flex-shrink: 0;
}

/* 非选中模式下行号可点击提示 */
.log-num-clickable {
  cursor: pointer;
}

.log-num-clickable:hover {
  color: #ccc;
}

.log-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
  color: #d4d4d4;
}

/* 日志中的可点击链接。
   注意：链接是 v-html 注入的元素，不带组件作用域属性，scoped 选择器命中不了，
   必须用 :deep 穿透作用域才能应用样式。 */
.log-body :deep(.log-link) {
  color: #4fc1ff;
  text-decoration: underline;
  cursor: pointer;
}

.log-body :deep(.log-link:hover) {
  color: #7ad0ff;
}

/* 暗色背景下的按钮文字强制浅色 */
.log-header-actions :deep(.t-button) {
  color: #d4d4d4;
}

/* hover 使用暗色友好的半透明背景，替代默认亮色背景 */
.log-header-actions :deep(.t-button:hover),
.log-header-actions :deep(.t-button--variant-text:hover) {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.08);
}

/* 点击按压态同样适配暗色 */
.log-header-actions :deep(.t-button:active),
.log-header-actions :deep(.t-button--variant-text:active) {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.12);
}
</style>
