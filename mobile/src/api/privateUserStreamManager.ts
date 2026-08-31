import {
  readAuthSessionSnapshot,
  subscribeAuthSession,
} from './client'
import { createPrivateUserStream } from './privateUserStream'
import { privateUserWebSocketUrl } from '@/config/app'
import {
  createPrivateUserStreamManager,
  type PrivateUserManagerSession,
} from '@/core/privateUserStreamManager'

function currentManagerSession(): PrivateUserManagerSession {
  const snapshot = readAuthSessionSnapshot()
  return {
    accessToken: snapshot.accessToken,
    scope: snapshot.scope,
    generation: snapshot.epoch,
  }
}

/** Process-wide private stream owner. It contains no Vue/page lifecycle state. */
export const privateUserStreamManager = createPrivateUserStreamManager({
  readSession: currentManagerSession,
  isOnline: () => typeof navigator === 'undefined' || navigator.onLine !== false,
  openTransport: ({
    accessToken,
    onOpen,
    onInboundFrame,
    onStateChange,
    onEvent,
  }) => createPrivateUserStream({
    // The token is frozen for this manager generation. Refresh/logout first
    // closes this stream, then a lease may open the replacement generation.
    getAccessToken: () => accessToken,
    getUrl: privateUserWebSocketUrl,
    onOpen,
    onInboundFrame,
    onStateChange,
    onEvent,
  }),
})

subscribeAuthSession((transition) => {
  privateUserStreamManager.synchronizeSession({
    accessToken: transition.current.accessToken,
    scope: transition.current.scope,
    generation: transition.current.epoch,
  })
})

if (typeof window !== 'undefined') {
  window.addEventListener('online', () => privateUserStreamManager.setOnline(true))
  window.addEventListener('offline', () => privateUserStreamManager.setOnline(false))
}
