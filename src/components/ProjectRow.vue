<script setup lang="ts">
import { ref, reactive, nextTick } from "vue"
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

// ---- 项目设置 Drawer ----
const showSettings = ref(false)
const scripts = ref<Record<string, string>>({})
const formData = reactive({
  selectedScript: "",
  prefixInput: "",
})
const loadingScripts = ref(false)

async function openSettings() {
  loadingScripts.value = true
  scripts.value = {}
  formData.selectedScript = ""
  formData.prefixInput = ""

  // 预填已保存的配置
  if (props.project.default_script) {
    formData.selectedScript = props.project.default_script
  }
  if (props.project.command_prefix) {
    formData.prefixInput = props.project.command_prefix
  }

  try {
    const pkg = await invoke<{ scripts?: Record<string, string> }>("read_package_json", {
      path: props.project.path,
    })
    if (pkg?.scripts) {
      scripts.value = pkg.scripts
      // 如果没有保存的配置但项目有 scripts，默认选中第一个
      if (!formData.selectedScript) {
        const keys = Object.keys(scripts.value)
        if (keys.length > 0) {
          formData.selectedScript = keys[0]
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
  const script = formData.selectedScript.trim() || null
  const prefix = formData.prefixInput.trim() || null
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
            shape="circle"
            variant="text"
            @click.stop="startEdit(project)"
          >
            <EditIcon />
          </t-button>
          <!-- 项目设置图标按钮，在编辑按钮旁 -->
          <t-button
            v-if="!editing"
            class="edit-trigger"
            shape="circle"
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
          shape="circle"
          theme="primary"
          variant="text"
          @click.stop="emit('start', project)"
        >
          <PlayCircleFilledIcon />
        </t-button>
        <t-button
          v-else-if="starting"
          shape="circle"
          variant="text"
          disabled
        >
          <LoadingSpinner />
        </t-button>
        <template v-else>
          <t-button
            shape="circle"
            variant="text"
            @click.stop="emit('openLog', project)"
          >
            <CodeIcon />
          </t-button>
          <t-button
            shape="circle"
            theme="danger"
            variant="text"
            @click.stop="emit('stop', project)"
          >
            <StopCircleFilledIcon />
          </t-button>
        </template>
        <t-button
          v-if="!running && !starting"
          shape="circle"
          theme="danger"
          variant="text"
          @click.stop="emit('unbind', project)"
        >
          <DeleteIcon />
        </t-button>
      </div>
    </div>

    <!-- 项目设置 Drawer -->
    <t-drawer v-model:visible="showSettings" placement="bottom" header="项目配置">
      <!-- 加载中 -->
      <div v-if="loadingScripts" class="settings-loading">
        <LoadingSpinner />
      </div>

      <t-form v-else :data="formData" label-align="top" class="settings-form">
        <t-form-item label="默认执行命令" name="selectedScript">
          <t-radio-group v-model="formData.selectedScript" variant="default-filled">
            <t-radio-button
              v-for="(cmd, key) in scripts"
              :key="key"
              :value="key"
              :label="cmd"
            >
              {{ key }}
            </t-radio-button>
          </t-radio-group>
          <!-- 无脚本或需要自定义时，允许手动输入 -->
          <div v-if="Object.keys(scripts).length === 0" class="settings-empty">
            该项目无可用脚本，请在下方输入自定义命令名
          </div>
          <t-input
            v-if="!formData.selectedScript"
            v-model="formData.selectedScript"
            placeholder="输入自定义脚本名，如 dev"
            class="settings-custom-input"
          />
        </t-form-item>

        <t-form-item label="命令前缀" name="prefixInput">
          <t-input
            v-model="formData.prefixInput"
            placeholder="可选，如 tauri"
            clearable
          />
        </t-form-item>
      </t-form>

      <!-- 底部按钮 -->
      <template #footer>
          <t-button variant="outline" @click="cancelSettings">取消</t-button>
          <t-button theme="primary" @click="saveSettings">保存</t-button>
      </template>
    </t-drawer>
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
  margin-right: 12px;
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

/* 设置表单样式 */
.settings-loading {
  display: flex;
  justify-content: center;
  padding: 40px 0;
}

.settings-form :deep(.t-form__item) {
  margin-bottom: 20px;
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
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
