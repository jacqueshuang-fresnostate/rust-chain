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

function openProduct(name: ProductRouteName): void {
  void router.push({ name })
}
</script>

<template>
  <main class="page page--plain product-hub">
    <PageHeader :title="t('products.title')" />
    <div class="page-content">
      <header class="product-hub__overview">
        <span>{{ t('products.title') }}</span>
        <h1>{{ t('products.introTitle') }}</h1>
        <p>{{ t('products.introDescription') }}</p>
      </header>

      <section class="product-hub__matrix" :aria-label="t('products.title')">
        <button
          v-for="product in products"
          :key="product.name"
          class="product-card"
          :class="`product-card--${product.tier}`"
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
      </section>
    </div>
  </main>
</template>

<style scoped>
.product-hub {
  background: var(--background);
}

.product-hub .page-content {
  background: var(--surface);
  min-height: calc(100dvh - 56px);
  padding-bottom: calc(36px + env(safe-area-inset-bottom));
  padding-top: 14px;
}

.product-hub__overview {
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  border-top: 3px solid var(--accent);
  display: grid;
  gap: 6px;
  margin: 0 -20px;
  min-height: 132px;
  padding: 20px;
}

.product-hub__overview > span {
  color: var(--accent);
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

.product-hub__matrix {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  margin-top: 14px;
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
  background: color-mix(in srgb, var(--accent) 7%, var(--surface-elevated, var(--surface)));
  border-top: 3px solid var(--accent);
  grid-column: span 3;
  min-height: 170px;
  padding: 15px 13px;
}

.product-card[data-product="new-coins"] {
  border-top-color: var(--positive);
}

.product-card[data-product="prediction"] {
  border-top-color: var(--focus, var(--accent));
}

.product-card[data-product="seconds"] {
  border-top-color: var(--accent);
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
    margin-left: -16px;
    margin-right: -16px;
    padding-left: 16px;
    padding-right: 16px;
  }
}

@media (max-width: 340px) {
  .product-hub .page-content {
    padding-left: 14px;
    padding-right: 14px;
  }

  .product-hub__overview {
    margin-left: -14px;
    margin-right: -14px;
    padding-left: 14px;
    padding-right: 14px;
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
