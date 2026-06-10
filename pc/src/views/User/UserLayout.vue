<template>
  <div class="max-w-7xl mx-auto p-4 md:p-8 flex flex-col md:flex-row gap-8">
    <!-- Sidebar -->
    <div class="w-full md:w-64 flex flex-col gap-2">
      <h2 class="text-xl font-bold mb-4 px-4 flex items-center gap-2">
        <Icon icon="mdi:account-circle" class="text-primary" />
        User Center
      </h2>

      <router-link to="/user/assets" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:wallet-outline" class="w-5 h-5" />
        {{ $t('nav.assets') }}
      </router-link>

      <router-link to="/user/kyc" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:card-account-details-outline" class="w-5 h-5" />
        {{ $t('nav.kyc') }}
      </router-link>

      <router-link to="/user/security" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:shield-lock-outline" class="w-5 h-5" />
        {{ $t('nav.security') }}
      </router-link>


      <router-link to="/user/transaction" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:history" class="w-5 h-5" />
        {{ $t('nav.transaction') }}
      </router-link>


      <router-link to="/user/finance-orders" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:robot-outline" class="w-5 h-5" />
        {{ $t('nav.ai_finance') }}
      </router-link>

      <router-link to="/user/invite" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:account-multiple-plus-outline" class="w-5 h-5" />
        {{ $t('invite.title') }}
      </router-link>

      <router-link to="/user/api" class="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-muted transition-colors" active-class="bg-primary/10 text-primary font-bold">
        <Icon icon="mdi:api" class="w-5 h-5" />
        {{ $t('nav.api') }}
      </router-link>
    </div>

    <!-- Content Area -->
    <div class="flex-1 bg-card border border-border rounded-xl p-6 min-h-[500px]">
      <router-view></router-view>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Icon } from '@iconify/vue'
</script>
