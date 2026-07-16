import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const devHost = process.env.TAURI_DEV_HOST || '0.0.0.0'
const apiPrefix = process.env.VITE_BACKEND_API_PREFIX || '/api/v1'
const backendTarget = process.env.VITE_BACKEND_API_DOMAIN || 'http://127.0.0.1:8080'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_'],
  server: {
    host: devHost,
    port: 1611,
    strictPort: true,
    proxy: {
      [apiPrefix]: {
        target: backendTarget,
        changeOrigin: true,
      },
    },
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  preview: {
    host: '0.0.0.0',
    port: 4611,
    strictPort: true,
  },
})
