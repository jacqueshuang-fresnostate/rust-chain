<template>
  <div class="min-h-full flex items-center justify-center bg-background p-6">
    <div class="max-w-xl w-full bg-card border border-border rounded-2xl p-8 text-center space-y-4">
      <Icon icon="mdi:credit-card-off-outline" class="w-16 h-16 text-muted-foreground mx-auto" />
      <h1 class="text-2xl font-bold">OTC Service Unavailable</h1>
      <p class="text-muted-foreground">
        The backend has not opened OTC trading yet. This entry is disabled until a real OTC product module is available.
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Icon } from '@iconify/vue'
</script>
