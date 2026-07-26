import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { TDesignResolver } from '@tdesign-vue-next/auto-import-resolver'

export default defineConfig({
  plugins: [
    vue(),
    AutoImport({
      dts: 'node_modules/.tmp/auto-imports.d.ts',
      resolvers: [TDesignResolver({
        library: 'vue-next',
        resolveIcons: true,
      })],
    }),
    Components({
      dts: 'node_modules/.tmp/components.d.ts',
      resolvers: [TDesignResolver({
        library: 'vue-next',
        resolveIcons: true,
      })],
    }),
  ],
  server: {
    port: 5199,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/target/**'],
    },
  },
})
