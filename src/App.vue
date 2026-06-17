<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { confirm } from '@tauri-apps/plugin-dialog'
import LogView from './panels/LogView.vue'
import VersionListPanel from './panels/VersionListPanel.vue'

const params = new URLSearchParams(window.location.search)
const isLogView = ref(params.get('view') === 'log')
const setupError = ref<string | null>(null)

onMounted(async () => {
  if (isLogView.value) return

  try {
    const done = await invoke<boolean>('is_auto_setup_done')
    if (!done) {
      // Check if there's a stored error from the initial attempt
      const err = await invoke<string | null>('get_setup_error')
      if (err) setupError.value = err

      while (true) {
        const retry = await confirm(
          `nodepilot 需要配置终端环境才能自动接管 Node.js。\n\n` +
          (setupError.value ? `上次失败原因：${setupError.value}\n\n` : '') +
          `是否重试？`,
          { title: '环境配置', kind: 'warning', okLabel: '重试', cancelLabel: '跳过' }
        )
        if (!retry) {
          // User skipped – prevent future retries
          await invoke('mark_setup_skipped')
          break
        }
        try {
          setupError.value = null
          await invoke('auto_setup')
          break
        } catch (e: any) {
          setupError.value = String(e)
        }
      }
    }
  } catch {
    // IPC not available (e.g. browser dev), ignore
  }
})

// Expose helpers to DevTools console for testing
;(window as any).__rollbackSetup = () => invoke('rollback_setup')
;(window as any).__isAutoSetupDone = () => invoke('is_auto_setup_done')
;(window as any).__autoSetup = () => invoke('auto_setup')
</script>

<template>
  <LogView v-if="isLogView" />
  <VersionListPanel v-else />
</template>
