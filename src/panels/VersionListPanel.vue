<script setup lang="ts">
import { ref, computed } from "vue";
import type { NodeVersion } from "../types";
import VersionRow from "../components/VersionRow.vue";
import { useTheme } from "../composables/useTheme";
import { useVersionManager } from "../composables/useVersionManager";

const { isDark, toggleTheme } = useTheme();

const searchValue = ref("");
const showLtsOnly = ref(true);

const {
  versions,
  loading,
  refreshing,
  installingVersion,
  deletingVersion,
  installProgress,
  installStage,
  onRefresh,
  onInstall,
  onActivate,
  onDelete,
  setup,
} = useVersionManager();

setup();

const filteredVersions = computed(() => {
  let list = versions.value;
  if (showLtsOnly.value) {
    list = list.filter((v) => v.lts !== false);
  }
  if (searchValue.value) {
    const major = searchValue.value.replace("v", "");
    list = list.filter((v) => v.version.startsWith(`v${major}`));
  }
  return list;
});

// 删除确认对话框状态
const deleteDialogVisible = ref(false);
const deleteTarget = ref<NodeVersion | null>(null);

function openDeleteDialog(v: NodeVersion) {
  deleteTarget.value = v;
  deleteDialogVisible.value = true;
}

function confirmDelete() {
  deleteDialogVisible.value = false;
  if (deleteTarget.value) {
    onDelete(deleteTarget.value);
  }
}

function cancelDelete() {
  deleteDialogVisible.value = false;
}
</script>

<template>
  <div class="panel-scroll">
    <div class="panel-scroll">
      <div class="panel">
        <div class="panel-head">
          <div class="panel-header">
            <div class="panel-title">Node.js 版本管理</div>
            <div class="header-actions">
              <div class="lts-toggle">
                <span class="lts-label">仅 LTS</span>
                <t-switch v-model="showLtsOnly" />
              </div>
              <t-divider layout="vertical" />
              <t-button
                class="refresh-btn"
                shape="circle"
                variant="text"
                :disabled="refreshing"
                @click="onRefresh"
              >
                <LoadingSpinner v-if="refreshing" />
                <RefreshIcon v-else />
              </t-button>
              <t-button
                shape="circle"
                variant="text"
                @click="toggleTheme"
              >
                <ModeDarkFilledIcon v-if="!isDark" />
                <ModeLightFilledIcon v-else />
              </t-button>
            </div>
          </div>

          <div class="search-wrapper">
            <t-input
              v-model="searchValue"
              placeholder="搜索主版本号，如 24"
              clearable
            >
              <template #prefix-icon>
                <SearchIcon />
              </template>
            </t-input>
          </div>
        </div>

        <div v-if="installingVersion" class="install-progress">
          <div class="progress-version">
            正在安装 {{ installingVersion }}...
          </div>
          <t-progress :percentage="Math.round(installProgress * 10) / 10">
            <template #label>
              <span class="progress-pct-label">{{ installProgress.toFixed(1) }}%</span>
            </template>
          </t-progress>
          <div class="progress-stage">{{ installStage }}</div>
        </div>

        <div v-if="loading" class="loading-state">
          <LoadingSpinner size="32px" />
        </div>

        <div v-else class="version-list">
          <VersionRow
            v-for="v in filteredVersions"
            :key="v.version"
            :version="v"
            :installing="installingVersion === v.version"
            :deleting="deletingVersion === v.version"
            @install="onInstall"
            @activate="onActivate"
            @delete="openDeleteDialog"
          />
        </div>
      </div>
    </div>

    <t-dialog
      v-model:visible="deleteDialogVisible"
      header="确认删除"
      :width="'calc(100vw - 48px)'"
      :confirm-btn="{ content: '删除', theme: 'danger' }"
      cancel-btn="取消"
      @confirm="confirmDelete"
      @cancel="cancelDelete"
      @close="cancelDelete"
    >
      <template v-if="deleteTarget">
        确定删除 Node.js {{ deleteTarget.version }}？
      </template>
    </t-dialog>
  </div>
</template>

<style scoped>
.panel-scroll {
  height: 100%;
}

.panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px;
  background: var(--surface-alt);
  box-sizing: border-box;
}

.panel-head {
  background: var(--surface);
  border-radius: var(--radius-lg);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06), 0 1px 8px rgba(0, 0, 0, 0.04);
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px 8px;
}

.panel-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-h);
}

.lts-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
}

.lts-label {
  font-size: 12px;
  color: var(--text);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.header-actions :deep(.t-divider--vertical) {
  height: 16px;
}

.search-wrapper {
  padding: 0 8px;
  margin-bottom: 8px;
}

.search-wrapper :deep(.t-input) {
  border-radius: 20px;
}

.loading-state {
  display: flex;
  justify-content: center;
  padding: 40px 0;
}

.version-list {
  flex: 1;
  overflow-y: auto;
  padding: 0;
  background: var(--surface);
  border-radius: var(--radius-lg);
  margin-top: 12px;
}

.install-progress {
  padding: 12px 16px;
  background: var(--code-bg);
  margin-top: 12px;
  border-radius: var(--radius-lg);
}

.progress-version {
  font-size: 12px;
  margin-bottom: 8px;
  color: var(--text);
}

.progress-stage {
  font-size: 11px;
  color: var(--text);
  margin-top: 4px;
  text-align: right;
}

.progress-pct-label {
  display: inline-block;
  min-width: 3.2em;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
</style>
