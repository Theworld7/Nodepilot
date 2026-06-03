<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import CodeBlock from '../components/CodeBlock.vue'

const emit = defineEmits<{ done: [] }>()

const visible = ref(false)
const shell = ref<string>('zsh')
const pathCommand = ref('')

function detectShell() {
  shell.value = navigator.userAgent.includes('Windows') ? 'powershell' : 'zsh'
  pathCommand.value = shell.value === 'powershell'
    ? '$env:Path = "$env:USERPROFILE\\.nodepilot\\current\\bin;" + $env:Path'
    : 'export PATH="$HOME/.nodepilot/current/bin:$PATH"'
}

async function markDone() {
  try {
    await invoke('mark_setup_done')
  } catch (e) {
    console.error('mark_setup_done failed:', e)
  }
  emit('done')
}

onMounted(async () => {
  detectShell()
  try {
    const firstRun = await invoke<boolean>('is_first_run')
    visible.value = firstRun
  } catch {
    visible.value = true
  }
})
</script>

<template>
  <div v-if="visible" class="setup-overlay">
    <div class="setup-card">
      <h2 class="setup-title">欢迎使用 nodepilot</h2>
      <p class="setup-desc">
        请将以下路径添加到你的 shell 配置文件中，以便在终端中使用 nodepilot 管理的 Node.js 版本：
      </p>

      <div class="setup-shell-hint">
        检测到 shell: <strong>{{ shell }}</strong>
      </div>

      <CodeBlock :code="pathCommand" />

      <div v-if="shell === 'powershell'" class="setup-instructions">
        <p>1. 以管理员身份打开 PowerShell</p>
        <p>2. 运行：</p>
        <CodeBlock code="notepad $PROFILE" />
        <p>3. 将上面的命令粘贴到文件末尾并保存</p>
      </div>
      <div v-else class="setup-instructions">
        <p>1. 打开终端，运行：</p>
        <CodeBlock :code="`nano ~/.${shell}rc`" />
        <p>2. 将上面的命令粘贴到文件末尾</p>
        <p>3. 保存后运行：</p>
        <CodeBlock :code="`source ~/.${shell}rc`" />
      </div>

      <t-button
        theme="primary"
        size="small"
        block
        @click="markDone"
      >
        我已设置完成
      </t-button>
    </div>
  </div>
</template>

<style scoped>
.setup-overlay {
  position: fixed;
  inset: 0;
  background: var(--bg);
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow-y: auto;
}

.setup-card {
  width: 100%;
  max-width: 343px;
  padding: 40px 16px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.setup-title {
  font-size: 20px;
  font-weight: 600;
  text-align: center;
  color: var(--text-h);
  margin: 0;
}

.setup-desc {
  font-size: 14px;
  line-height: 1.6;
  color: var(--text);
  margin: 0;
}

.setup-shell-hint {
  font-size: 12px;
  color: var(--text);
}

.setup-instructions {
  font-size: 13px;
  line-height: 1.8;
  color: var(--text);
}

.setup-instructions p {
  margin: 4px 0;
}

.setup-instructions code {
  display: block;
  background: var(--code-bg);
  border-radius: 4px;
  padding: 8px 12px;
  font-size: 12px;
  margin: 4px 0 12px;
}
</style>
