<script setup lang="ts">
import { ref, nextTick } from "vue"
import { invoke } from "@tauri-apps/api/core"
import type { ProjectInfo } from "../types"

interface Props {
  project: ProjectInfo
  running: boolean
  starting: boolean
}

const props = defineProps<Props>()

const emit = defineEmits<{
  start: [project: ProjectInfo]
  stop: [project: ProjectInfo]
  openLog: [project: ProjectInfo]
  unbind: [project: ProjectInfo]
  updateName: [project: ProjectInfo, newName: string]
  updateConfig: []
}>()

// ---- 别名编辑 ----
const editing = ref(false)
const nameEl = ref<HTMLElement | null>(null)
let originalName = ""

function startEdit(project: ProjectInfo) {
  originalName = project.name
  editing.value = true
  nextTick(() => {
    nameEl.value?.focus()
    const sel = window.getSelection()
    sel?.selectAllChildren(nameEl.value!)
  })
}

function confirmEdit(project: ProjectInfo) {
  if (!editing.value) return
  editing.value = false
  const text = nameEl.value?.textContent?.trim() ?? ""
  if (text && text !== originalName) {
    emit("updateName", project, text)
  }
}

function cancelEdit() {
  editing.value = false
  nextTick(() => {
    if (nameEl.value) nameEl.value.textContent = originalName
  })
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault()
    nameEl.value?.blur()
  } else if (e.key === "Escape") {
    e.preventDefault()
    cancelEdit()
  }
}

// ---- 项目设置 Popup ----
const showSettings = ref(false)
const scripts = ref<Record<string, string>>({})
const selectedScript = ref<string>("")
const prefixInput = ref("")
const loadingScripts = ref(false)

async function openSettings() {
  loadingScripts.value = true
  scripts.value = {}
  scripts.value = {}
  selectedScript.value = ""
  prefixInput.value = ""

  // 预填已保存的配置
  if (props.project.default_script) {
    selectedScript.value = props.project.default_script
  }
  if (props.project.command_prefix) {
    prefixInput.value = props.project.command_prefix
  }

  try {
    const pkg = await invoke<{ scripts?: Record<string, string> }>("read_package_json", {
      path: props.project.path,
    })
    if (pkg?.scripts) {
      scripts.value = pkg.scripts
      // 如果没有保存的配置但项目有 scripts，默认选中第一个
      if (!selectedScript.value) {
        const keys = Object.keys(scripts.value)
        if (keys.length > 0) {
          selectedScript.value = keys[0]
        }
      }
    } else {
      scripts.value = {}
    }
  } catch {
    // package.json 读取失败，script 列表为空，用户可手动输入
    scripts.value = {}
  }

  loadingScripts.value = false
  showSettings.value = true
}

async function saveSettings() {
  const script = selectedScript.value.trim() || null
  const prefix = prefixInput.value.trim() || null
  try {
    await invoke("update_project_config", {
      version: props.project.version,
      path: props.project.path,
      defaultScript: script,
      commandPrefix: prefix,
    })
    emit("updateConfig")
  } catch (e) {
    console.error("update project config failed:", e)
  }
  showSettings.value = false
}

function cancelSettings() {
  showSettings.value = false
}
</script>

<template>
  <div class="project-row">
    <div class="row-content">
      <!-- 左侧：项目名（可编辑别名）+ 项目路径 -->
      <div class="row-left">
        <div class="name-line">
          <span
            ref="nameEl"
            class="project-name"
            :contenteditable="editing"
            :spellcheck="false"
            @keydown="onKeydown($event)"
            @blur="confirmEdit(project)"
          >{{ project.name }}</span>
          <t-button
            v-if="!editing"
            class="edit-trigger"
            size="extra-small"
            variant="text"
            @click.stop="startEdit(project)"
          >
            <EditIcon />
          </t-button>
          <!-- 项目设置图标按钮，在编辑按钮旁 -->
          <t-button
            v-if="!editing"
            class="edit-trigger"
            size="extra-small"
            variant="text"
            @click.stop="openSettings"
          >
            <SettingIcon />
          </t-button>
        </div>
        <span class="project-path">{{ project.path }}</span>
      </div>

      <!-- 右侧：启动 / 停止 / 删除按钮 -->
      <div class="row-actions">
        <t-button
          v-if="!running && !starting"
          size="extra-small"
          theme="primary"
          variant="text"
          @click.stop="emit('start', project)"
        >
          <PlayCircleFilledIcon />
        </t-button>
        <t-button
          v-else-if="starting"
          size="extra-small"
          variant="text"
          disabled
        >
          <t-loading theme="spinner" />
        </t-button>
        <template v-else>
          <t-button
            size="extra-small"
            variant="text"
            @click.stop="emit('openLog', project)"
          >
            <CodeIcon />
          </t-button>
          <t-button
            size="extra-small"
            theme="danger"
            variant="text"
            @click.stop="emit('stop', project)"
          >
            <StopCircleFilledIcon />
          </t-button>
        </template>
        <t-button
          v-if="!running && !starting"
          size="extra-small"
          theme="danger"
          variant="text"
          @click.stop="emit('unbind', project)"
        >
          <DeleteIcon />
        </t-button>
      </div>
    </div>

    <!-- 项目设置 Popup -->
    <t-popup v-model:visible="showSettings" placement="bottom">
      <template #content>
        <div class="settings-panel">
          <!-- 加载中 -->
          <div v-if="loadingScripts" class="settings-loading">
            <t-loading theme="spinner" />
          </div>

          <template v-else>
            <div class="settings-header">项目配置</div>

            <div class="settings-body">
              <!-- 默认执行命令 -->
              <div class="settings-group">
                <div class="settings-label">默认执行命令</div>
                <t-radio-group v-model="selectedScript">
                  <div
                    v-for="(cmd, key) in scripts"
                    :key="key"
                    class="script-card"
                    :class="{ 'script-card--active': selectedScript === key }"
                  >
                    <CheckIcon v-if="selectedScript === key" class="script-card__check" />
                    <t-radio
                      :value="key"
                      borderless
                      icon="none"
                      :label="key"
                      :content="cmd"
                    />
                  </div>
                </t-radio-group>
                <!-- 无脚本或需要自定义时，允许手动输入 -->
                <div v-if="Object.keys(scripts).length === 0" class="settings-empty">
                  该项目无可用脚本，请在下方输入自定义命令名
                </div>
                <t-input
                  v-if="!selectedScript"
                  v-model="selectedScript"
                  placeholder="输入自定义脚本名，如 dev"
                  class="settings-custom-input"
                />
              </div>

              <!-- 命令前缀 -->
              <div class="settings-group">
                <div class="settings-label">命令前缀</div>
                <t-input
                  v-model="prefixInput"
                  placeholder="可选，如 tauri"
                  clearable
                />
              </div>
            </div>

            <!-- 底部按钮 -->
            <div class="settings-footer">
              <t-button variant="outline" @click="cancelSettings">取消</t-button>
              <t-button theme="primary" @click="saveSettings">保存</t-button>
            </div>
          </template>
        </div>
      </template>
    </t-popup>
  </div>
</template>

<style scoped>
.project-row {
  padding: 0 16px;
}

.project-row :deep(.t-collapse-panel) {
  background: transparent;
}

.row-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 0;
}

.row-left {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.project-name {
  font-size: 13px;
  font-weight: 500;
  line-height: 28px;
  color: var(--text-h);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.name-line {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
  min-height: 28px;
}

.edit-trigger {
  flex-shrink: 0;
  opacity: 0;
  transition: opacity 0.15s;
}

.name-line:hover .edit-trigger {
  opacity: 1;
}

.project-name[contenteditable="true"] {
  background: var(--bg);
  outline: 1px solid var(--brand);
  border-radius: 2px;
  padding: 0 4px;
  margin: 0 -4px;
}

.project-path {
  font-size: 11px;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

/* 设置面板样式 */
.settings-panel {
  display: flex;
  flex-direction: column;
  max-height: 65vh;
  background: var(--surface);
}

.settings-header {
  flex-shrink: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--text-h);
  padding: 20px 16px 0;
  margin-bottom: 16px;
}

.settings-body {
  flex: 1;
  overflow-y: auto;
  padding: 0 16px;
}

.settings-loading {
  display: flex;
  justify-content: center;
  padding: 40px 0;
}

.settings-group {
  margin-bottom: 16px;
}

.settings-label {
  font-size: 13px;
  color: var(--text);
  margin-bottom: 8px;
}

.settings-group :deep(.t-radio-group) {
  display: flex;
  flex-direction: column;
  gap: 0;
}

/* 纵向卡片样式 */
.script-card {
  position: relative;
  margin: 0 0 12px 0;
  border-radius: 8px;
  overflow: hidden;
  box-sizing: border-box;
  border: 1.5px solid var(--border);
  transition: border-color 0.2s;
}

.script-card:last-child {
  margin-bottom: 0;
}

.script-card--active {
  border-color: var(--brand);
}

.script-card--active::after {
  content: '';
  display: block;
  position: absolute;
  left: 0;
  top: 0;
  width: 0;
  height: 0;
  border: 14px solid var(--brand);
  border-bottom-color: transparent;
  border-right-color: transparent;
}

.script-card__check {
  display: block;
  color: #fff;
  position: absolute;
  left: 1.5px;
  top: 1.5px;
  z-index: 1;
  font-size: 14px;
}

.settings-empty {
  font-size: 12px;
  color: var(--text-light);
  margin: 8px 0;
}

.settings-custom-input {
  margin-top: 8px;
}

.settings-footer {
  flex-shrink: 0;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 16px;
  border-top: 1px solid var(--border);
}
</style>
