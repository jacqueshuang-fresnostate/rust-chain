import { fileURLToPath, URL } from 'node:url'
import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { VitePWA } from 'vite-plugin-pwa'

function normalizePublicBase(value: string): string {
  const path = value.trim().replace(/^\/+|\/+$/g, '')
  return path ? `/${path}/` : '/'
}

function isolateTauriIndexHtml(isTauriBuild: boolean) {
  return {
    name: 'hippo-tauri-index-isolation',
    transformIndexHtml(html: string): string {
      if (isTauriBuild) {
        return html.replace(/\s*<(?:link|meta)\b[^>]*\bdata-pwa-only\b[^>]*>\s*/g, '\n')
      }
      return html.replace(/\sdata-pwa-only\b/g, '')
    },
  }
}

export default defineConfig(({ mode }) => {
  const environment = loadEnv(mode, process.cwd(), '')
  const devHost = environment.TAURI_DEV_HOST || process.env.TAURI_DEV_HOST || '0.0.0.0'
  const apiPrefix = environment.VITE_BACKEND_API_PREFIX || '/api/v1'
  const backendTarget = environment.VITE_BACKEND_API_DOMAIN || 'http://127.0.0.1:8080'
  const appBase = normalizePublicBase(environment.VITE_PWA_BASE || '/')
  const isTauriBuild = Boolean(environment.TAURI_ENV_PLATFORM || process.env.TAURI_ENV_PLATFORM) || mode === 'tauri'
  const pwaEnabled = mode === 'pwa' && !isTauriBuild
  const buildBase = isTauriBuild ? '/' : appBase
  const withBase = (path: string) => `${appBase}${path.replace(/^\/+/, '')}`

  return {
    base: buildBase,
    plugins: [
      vue(),
      isolateTauriIndexHtml(isTauriBuild),
      VitePWA({
        disable: !pwaEnabled,
        strategies: 'generateSW',
        registerType: 'prompt',
        injectRegister: null,
        scope: appBase,
        includeAssets: [
          'pwa/icon-192.png',
          'pwa/icon-512.png',
          'pwa/icon-maskable-512.png',
          'pwa/apple-touch-icon.png',
        ],
        manifest: {
          id: appBase,
          name: 'Hippo Mobile',
          short_name: 'Hippo',
          lang: 'zh-CN',
          dir: 'ltr',
          start_url: appBase,
          scope: appBase,
          display: 'standalone',
          orientation: 'portrait-primary',
          theme_color: '#f7f8fa',
          background_color: '#f1f4f8',
          categories: ['finance'],
          icons: [
            {
              src: withBase('pwa/icon-192.png'),
              sizes: '192x192',
              type: 'image/png',
              purpose: 'any',
            },
            {
              src: withBase('pwa/icon-512.png'),
              sizes: '512x512',
              type: 'image/png',
              purpose: 'any',
            },
            {
              src: withBase('pwa/icon-maskable-512.png'),
              sizes: '512x512',
              type: 'image/png',
              purpose: 'maskable',
            },
          ],
        },
        workbox: {
          cacheId: 'hippo-mobile-shell',
          globPatterns: ['**/*.{js,css,html,png,svg,ico,woff,woff2,webmanifest}'],
          globIgnores: ['pwa/*.png', 'manifest.webmanifest'],
          navigateFallback: withBase('index.html'),
          navigateFallbackDenylist: [
            /\/api(?:\/|$)/,
            /\/ws(?:\/|$)/,
            /\/health(?:\/|$)/,
            /\/downloads?(?:\/|$)/,
          ],
          runtimeCaching: [],
          cleanupOutdatedCaches: true,
          clientsClaim: false,
          skipWaiting: false,
        },
        devOptions: {
          enabled: false,
        },
      }),
    ],
    publicDir: isTauriBuild ? false : 'public',
    define: {
      __PWA_ENABLED__: JSON.stringify(pwaEnabled),
    },
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
  }
})
