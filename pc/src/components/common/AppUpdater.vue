<script setup lang="ts">
import { onMounted } from 'vue'
import { check } from '@tauri-apps/plugin-updater'
import { ask } from '@tauri-apps/plugin-dialog'
import { relaunch } from '@tauri-apps/plugin-process'

/**
 * Check for updates
 */
async function checkForUpdates() {
  try {
    const update = await check()

    if (update) {
      console.log(`[Updater] Found update: ${update.version}`)

      const yes = await ask(
        `Update to ${update.version} is available!\n\nRelease notes: ${update.body}`,
        {
          title: 'Update Available',
          kind: 'info',
          okLabel: 'Update',
          cancelLabel: 'Later'
        }
      )

      if (yes) {
        await update.downloadAndInstall((event) => {
          switch (event.event) {
            case 'Started':
              console.log(`[Updater] Started downloading ${update.version}`)
              break
            case 'Progress':
              console.log(`[Updater] Downloaded ${event.data.chunkLength} bytes`)
              break
            case 'Finished':
              console.log(`[Updater] Download finished`)
              break
          }
        })

        await relaunch()
      }
    } else {
      console.log('[Updater] No updates available')
    }
  } catch (error) {
    console.error('[Updater] Failed to check for updates:', error)
  }
}

onMounted(() => {
  // Check for updates on launch
  // Only check in production environment to avoid errors during dev
  if (import.meta.env.PROD || (window as any).__TAURI_INTERNALS__) {
     checkForUpdates()
  }
})
</script>

<template>
  <div class="fixed top-0 right-0 p-4 z-50 pointer-events-none">
    <!-- Updater UI can be expanded here if needed, currently using native dialogs -->
  </div>
</template>
