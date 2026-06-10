import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { getWallets } from '@/api/asset'
import { getSecuritySetting } from '@/api/user'

export interface UserAssets {
  [symbol: string]: number
}

interface AuthSession {
  token: string
  refreshToken?: string
  user?: any
}

export const useUserStore = defineStore('user', () => {
  const token = ref<string | null>(null)
  const user = ref<any>(null)
  const assets = ref<UserAssets>({
    USDT: 0,
    BTC: 0,
    ETH: 0
  })

  const isLoggedIn = computed(() => !!token.value)

  function setToken(newToken: string) {
    token.value = newToken
    localStorage.setItem('token', newToken)
  }

  function setUser(userData: any) {
    user.value = userData
  }

  function setAuthSession(session: AuthSession) {
    setToken(session.token)
    if (session.refreshToken) {
      localStorage.setItem('refresh_token', session.refreshToken)
    }
    if (session.user) {
      setUser(session.user)
    }
  }

  async function loadProfile() {
    const res = await getSecuritySetting()
    if (res.code === 0 || res.code === 200) {
      user.value = res.data
    }
    return res
  }

  async function loadWalletAccounts() {
    const res = await getWallets()
    if (res.data.code === 0) {
      assets.value = res.data.data.reduce<UserAssets>((nextAssets, wallet) => {
        nextAssets[wallet.coin.coinGroup] = wallet.balance
        return nextAssets
      }, {})
    }
    return res
  }

  function login(session?: AuthSession) {
    if (session) {
      setAuthSession(session)
    }
  }

  function logout() {
    token.value = null
    user.value = null
    assets.value = {}
    localStorage.removeItem('token')
    localStorage.removeItem('refresh_token')
    localStorage.removeItem('user')
  }

  return {
    token,
    user,
    assets,
    isLoggedIn,
    setToken,
    setUser,
    setAuthSession,
    loadProfile,
    loadWalletAccounts,
    login,
    logout
  }
}, {
  persist: true
})
