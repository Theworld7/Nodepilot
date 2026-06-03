<script setup lang="ts">
import { Cell, Tag, Button, Loading } from "tdesign-mobile-vue"
import { DownloadIcon, CheckCircleIcon, DeleteIcon } from "tdesign-icons-vue-next"
import type { NodeVersion } from "../types"

interface Props {
  version: NodeVersion
  installing: boolean
  deleting: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  install: [version: NodeVersion]
  activate: [version: NodeVersion]
  delete: [version: NodeVersion]
}>()

function getStatusLabel(v: NodeVersion): string {
  if (v.active) return "使用中"
  if (v.installed) return "已安装"
  return "可安装"
}

function getStatusTheme(
  v: NodeVersion,
): "default" | "primary" | "success" | "warning" | "danger" {
  if (v.active) return "success"
  if (v.installed) return "primary"
  return "default"
}
</script>

<template>
  <div class="version-row">
    <Cell
      @click="
        version.installed && !version.active
          ? emit('activate', version)
          : undefined
      "
    >
      <template #title>
        <div class="cell-title-row">
          <span class="version-name">{{ version.version }}</span>
          <Tag
            v-if="version.lts"
            theme="warning"
            size="small"
            class="lts-tag"
          >LTS</Tag>
          <Tag :theme="getStatusTheme(version)" size="small">
            {{ getStatusLabel(version) }}
          </Tag>
        </div>
      </template>
      <template #description>{{ version.date }}</template>
      <template #note>
        <template v-if="version.installed && !version.active">
          <Button
            size="extra-small"
            theme="primary"
            variant="text"
            @click.stop="emit('activate', version)"
          >
            <CheckCircleIcon />
          </Button>
          <Button
            size="extra-small"
            theme="danger"
            variant="text"
            :disabled="deleting"
            @click.stop="emit('delete', version)"
          >
            <Loading v-if="deleting" theme="spinner" />
            <DeleteIcon v-else />
          </Button>
        </template>
        <Button
          v-else-if="!version.installed"
          size="extra-small"
          variant="text"
          :disabled="installing"
          @click.stop="emit('install', version)"
        >
          <Loading v-if="installing" theme="spinner" />
          <DownloadIcon v-else />
        </Button>
      </template>
    </Cell>
  </div>
</template>

<style scoped>
.version-row :deep(.t-cell) {
  background: transparent;
}

.version-row :deep(.t-cell__right) {
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
  padding-left: 8px;
}

.cell-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.version-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-h, #08060d);
}

.lts-tag {
  flex-shrink: 0;
}
</style>
