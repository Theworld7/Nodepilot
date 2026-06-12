<script setup lang="ts">
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
}>()
</script>

<template>
  <!-- 单条项目条目，显示项目名 + 路径和开发服务启动/停止按钮 -->
  <div class="project-row">
    <div class="row-content">
      <!-- 左侧：项目名 + 项目路径（上下分布） -->
      <div class="row-left">
        <span class="project-name">{{ project.name }}</span>
        <span class="project-path">{{ project.path }}</span>
      </div>
      <!-- 右侧：启动 / 停止按钮（互斥显示） -->
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
  color: var(--text-h);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
