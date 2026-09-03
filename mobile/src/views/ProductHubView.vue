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
    class="page page--plain pencil-page product-hub"
    data-pencil-source="Z0B0N6 zMsKE"
    data-product-workspace="live"
  >
    <PageHeader :back="true" :pencil="true" :title="t('products.title')">
      <template #actions>
        <button class="icon-button" type="button" :aria-label="t('products.hubHelp')" @click="openProductHelp">
          <Ellipsis :size="18" aria-hidden="true" />
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
  font-family: var(--font-geist-sans), "PingFang SC", "Microsoft YaHei", sans-serif;
  gap: 0;
  min-width: 0;
}

.product-hub__body {
  display: grid;
  gap: 18px;
  grid-template-rows: 128px 48px;
  min-width: 0;
  padding: 8px 20px calc(20px + env(safe-area-inset-bottom));
}

.product-hub__list {
  display: grid;
  gap: 0;
  grid-template-rows: repeat(2, 64px);
  height: 128px;
  min-width: 0;
}

.product-hub__row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 0;
  box-shadow: none;
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

.product-hub__row .product-card__icon {
  background: var(--surface-elevated);
  border: 1px solid var(--line);
  border-radius: 22px;
  box-shadow: none;
  color: var(--text);
  display: grid;
  flex: 0 0 44px;
  height: 44px;
  place-items: center;
  width: 44px;
}

.product-hub__copy {
  display: grid;
  flex: 1 1 auto;
  gap: 3px;
  height: 41px;
  min-width: 0;
}

.product-hub__copy strong {
  color: var(--text);
  font-size: 15px;
  font-weight: 700;
  line-height: 22px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.product-hub__copy small {
  color: var(--product-hub-muted);
  font-size: 11px;
  font-weight: 450;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.product-hub__row > svg {
  color: var(--product-hub-muted);
  flex: 0 0 auto;
}

.product-hub__help {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 0;
  box-shadow: none;
  color: var(--text);
  display: flex;
  font-size: 13px;
  font-weight: 650;
  gap: 10px;
  height: 48px;
  justify-content: flex-start;
  line-height: 19px;
  min-height: 48px;
  min-width: 0;
  padding: 0;
  text-align: left;
  width: 100%;
}

.product-hub__help > svg:first-child {
  color: var(--text);
  flex: 0 0 auto;
}

.product-hub__help > svg:last-child {
  color: var(--product-hub-muted);
  flex: 0 0 auto;
  margin-left: auto;
}

.product-hub__help span {
  flex: 0 1 auto;
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

@media (prefers-reduced-motion: reduce) {
  .product-hub__row,
  .product-hub__help {
    transition: none;
  }
}
</style>
