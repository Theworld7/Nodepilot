<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { DialogPlugin } from "tdesign-mobile-vue";
import { listen } from "@tauri-apps/api/event";
import type { NodeVersion } from "../types";
import type { UnlistenFn } from "@tauri-apps/api/event";
import VersionRow from "../components/VersionRow.vue";
import { useTheme } from "../composables/useTheme";

const { isDark, toggleTheme } = useTheme();

const searchValue = ref("");
const showLtsOnly = ref(true);
const loading = ref(false);
const refreshing = ref(false);

const versions = ref<NodeVersion[]>([]);

const installingVersion = ref<string | null>(null);
const deletingVersion = ref<string | null>(null);
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
    DialogPlugin.confirm({
      title: "确认删除",
      content: `确定删除 Node.js ${v.version}？`,
      confirmBtn: "删除",
      cancelBtn: "取消",
      onConfirm: () => resolve(true),
      onClose: () => resolve(false),
    });
  });
  if (!confirmed) return;
  deletingVersion.value = v.version;
  try {
    await invoke("delete_version", { version: v.version });
    const target = versions.value.find((x) => x.version === v.version);
    if (target) {
      target.installed = false;
    }
  } catch (e) {
    console.error("delete failed:", e);
  } finally {
    deletingVersion.value = null;
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
    <t-pull-down-refresh v-model="refreshing" @refresh="onRefresh">
      <div class="panel">
        <div class="panel-head">
          <div class="panel-header">
            <div class="panel-title">Node.js 版本管理</div>
            <div class="header-actions">
              <div class="lts-toggle">
                <span class="lts-label">仅 LTS</span>
                <t-switch v-model="showLtsOnly" size="small" />
              </div>
              <t-divider layout="vertical" />
              <t-button
                class="theme-btn"
                variant="text"
                shape="circle"
                size="small"
                @click="toggleTheme"
              >
                <ModeDarkFilledIcon v-if="!isDark" />
                <ModeLightFilledIcon v-else />
              </t-button>
            </div>
          </div>

          <div class="search-wrapper">
            <t-search
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
          <t-progress :percentage="Math.round(installProgress * 10) / 10">
            <template #label>
              <span class="progress-pct-label">{{ installProgress.toFixed(1) }}%</span>
            </template>
          </t-progress>
          <div class="progress-stage">{{ installStage }}</div>
        </div>

        <div v-if="loading" class="loading-state">
          <t-loading />
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
            @delete="onDelete"
          />
        </div>
      </div>
    </t-pull-down-refresh>
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
  background: var(--surface-alt);
  box-sizing: border-box;
}

.panel-head {
  background: var(--surface);
  border-radius: var(--radius-lg);
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

.theme-btn {
  color: var(--text);
}

.theme-btn :deep(.t-button__icon) {
  font-size: 18px;
  transform: translateY(-1px);
}

.search-wrapper {
  padding: 0 8px;
  margin-bottom: 8px;
}

.search-wrapper :deep(.t-search__input-box) {
  background-color: var(--input-bg);
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
