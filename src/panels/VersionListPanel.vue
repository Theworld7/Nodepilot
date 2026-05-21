<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import {
  Search,
  Cell,
  Button,
  Tag,
  Switch,
  PullDownRefresh,
  Loading,
  Progress,
  DialogPlugin as Dialog,
} from "tdesign-mobile-vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DownloadIcon, CheckCircleIcon, DeleteIcon } from "tdesign-icons-vue-next";
import type { NodeVersion } from "../types";
import type { UnlistenFn } from "@tauri-apps/api/event";

const searchValue = ref("");
const showLtsOnly = ref(true);
const loading = ref(false);
const refreshing = ref(false);

const versions = ref<NodeVersion[]>([]);

const installingVersion = ref<string | null>(null);
const installProgress = ref(0);
const installStage = ref("");

let unlistenVersions: UnlistenFn | null = null;
let unlistenInstallProgress: UnlistenFn | null = null;

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

function getStatusLabel(v: NodeVersion): string {
  if (v.active) return "使用中";
  if (v.installed) return "已安装";
  return "可安装";
}

function getStatusTheme(
  v: NodeVersion,
): "default" | "primary" | "success" | "warning" | "danger" {
  if (v.active) return "success";
  if (v.installed) return "primary";
  return "default";
}

async function onRefresh() {
  refreshing.value = true;
  try {
    const result = await invoke<NodeVersion[]>("refresh_versions");
    versions.value = result;
  } catch (e) {
    console.error("refresh failed:", e);
  }
  refreshing.value = false;
}

function mergeWithLocal(remote: NodeVersion[]): NodeVersion[] {
  return remote.map((rv) => {
    const local = versions.value.find((v) => v.version === rv.version);
    return local ?? rv;
  });
}

async function onInstall(v: NodeVersion) {
  installingVersion.value = v.version;
  installProgress.value = 0;
  installStage.value = "正在下载...";
  try {
    await invoke("install_version", { version: v.version });
    installProgress.value = 100;
    installStage.value = "安装完成";
    v.installed = true;
    setTimeout(() => {
      installingVersion.value = null;
    }, 1500);
  } catch (e) {
    console.error("install failed:", e);
    installingVersion.value = null;
  }
}

async function onActivate(v: NodeVersion) {
  try {
    await invoke("activate_version", { version: v.version });
    const current = versions.value.find((x) => x.active);
    if (current) current.active = false;
    v.active = true;
  } catch (e) {
    console.error("activate failed:", e);
  }
}

async function onDelete(v: NodeVersion) {
  const confirmed = await new Promise<boolean>((resolve) => {
    Dialog.confirm({
      title: "确认删除",
      content: `确定删除 Node.js ${v.version}？`,
      confirmBtn: "删除",
      cancelBtn: "取消",
      onConfirm: () => resolve(true),
      onClose: () => resolve(false),
    });
  });
  if (!confirmed) return;
  try {
    await invoke("delete_version", { version: v.version });
    const idx = versions.value.indexOf(v);
    if (idx > -1) versions.value.splice(idx, 1);
  } catch (e) {
    console.error("delete failed:", e);
  }
}

async function loadVersions() {
  loading.value = true;
  try {
    const result = await invoke<NodeVersion[]>("get_versions");
    versions.value = result;
  } catch (e) {
    console.error("get_versions failed:", e);
  }
  loading.value = false;
}

onMounted(async () => {
  unlistenVersions = await listen<NodeVersion[]>(
    "versions_updated",
    (event) => {
      versions.value = mergeWithLocal(event.payload);
    },
  );

  unlistenInstallProgress = await listen<{
    version: string;
    stage: string;
    percent: number;
  }>("install_progress", (event) => {
    if (event.payload.version === installingVersion.value) {
      installProgress.value = event.payload.percent;
      const stageMap: Record<string, string> = {
        downloading: "正在下载...",
        extracting: "正在解压...",
        done: "安装完成",
      };
      installStage.value = stageMap[event.payload.stage] || event.payload.stage;
    }
  });

  loadVersions();
});

onUnmounted(() => {
  if (unlistenVersions) unlistenVersions();
  if (unlistenInstallProgress) unlistenInstallProgress();
});
</script>

<template>
  <div class="panel-scroll">
    <PullDownRefresh v-model="refreshing" @refresh="onRefresh">
      <div class="panel">
        <div class="panel-head">
          <div class="panel-header">
            <div class="panel-title">Node.js 版本管理</div>
            <div class="lts-toggle">
              <span class="lts-label">仅 LTS</span>
              <Switch v-model="showLtsOnly" size="small" />
            </div>
          </div>

          <div class="search-wrapper">
            <Search
              v-model="searchValue"
              shape="round"
              size="small"
              placeholder="搜索主版本号，如 24"
              clearable
            />
          </div>
        </div>

        <div v-if="installingVersion" class="install-progress">
          <div class="progress-version">
            正在安装 {{ installingVersion }}...
          </div>
          <Progress :percentage="installProgress" />
          <div class="progress-stage">{{ installStage }}</div>
        </div>

        <div v-if="loading" class="loading-state">
          <Loading />
        </div>

        <div v-else class="version-list">
          <div
            v-for="v in filteredVersions"
            :key="v.version"
            class="version-row"
          >
            <Cell @click="v.installed && !v.active ? onActivate(v) : undefined">
              <template #title>
                <div class="cell-title-row">
                  <span class="version-name">{{ v.version }}</span>
                  <Tag v-if="v.lts" theme="warning" size="small" class="lts-tag"
                    >LTS</Tag
                  >
                  <Tag :theme="getStatusTheme(v)" size="small">
                    {{ getStatusLabel(v) }}
                  </Tag>
                </div>
              </template>
              <template #description>{{ v.date }}</template>
              <template #note>
                <template v-if="v.installed && !v.active">
                  <Button
                    size="extra-small"
                    theme="primary"
                    variant="text"
                    @click.stop="onActivate(v)"
                  >
                    <CheckCircleIcon />
                  </Button>
                  <Button
                    size="extra-small"
                    theme="danger"
                    variant="text"
                    @click.stop="onDelete(v)"
                  >
                    <DeleteIcon />
                  </Button>
                </template>
                <Button
                  v-else-if="!v.installed"
                  size="extra-small"
                  variant="text"
                  :loading="installingVersion === v.version"
                  @click.stop="onInstall(v)"
                >
                  <DownloadIcon />
                </Button>
              </template>
            </Cell>
          </div>
        </div>
      </div>
    </PullDownRefresh>
  </div>
</template>

<style scoped>
.panel-scroll {
  height: 100%;
}

.panel-scroll :deep(.t-pull-down-refresh) {
  overflow: visible;
}

.panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px;
  background: rgba(255, 255, 255, 0.8);
  box-sizing: border-box;
}

.panel-head {
  background: #f7f7f7;
  border-radius: 16px;
  margin-bottom: 12px;
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
  color: var(--text-h, #08060d);
}

.lts-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
}

.lts-label {
  font-size: 12px;
  color: var(--text, #6b6375);
}

.search-wrapper {
  padding: 0 8px;
  margin-bottom: 8px;
}

.search-wrapper :deep(.t-search__input-box) {
  background-color: #e5e5e5;
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
  background: #f7f7f7;
  border-radius: 16px;
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

.version-row :deep(.t-cell) {
  background: transparent;
}

.version-row :deep(.t-cell__right) {
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
  padding-left: 8px;
}

.install-progress {
  padding: 12px 16px;
  background: var(--code-bg, #f4f3ec);
  margin: 8px 16px;
  border-radius: 8px;
}

.progress-version {
  font-size: 12px;
  margin-bottom: 8px;
  color: var(--text, #6b6375);
}

.progress-stage {
  font-size: 11px;
  color: var(--text, #6b6375);
  margin-top: 4px;
  text-align: right;
}
</style>
