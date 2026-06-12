<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import LogView from './panels/LogView.vue'
import VersionListPanel from './panels/VersionListPanel.vue'
import SetupGuide from './panels/SetupGuide.vue'

const params = new URLSearchParams(window.location.search)
const isLogView = ref(params.get('view') === 'log')

const showSetup = ref(true)
const loading = ref(true)

onMounted(async () => {
  if (isLogView.value) return
  try {
    const firstRun = await invoke<boolean>('is_first_run')
    showSetup.value = firstRun
  } catch {
    showSetup.value = true
  }
  loading.value = false
})
</script>

<template>
  <LogView v-if="isLogView" />
  <SetupGuide v-else-if="showSetup" @done="showSetup = false" />
  <VersionListPanel v-else />
</template>
