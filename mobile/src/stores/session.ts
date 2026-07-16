import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { clearAuthTokens, readAccessToken } from '@/api/client'

export const useSessionStore = defineStore('mobile-session', () => {
  const token = ref(readAccessToken())
  const isAuthenticated = computed(() => Boolean(token.value))

  function sync(): void {
    token.value = readAccessToken()
  }

  function logout(): void {
    clearAuthTokens()
    token.value = ''
  }

  return { token, isAuthenticated, sync, logout }
})
