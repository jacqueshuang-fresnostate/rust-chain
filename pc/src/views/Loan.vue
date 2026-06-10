<template>
  <div class="min-h-full flex items-center justify-center bg-background p-6">
    <div class="max-w-xl w-full bg-card border border-border rounded-2xl p-8 text-center space-y-4">
      <Icon icon="mdi:bank-off-outline" class="w-16 h-16 text-muted-foreground mx-auto" />
      <h1 class="text-2xl font-bold">Crypto Loan Unavailable</h1>
      <p class="text-muted-foreground">
        The backend has not opened crypto loan products yet. This page no longer submits applications or displays placeholder products.
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Icon } from '@iconify/vue'
</script>
