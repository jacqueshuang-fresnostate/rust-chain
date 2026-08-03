<script setup lang="ts">
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  BookOpen,
  ChevronRight,
  Ellipsis,
  Gauge,
  Newspaper,
} from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'

const router = useRouter()
const { t } = useI18n()

function openPrediction(): void {
  void router.push({ name: 'prediction' })
}

function openNews(): void {
  void router.push({ name: 'news' })
}

function openProductHelp(): void {
  void router.push({ name: 'news', query: { category: 'product' } })
}
</script>

<template>
  <main
    class="page page--plain product-hub"
    data-pencil-source="Z0B0N6 zMsKE"
    data-product-workspace="live"
  >
    <PageHeader :back="true" :pencil="true" :title="t('products.title')" :subtitle="t('products.introDescription')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('news.product')" @click="openProductHelp">
          <Ellipsis :size="20" aria-hidden="true" />
        </button>
      </template>
    </PageHeader>

    <div class="page-content product-hub__body" data-product-count="2">
      <section class="product-hub__list" :aria-label="t('products.specializedServices')">
        <button
          class="product-card product-card--secondary product-hub__row"
          data-product="prediction"
          data-product-tier="secondary"
          type="button"
          @click="openPrediction"
        >
          <span class="product-card__icon"><Gauge :size="19" aria-hidden="true" /></span>
          <span class="product-hub__copy">
            <strong>{{ t('products.hubPrediction') }}</strong>
            <small>{{ t('products.hubPredictionDescription') }}</small>
          </span>
          <ChevronRight :size="18" aria-hidden="true" />
        </button>

        <button
          class="product-card product-card--secondary product-hub__row"
          data-product="news"
          data-product-tier="secondary"
          type="button"
          @click="openNews"
        >
          <span class="product-card__icon"><Newspaper :size="19" aria-hidden="true" /></span>
          <span class="product-hub__copy">
            <strong>{{ t('products.hubNews') }}</strong>
            <small>{{ t('products.hubNewsDescription') }}</small>
          </span>
          <ChevronRight :size="18" aria-hidden="true" />
        </button>
      </section>

      <button class="product-hub__help" type="button" @click="openProductHelp">
        <BookOpen :size="16" aria-hidden="true" />
        <span>{{ t('products.hubHelp') }}</span>
        <ChevronRight :size="16" aria-hidden="true" />
      </button>
    </div>
  </main>
</template>

<style scoped>
.product-hub {
  background: var(--page);
  color: var(--text);
  display: block;
  gap: 0;
  min-width: 0;
}

.product-hub__body {
  display: grid;
  gap: 18px;
  min-width: 0;
  padding: 8px 20px calc(20px + env(safe-area-inset-bottom));
}

.product-hub__list {
  display: grid;
  gap: 0;
  min-width: 0;
}

.product-hub__row {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--line);
  border-radius: 0;
  color: var(--text);
  display: flex;
  gap: 14px;
  height: 64px;
  min-height: 64px;
  min-width: 0;
  padding: 0;
  text-align: left;
  width: 100%;
}

.product-card__icon {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line-strong);
  border-radius: 50%;
  color: var(--positive);
  display: flex;
  flex: 0 0 44px;
  height: 44px;
  justify-content: center;
  width: 44px;
}

.product-hub__row[data-product="news"] .product-card__icon {
  color: var(--accent);
}

.product-hub__copy {
  display: grid;
  flex: 1 1 auto;
  gap: 3px;
  min-width: 0;
}

.product-hub__copy strong {
  font-size: 14px;
  line-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.product-hub__copy small {
  color: var(--muted);
  font-size: 11px;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.product-hub__row > svg,
.product-hub__help > svg:last-child {
  color: var(--muted);
  flex: 0 0 auto;
}

.product-hub__help {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 1px solid var(--line);
  border-radius: 0;
  color: var(--muted);
  display: flex;
  font-size: 12px;
  gap: 10px;
  height: 48px;
  justify-content: flex-start;
  min-height: 48px;
  min-width: 0;
  padding: 0;
  text-align: left;
  width: 100%;
}

.product-hub__help > svg:first-child {
  color: var(--positive);
  flex: 0 0 auto;
}

.product-hub__help span {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.product-hub__row:focus-visible,
.product-hub__help:focus-visible {
  box-shadow: inset 0 0 0 2px var(--focus);
  outline: 0;
}

@media (max-width: 340px) {
  .product-hub__body {
    padding-inline: 16px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .product-hub__row,
  .product-hub__help {
    transition: none;
  }
}
</style>
