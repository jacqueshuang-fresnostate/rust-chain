<script setup lang="ts">
import { computed, type Component } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowRight, Banknote, CircleDollarSign, Gauge, Landmark, Rocket } from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'

type ProductRouteName = 'earn' | 'loan' | 'new-coins' | 'prediction' | 'seconds'
type ProductTier = 'featured' | 'secondary'

interface ProductEntry {
  name: ProductRouteName
  tier: ProductTier
  label: string
  description: string
  icon: Component
}

const router = useRouter()
const { t } = useI18n()

const products = computed<ProductEntry[]>(() => [
  {
    name: 'earn',
    tier: 'featured',
    label: t('products.earn'),
    description: t('products.earnDescription'),
    icon: Landmark,
  },
  {
    name: 'loan',
    tier: 'featured',
    label: t('products.loan'),
    description: t('products.loanDescription'),
    icon: Banknote,
  },
  {
    name: 'new-coins',
    tier: 'secondary',
    label: t('products.newCoins'),
    description: t('products.newCoinsDescription'),
    icon: Rocket,
  },
  {
    name: 'prediction',
    tier: 'secondary',
    label: t('products.prediction'),
    description: t('products.predictionDescription'),
    icon: CircleDollarSign,
  },
  {
    name: 'seconds',
    tier: 'secondary',
    label: t('products.seconds'),
    description: t('products.secondsDescription'),
    icon: Gauge,
  },
])
const featuredProducts = computed(() => products.value.filter((product) => product.tier === 'featured'))
const secondaryProducts = computed(() => products.value.filter((product) => product.tier === 'secondary'))

function openProduct(name: ProductRouteName): void {
  void router.push({ name })
}
</script>

<template>
  <main class="page page--plain page--prototype-grid product-hub" data-product-workspace="live">
    <PageHeader :title="t('products.title')" :subtitle="t('products.introDescription')" />
    <div class="page-content">
      <header class="product-hub__overview">
        <span>{{ t('products.title') }}</span>
        <h1>{{ t('products.introTitle') }}</h1>
        <p>{{ t('products.introDescription') }}</p>
      </header>

      <section class="product-hub__group" :aria-labelledby="'featured-products-title'">
        <header>
          <span>01</span>
          <h2 id="featured-products-title">{{ t('products.featuredServices') }}</h2>
        </header>
        <div class="product-hub__matrix product-hub__matrix--featured">
          <button
            v-for="product in featuredProducts"
            :key="product.name"
            class="product-card product-card--featured"
            :data-product="product.name"
            :data-product-tier="product.tier"
            type="button"
            :aria-label="product.label"
            @click="openProduct(product.name)"
          >
            <span class="product-card__top">
              <span class="product-card__icon"><component :is="product.icon" :size="20" /></span>
              <ArrowRight :size="17" />
            </span>
            <strong>{{ product.label }}</strong>
            <small>{{ product.description }}</small>
          </button>
        </div>
      </section>

      <section class="product-hub__group" :aria-labelledby="'specialized-products-title'">
        <header>
          <span>02</span>
          <h2 id="specialized-products-title">{{ t('products.specializedServices') }}</h2>
        </header>
        <div class="product-hub__matrix product-hub__matrix--secondary">
          <button
            v-for="product in secondaryProducts"
            :key="product.name"
            class="product-card product-card--secondary"
            :data-product="product.name"
            :data-product-tier="product.tier"
            type="button"
            :aria-label="product.label"
            @click="openProduct(product.name)"
          >
            <span class="product-card__top">
              <span class="product-card__icon"><component :is="product.icon" :size="20" /></span>
              <ArrowRight :size="17" />
            </span>
            <strong>{{ product.label }}</strong>
            <small>{{ product.description }}</small>
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.product-hub {
  background-color: var(--background);
}

.product-hub .page-content {
  background: var(--surface);
  min-height: calc(100dvh - 72px);
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 14px;
}

.product-hub__overview {
  background:
    linear-gradient(var(--grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid-line) 1px, transparent 1px),
    var(--surface);
  background-size: 36px 36px;
  border-bottom: 1px solid var(--line);
  border-top: 3px solid var(--signal-green);
  display: grid;
  gap: 6px;
  margin: 0 -16px;
  min-height: 132px;
  padding: 20px;
  position: relative;
}

.product-hub__overview::after {
  background: linear-gradient(90deg, var(--signal-green) 0 34%, var(--signal-coral) 34% 67%, var(--signal-blue) 67%);
  bottom: 0;
  content: '';
  height: 4px;
  left: 20px;
  position: absolute;
  width: 96px;
}

.product-hub__overview > span {
  color: var(--positive);
  font-family: var(--data-font);
  font-size: 10px;
  font-weight: 760;
  letter-spacing: 0;
  text-transform: uppercase;
}

.product-hub__overview h1 {
  color: var(--ink);
  font-size: 28px;
  letter-spacing: 0;
  line-height: 1.08;
  margin: 0;
}

.product-hub__overview p {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.45;
  margin: 0;
}

.product-hub__group {
  margin-top: 18px;
}

.product-hub__group > header {
  align-items: center;
  border-bottom: 1px solid var(--line);
  display: grid;
  gap: 10px;
  grid-template-columns: 30px minmax(0, 1fr);
  min-height: 44px;
}

.product-hub__group > header span {
  color: var(--accent);
  font-family: var(--data-font);
  font-size: 10px;
  font-weight: 800;
}

.product-hub__group > header h2 {
  font-size: 15px;
  margin: 0;
}

.product-hub__matrix {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  margin-top: 8px;
  min-width: 0;
}

.product-card {
  align-content: start;
  background: var(--surface-elevated, var(--surface));
  border: 1px solid var(--line);
  border-top: 2px solid var(--line-strong, var(--line));
  color: var(--ink);
  display: grid;
  gap: 9px;
  grid-column: span 2;
  min-height: 150px;
  min-width: 0;
  overflow: hidden;
  padding: 12px 10px;
  text-align: left;
  width: 100%;
}

.product-card--featured {
  background: color-mix(in srgb, var(--signal-green) 7%, var(--surface-elevated, var(--surface)));
  border-top: 3px solid var(--signal-green);
  grid-column: span 3;
  min-height: 170px;
  padding: 15px 13px;
}

.product-card[data-product="new-coins"] {
  border-top-color: var(--positive);
}

.product-card[data-product="prediction"] {
  border-top-color: var(--signal-blue);
}

.product-card[data-product="seconds"] {
  border-top-color: var(--signal-coral);
}

.product-card__top {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.product-card__top > svg {
  color: var(--muted);
}

.product-card__icon {
  align-items: center;
  background: var(--soft);
  border: 1px solid var(--line);
  color: var(--accent);
  display: flex;
  height: 40px;
  justify-content: center;
  width: 40px;
}

.product-card--featured .product-card__icon {
  background: var(--ink);
  border-color: var(--ink);
  color: var(--surface);
  height: 44px;
  width: 44px;
}

.product-card strong {
  font-size: 15px;
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.product-card small {
  color: var(--muted);
  display: -webkit-box;
  font-size: 10px;
  line-height: 1.45;
  overflow: hidden;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}

.product-card--featured strong {
  font-size: 18px;
}

.product-card--featured small {
  font-size: 11px;
  -webkit-line-clamp: 2;
}

@media (max-width: 360px) {
  .product-hub__overview {
    margin-left: -12px;
    margin-right: -12px;
    padding-left: 12px;
    padding-right: 12px;
  }
}

@media (max-width: 340px) {
  .product-hub .page-content {
    padding-left: 12px;
    padding-right: 12px;
  }

  .product-hub__overview {
    margin-left: -12px;
    margin-right: -12px;
    padding-left: 12px;
    padding-right: 12px;
  }

  .product-hub__matrix {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .product-card,
  .product-card--featured {
    grid-column: span 1;
    min-height: 154px;
  }

  .product-card--secondary:last-child {
    grid-column: 1 / -1;
    min-height: 122px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .product-card {
    transition: none;
  }
}
</style>
