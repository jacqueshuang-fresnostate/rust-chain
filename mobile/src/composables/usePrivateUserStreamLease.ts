import {
  computed,
  onBeforeUnmount,
  onMounted,
  shallowRef,
  watch,
  type ComputedRef,
  type ShallowRef,
} from 'vue'
import { privateUserStreamManager } from '@/api/privateUserStreamManager'
import type { PrivateUserEvent } from '@/api/privateUserStream'
import type {
  PrivateUserStreamManagerSnapshot,
  PrivateUserTopic,
  PrivateUserTopicLease,
} from '@/core/privateUserStreamManager'

export interface UsePrivateUserStreamLeaseOptions {
  readonly topic: PrivateUserTopic
  readonly consumerId: string
  readonly enabled: () => boolean
  readonly onOpen?: () => void
  readonly onEvent: (event: PrivateUserEvent) => void
}

export interface PrivateUserStreamLeaseDiagnostics {
  readonly snapshot: ShallowRef<PrivateUserStreamManagerSnapshot>
  readonly connecting: ComputedRef<boolean>
  readonly live: ComputedRef<boolean>
  readonly stale: ComputedRef<boolean>
  readonly offline: ComputedRef<boolean>
  readonly lastFrameAt: ComputedRef<number>
}

/**
 * Binds one component to one manager topic lease. The process-wide connection,
 * session generation, reconnect, and diagnostics remain manager-owned.
 */
export function usePrivateUserStreamLease(
  options: UsePrivateUserStreamLeaseOptions,
): PrivateUserStreamLeaseDiagnostics {
  const snapshot = shallowRef(privateUserStreamManager.snapshot())
  let mounted = false
  let lease: PrivateUserTopicLease | null = null

  const unsubscribe = privateUserStreamManager.subscribe((next) => {
    snapshot.value = next
  })

  const syncLease = (): void => {
    const shouldLease = mounted && Boolean(options.enabled())
    if (shouldLease && !lease) {
      lease = privateUserStreamManager.acquire({
        topic: options.topic,
        consumerId: options.consumerId,
        onOpen: options.onOpen,
        onEvent: options.onEvent,
      })
      return
    }
    if (!shouldLease && lease) {
      lease.release()
      lease = null
    }
  }

  watch(options.enabled, syncLease, { flush: 'sync' })
  onMounted(() => {
    mounted = true
    syncLease()
  })
  onBeforeUnmount(() => {
    mounted = false
    lease?.release()
    lease = null
    unsubscribe()
  })

  return {
    snapshot,
    connecting: computed(() => snapshot.value.connecting),
    live: computed(() => snapshot.value.live),
    stale: computed(() => snapshot.value.stale),
    offline: computed(() => snapshot.value.offline),
    lastFrameAt: computed(() => snapshot.value.lastFrameAt),
  }
}
