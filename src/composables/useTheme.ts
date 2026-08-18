import { ref, watch } from 'vue'

const STORAGE_KEY = 'nodepilot-theme'

function getInitialTheme(): boolean {
  const saved = localStorage.getItem(STORAGE_KEY)
  if (saved !== null) return saved === 'dark'
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

const isDark = ref(getInitialTheme())

function applyTheme(dark: boolean) {
  document.documentElement.classList.toggle('dark', dark)
  // Sync tdesign-vue-next theme-mode so component library styles follow the
  // user's choice instead of its built-in prefers-color-scheme media query.
  document.documentElement.setAttribute('theme-mode', dark ? 'dark' : 'light')
  localStorage.setItem(STORAGE_KEY, dark ? 'dark' : 'light')
}

// Apply on first load
applyTheme(isDark.value)

export function useTheme() {
  function toggleTheme() {
    isDark.value = !isDark.value
  }

  watch(isDark, applyTheme)

  return { isDark, toggleTheme }
}
