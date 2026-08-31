import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import i18n from './i18n'
import router from './router'
import { isTauriRuntime } from './core/platform'
import { detectPerformanceTier } from './core/performanceTier'
import { initializePwa } from './pwa'
import './styles/base.css'
import './styles/prototype-parity.css'
import './styles/pencil-selected-pages.css'

document.documentElement.dataset.performanceTier = detectPerformanceTier(globalThis.navigator)

createApp(App).use(createPinia()).use(router).use(i18n).mount('#app')

if (__PWA_ENABLED__ && !isTauriRuntime()) {
  void initializePwa()
}
