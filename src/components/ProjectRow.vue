<script setup lang="ts">
import { ref, nextTick } from "vue"
import type { ProjectInfo } from "../types"

interface Props {
  project: ProjectInfo
  running: boolean
  starting: boolean
}

defineProps<Props>()

const emit = defineEmits<{
  start: [project: ProjectInfo]
  stop: [project: ProjectInfo]
  openLog: [project: ProjectInfo]
  unbind: [project: ProjectInfo]
  updateName: [project: ProjectInfo, newName: string]
}>()

const editing = ref(false)
const nameEl = ref<HTMLElement | null>(null)
let originalName = ""

function startEdit(project: ProjectInfo) {
  originalName = project.name
  editing.value = true
  nextTick(() => {
    nameEl.value?.focus()
    // 全选文字方便直接覆盖输入
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

function cancelEdit(project: ProjectInfo) {
  editing.value = false
  nextTick(() => {
    if (nameEl.value) nameEl.value.textContent = originalName
  })
}

function onKeydown(e: KeyboardEvent, project: ProjectInfo) {
  if (e.key === "Enter") {
    e.preventDefault()
    nameEl.value?.blur()
  } else if (e.key === "Escape") {
    e.preventDefault()
    cancelEdit(project)
  }
}
</script>

<template>
  <!-- 单条项目条目，显示项目名 + 路径和开发服务启动/停止按钮 -->
  <div class="project-row">
    <div class="row-content">
      <!-- 左侧：项目名（可编辑别名）+ 项目路径（上下分布） -->
      <div class="row-left">
        <div class="name-line">
          <span
            ref="nameEl"
            class="project-name"
            :contenteditable="editing"
            :spellcheck="false"
            @keydown="onKeydown($event, project)"
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
        <!-- 仅在服务未运行时显示删除按钮，避免运行中误删 -->
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
</style>
