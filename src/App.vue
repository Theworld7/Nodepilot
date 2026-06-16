<script setup lang="ts">
import { ref } from 'vue'
import LogView from './panels/LogView.vue'
import VersionListPanel from './panels/VersionListPanel.vue'

const params = new URLSearchParams(window.location.search)
const isLogView = ref(params.get('view') === 'log')
</script>

<template>
  <LogView v-if="isLogView" />
  <VersionListPanel v-else />
</template>
