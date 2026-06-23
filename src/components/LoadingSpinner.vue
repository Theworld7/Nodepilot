<template>
  <span class="loading-spinner" :style="{ fontSize: size }">
    <LoadingIcon />
  </span>
</template>

<script setup lang="ts">
defineProps<{
  size?: string
}>()
</script>

<style scoped>
/* wrapper 为 HTML 元素，transform-origin: center 在所有浏览器中行为一致
   inline-flex + 对齐居中确保 SVG 与容器无偏移，避免公转 */
.loading-spinner {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  animation: spin 1s linear infinite;
  transform-origin: center;
  will-change: transform;
}

/* 禁用 LoadingIcon 自带的 t-spin 动画，避免与 wrapper 动画叠加 */
.loading-spinner :deep(.t-icon-loading) {
  animation: none;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
