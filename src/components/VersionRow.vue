<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { NodeVersion, ProjectInfo } from "../types";
import ProjectRow from "./ProjectRow.vue";

interface Props {
  version: NodeVersion;
  installing: boolean;
  deleting: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  install: [version: NodeVersion];
  activate: [version: NodeVersion];
  delete: [version: NodeVersion];
}>();

const projects = ref<ProjectInfo[]>([]);
const activePanels = ref<string[]>([]);
const runningServers = ref<Set<string>>(new Set());
const startingServers = ref<Set<string>>(new Set());

async function loadProjects() {
  try {
    const all = await invoke<ProjectInfo[]>("get_project_bindings");
    projects.value = all.filter((p) => p.version === props.version.version);
  } catch (e) {
    console.error("load projects failed:", e);
  }
}

async function handleStart(p: ProjectInfo) {
  try {
    startingServers.value = new Set(startingServers.value).add(p.path);

    let script = p.default_script || undefined;
    // 未配置时 fallback 到 package.json 的 dev 脚本
    if (!script) {
      const pkg = await invoke<{ scripts?: Record<string, string> }>(
        "read_package_json",
        {
          path: p.path,
        },
      );
      if (!pkg?.scripts?.dev) {
        console.error("no dev script in package.json");
        return;
      }
      script = pkg.scripts.dev;
    }

    const pm = await invoke<string>("detect_pm", { path: p.path });
    const prefix = p.command_prefix ? `${p.command_prefix} ` : "";
    const cmd =
      pm === "npm" ? `npm run ${prefix}${script}` : `${pm} ${prefix}${script}`;

    await invoke("start_dev_server", { path: p.path, command: cmd });
    runningServers.value = new Set(runningServers.value).add(p.path);
  } catch (e) {
    console.error("start dev server failed:", e);
  } finally {
    const next = new Set(startingServers.value);
    next.delete(p.path);
    startingServers.value = next;
  }
}

async function handleStop(p: ProjectInfo) {
  try {
    await invoke("stop_dev_server", { path: p.path });
    const next = new Set(runningServers.value);
    next.delete(p.path);
    runningServers.value = next;
  } catch (e) {
    console.error("stop dev server failed:", e);
  }
}

async function handleUnbind(p: ProjectInfo) {
  const ok = await confirm(
    `确定要移除项目「${p.name}」的绑定吗？\n\n路径：${p.path}\n\n此操作不可撤销。`,
    {
      title: "移除项目",
      kind: "warning",
      okLabel: "确认移除",
      cancelLabel: "取消",
    },
  );
  if (!ok) return;
  try {
    await invoke("unbind_project", {
      version: props.version.version,
      path: p.path,
    });
    await loadProjects();
  } catch (e) {
    console.error("unbind project failed:", e);
  }
}

async function handleUpdateName(p: ProjectInfo, newName: string) {
  try {
    await invoke("update_project_name", {
      version: props.version.version,
      path: p.path,
      newName,
    });
    await loadProjects();
  } catch (e) {
    console.error("update project name failed:", e);
  }
}

function safeLabel(s: string): string {
  let hash = 0;
  for (let i = 0; i < s.length; i++) {
    hash = ((hash << 5) - hash + s.charCodeAt(i)) | 0;
  }
  return `log-${Math.abs(hash).toString(36)}`;
}

async function handleOpenLog(p: ProjectInfo) {
  const label = safeLabel(p.path);
  const win = new WebviewWindow(label, {
    url: `/?view=log&path=${encodeURIComponent(p.path)}&name=${encodeURIComponent(p.name)}`,
    width: 680,
    height: 480,
    title: `日志 - ${p.name}`,
  });
  win.once("tauri://error", (e) => console.error("log window error:", e));
  win.once("tauri://created", () => {});
}

async function handleProjectBind() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择项目目录",
    });
    if (!selected) return;
    const dirName =
      selected.split("/").pop() || selected.split("\\").pop() || "项目";
    await invoke("bind_project", {
      version: props.version.version,
      name: dirName,
      path: selected,
    });
    await loadProjects();
    activePanels.value = [props.version.version];
  } catch (e) {
    console.error("bind project failed:", e);
  }
}

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

onMounted(loadProjects);
</script>

<template>
  <!-- 单条版本条目，包裹在折叠面板中；折叠内容展示绑定到该版本的项目列表 -->
  <div class="version-row" :class="{ 'no-projects': projects.length === 0 }">
    <t-collapse
      v-model="activePanels"
      :borderless="true"
      expand-icon-placement="right"
    >
      <t-collapse-panel
        :value="version.version"
        expand-icon-placement="right"
        borderless
      >
        <!-- 折叠面板头部：版本号 + LTS 标签 + 状态标签 + 发布日期 -->
        <template #header>
          <div
            class="row-content"
            @click.stop="
              version.installed && !version.active
                ? emit('activate', version)
                : undefined
            "
          >
            <!-- 第一行：版本号 + 标签行 -->
            <div class="row-tags">
              <span class="version-name">{{ version.version }}</span>
              <!-- LTS 版本标记 -->
              <t-tag v-if="version.lts" theme="warning" size="small">LTS</t-tag>
              <!-- 状态标签：使用中 / 已安装 / 可安装 -->
              <t-tag :theme="getStatusTheme(version)" size="small">
                {{ getStatusLabel(version) }}
              </t-tag>
            </div>
            <!-- 第二行：发布日期 -->
            <div class="row-date">{{ version.date }}</div>
          </div>
        </template>
        <!-- 头部右侧插槽（箭头图标左侧）：操作按钮区域 -->
        <template #headerRightContent>
          <div class="row-actions">
            <!-- 已安装状态：项目绑定按钮始终显示 -->
            <template v-if="version.installed">
              <!-- 项目绑定：将当前 Node 版本关联到项目目录 -->
              <t-button
                variant="text"
                shape="circle"
                @click.stop="handleProjectBind"
              >
                <Folder1Icon />
              </t-button>
              <!-- 未激活时额外显示激活 / 删除按钮 -->
              <template v-if="!version.active">
                <t-button
                  theme="primary"
                  variant="text"
                  shape="circle"
                  @click.stop="emit('activate', version)"
                >
                  <CheckCircleIcon />
                </t-button>
                <t-button
                  theme="danger"
                  variant="text"
                  shape="circle"
                  :disabled="deleting"
                  @click.stop="emit('delete', version)"
                >
                  <LoadingSpinner v-if="deleting" />
                  <DeleteIcon v-else />
                </t-button>
              </template>
            </template>
            <!-- 未安装 → 下载 -->
            <t-button
              v-else
              variant="text"
              shape="circle"
              :disabled="installing"
              @click.stop="emit('install', version)"
            >
              <LoadingSpinner v-if="installing" />
              <DownloadIcon v-else />
            </t-button>
          </div>
        </template>
        <!-- 折叠面板内容：绑定到该版本的项目列表 -->
        <div v-if="projects.length" class="panel-projects">
          <ProjectRow
            v-for="p in projects"
            :key="p.path"
            :project="p"
            :running="runningServers.has(p.path)"
            :starting="startingServers.has(p.path)"
            @start="handleStart"
            @stop="handleStop"
            @open-log="handleOpenLog"
            @unbind="handleUnbind"
            @update-name="handleUpdateName"
            @update-config="loadProjects"
          />
        </div>
      </t-collapse-panel>
    </t-collapse>
  </div>
</template>

<style scoped>
.version-row :deep(.t-collapse-panel) {
  background: transparent;
}

.version-row.no-projects :deep(.t-collapse-panel__icon) {
  display: none;
}

.row-content {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.row-tags {
  display: flex;
  align-items: center;
  gap: 8px;
}

.row-date {
  font-size: 12px;
  color: var(--text);
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.version-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-h);
  white-space: nowrap;
}

.version-row :deep(.t-collapse-panel__content) {
  padding: 0;
}

.panel-projects {
  border-top: 1px solid var(--border);
  padding: 8px 0;
}
</style>
