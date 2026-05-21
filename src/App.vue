<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import VersionListPanel from './panels/VersionListPanel.vue'
import SetupGuide from './panels/SetupGuide.vue'

const showSetup = ref(true)
const loading = ref(true)

onMounted(async () => {
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
  <SetupGuide v-if="showSetup" @done="showSetup = false" />
  <VersionListPanel v-else />
</template>
