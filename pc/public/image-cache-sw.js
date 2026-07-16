const IMAGE_CACHE_NAME = 'pc-image-cache-v1'
const IMAGE_CACHE_PREFIX = 'pc-image-cache-'
const MAX_IMAGE_CACHE_ENTRIES = 300
const IMAGE_EXTENSION_PATTERN = /\.(?:png|jpe?g|gif|webp|avif|svg|ico|bmp)(?:[?#].*)?$/i

const isHttpUrl = (url) => url.protocol === 'http:' || url.protocol === 'https:'

const isImageRequest = (request) => {
  if (request.method !== 'GET') return false
  if (request.headers.has('range')) return false

  const url = new URL(request.url)
  if (!isHttpUrl(url)) return false

  return request.destination === 'image' || IMAGE_EXTENSION_PATTERN.test(url.pathname)
}

const isCacheableImageResponse = (response) => {
  return Boolean(response && (response.ok || response.type === 'opaque'))
}

const trimImageCache = async (cache) => {
  const keys = await cache.keys()
  if (keys.length <= MAX_IMAGE_CACHE_ENTRIES) return

  const overflow = keys.slice(0, keys.length - MAX_IMAGE_CACHE_ENTRIES)
  await Promise.all(overflow.map((request) => cache.delete(request)))
}

const fetchAndCacheImage = async (request, cache) => {
  const response = await fetch(request)

  if (isCacheableImageResponse(response)) {
    await cache.put(request, response.clone())
    await trimImageCache(cache)
  }

  return response
}

const handleImageRequest = async (event) => {
  let cache
  try {
    cache = await caches.open(IMAGE_CACHE_NAME)
  } catch {
    return fetch(event.request)
  }

  const cachedResponse = await cache.match(event.request).catch(() => undefined)
  const networkResponse = fetchAndCacheImage(event.request, cache)

  if (cachedResponse) {
    event.waitUntil(networkResponse.catch(() => undefined))
    return cachedResponse
  }

  return networkResponse
}

self.addEventListener('install', () => {
  self.skipWaiting()
})

self.addEventListener('activate', (event) => {
  event.waitUntil((async () => {
    const cacheNames = await caches.keys()
    await Promise.all(
      cacheNames
        .filter((cacheName) => cacheName.startsWith(IMAGE_CACHE_PREFIX) && cacheName !== IMAGE_CACHE_NAME)
        .map((cacheName) => caches.delete(cacheName))
    )
    await self.clients.claim()
  })())
})

self.addEventListener('fetch', (event) => {
  if (!isImageRequest(event.request)) return

  event.respondWith(handleImageRequest(event))
})
