export interface ObjectUrlApi {
  createObjectURL: (value: Blob) => string
  revokeObjectURL: (url: string) => void
}

export interface ObjectUrlRegistry<Key extends string> {
  replace: (key: Key, value: Blob) => string
  clear: (key: Key) => void
  clearAll: () => void
  current: (key: Key) => string | undefined
}

/** Tracks only URLs created by this registry and revokes every replaced/current Blob URL once. */
export function createObjectUrlRegistry<Key extends string>(
  api: ObjectUrlApi = URL,
): ObjectUrlRegistry<Key> {
  const urls = new Map<Key, string>()
  const clear = (key: Key): void => {
    const current = urls.get(key)
    urls.delete(key)
    if (current?.startsWith('blob:')) api.revokeObjectURL(current)
  }

  return {
    replace(key: Key, value: Blob): string {
      const next = api.createObjectURL(value)
      const previous = urls.get(key)
      urls.set(key, next)
      if (previous && previous !== next && previous.startsWith('blob:')) {
        api.revokeObjectURL(previous)
      }
      return next
    },
    clear,
    clearAll(): void {
      for (const key of [...urls.keys()]) clear(key)
    },
    current(key: Key): string | undefined {
      return urls.get(key)
    },
  }
}
