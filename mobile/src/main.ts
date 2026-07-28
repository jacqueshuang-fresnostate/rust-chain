import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import i18n from './i18n'
import router from './router'
import { isTauriRuntime } from './core/platform'
import { initializePwa } from './pwa'
import './styles/base.css'

createApp(App).use(createPinia()).use(router).use(i18n).mount('#app')

if (__PWA_ENABLED__ && !isTauriRuntime()) {
  void initializePwa()
}
