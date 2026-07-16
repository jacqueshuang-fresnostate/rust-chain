import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useUserStore } from '@/stores/user'
import { readAuthToken } from '@/utils/authStorage'

export { readAuthToken }

export function useAuthRequired() {
  const route = useRoute()
  const router = useRouter()
  const userStore = useUserStore()

  const isLoggedIn = computed(() => Boolean(readAuthToken()))

  function goToLogin() {
    router.push({
      name: 'Login',
      query: {
        redirect: route.fullPath,
      },
    })
  }

  return {
    isLoggedIn,
    goToLogin,
    userStore,
  }
}
