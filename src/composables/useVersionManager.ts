import { ref, onMounted, onUnmounted } from "vue"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import type { UnlistenFn } from "@tauri-apps/api/event"
import type { NodeVersion } from "../types"

export function useVersionManager() {
  const versions = ref<NodeVersion[]>([])
  const loading = ref(false)
  const refreshing = ref(false)
  const installingVersion = ref<string | null>(null)
  const deletingVersion = ref<string | null>(null)
  const installProgress = ref(0)
  const installStage = ref("")

  let unlistenVersions: UnlistenFn | null = null
  let unlistenInstallProgress: UnlistenFn | null = null

  function mergeWithLocal(remote: NodeVersion[]): NodeVersion[] {
    return remote.map((rv) => {
      const local = versions.value.find((v) => v.version === rv.version)
      return local ?? rv
    })
  }

  async function loadVersions() {
    loading.value = true
    try {
      const result = await invoke<NodeVersion[]>("get_versions")
      versions.value = result
    } catch (e) {
      console.error("get_versions failed:", e)
    }
    loading.value = false
  }

  async function onRefresh() {
    refreshing.value = true
    try {
      const result = await invoke<NodeVersion[]>("refresh_versions")
      versions.value = result
    } catch (e) {
      console.error("refresh failed:", e)
    }
    refreshing.value = false
  }

  async function onInstall(v: NodeVersion) {
    installingVersion.value = v.version
    installProgress.value = 0
    installStage.value = "正在下载..."
    try {
      await invoke("install_version", { version: v.version })
      installProgress.value = 100
      installStage.value = "安装完成"
      v.installed = true
      setTimeout(() => {
        installingVersion.value = null
      }, 1500)
    } catch (e) {
      console.error("install failed:", e)
      installingVersion.value = null
    }
  }

  async function onActivate(v: NodeVersion) {
    try {
      await invoke("activate_version", { version: v.version })
      const current = versions.value.find((x) => x.active)
      if (current) current.active = false
      v.active = true
    } catch (e) {
      console.error("activate failed:", e)
    }
  }

  async function onDelete(v: NodeVersion) {
    deletingVersion.value = v.version
    try {
      await invoke("delete_version", { version: v.version })
      const target = versions.value.find((x) => x.version === v.version)
      if (target) {
        target.installed = false
      }
    } catch (e) {
      console.error("delete failed:", e)
    } finally {
      deletingVersion.value = null
    }
  }

  function setup() {
    onMounted(async () => {
      unlistenVersions = await listen<NodeVersion[]>(
        "versions_updated",
        (event) => {
          versions.value = mergeWithLocal(event.payload)
        },
      )

      unlistenInstallProgress = await listen<{
        version: string
        stage: string
        percent: number
      }>("install_progress", (event) => {
        if (event.payload.version === installingVersion.value) {
          installProgress.value = event.payload.percent
          const stageMap: Record<string, string> = {
            downloading: "正在下载...",
            extracting: "正在解压...",
            done: "安装完成",
          }
          installStage.value = stageMap[event.payload.stage] || event.payload.stage
        }
      })

      loadVersions()
    })

    onUnmounted(() => {
      if (unlistenVersions) unlistenVersions()
      if (unlistenInstallProgress) unlistenInstallProgress()
    })
  }

  return {
    versions,
    loading,
    refreshing,
    installingVersion,
    deletingVersion,
    installProgress,
    installStage,
    loadVersions,
    onRefresh,
    onInstall,
    onActivate,
    onDelete,
    setup,
  }
}
