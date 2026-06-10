import { createApp } from 'vue'
import { createPinia } from 'pinia'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'
import './style.css'
import App from './App.vue'
import '@klinecharts/pro/dist/klinecharts-pro.css'
import router from './router'
import { VueQueryPlugin } from '@tanstack/vue-query'
import i18n from './i18n'
import Toast, { PluginOptions, POSITION, globalEventBus } from 'vue-toastification'
import 'vue-toastification/dist/index.css'

const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)

const app = createApp(App)

const toastOptions: PluginOptions = {
    eventBus: globalEventBus,
    position: POSITION.TOP_RIGHT,
    timeout: 3000,
    closeOnClick: true,
    pauseOnFocusLoss: true,
    pauseOnHover: true,
    draggable: true,
    draggablePercent: 0.6,
    showCloseButtonOnHover: true,
    hideProgressBar: false,
    closeButton: 'button',
    icon: true,
    rtl: false,
}

app.use(pinia)
app.use(router)
app.use(VueQueryPlugin)
app.use(i18n)
app.use(Toast, toastOptions)

app.mount('#app')
