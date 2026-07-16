<script setup lang="ts">
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowRight, Banknote, CircleDollarSign, Gauge, Landmark, Rocket } from 'lucide-vue-next'
import PageHeader from '@/components/PageHeader.vue'

const router = useRouter()
const { t } = useI18n()
</script>

<template>
  <main class="page page--plain product-hub">
    <PageHeader :title="t('products.title')" />
    <div class="page-content">
      <section class="product-hub__intro"><h1>{{ t('products.introTitle') }}</h1><p>{{ t('products.introDescription') }}</p></section>
      <div class="product-hub__list">
        <button type="button" @click="router.push({ name: 'earn' })"><span class="product-hub__icon product-hub__icon--earn"><Landmark :size="25" /></span><div><strong>{{ t('products.earn') }}</strong><small>{{ t('products.earnDescription') }}</small></div><ArrowRight :size="20" /></button>
        <button type="button" @click="router.push({ name: 'loan' })"><span class="product-hub__icon product-hub__icon--loan"><Banknote :size="25" /></span><div><strong>{{ t('products.loan') }}</strong><small>{{ t('products.loanDescription') }}</small></div><ArrowRight :size="20" /></button>
        <button type="button" @click="router.push({ name: 'new-coins' })"><span class="product-hub__icon product-hub__icon--new-coin"><Rocket :size="25" /></span><div><strong>{{ t('products.newCoins') }}</strong><small>{{ t('products.newCoinsDescription') }}</small></div><ArrowRight :size="20" /></button>
        <button type="button" @click="router.push({ name: 'prediction' })"><span class="product-hub__icon product-hub__icon--prediction"><CircleDollarSign :size="25" /></span><div><strong>{{ t('products.prediction') }}</strong><small>{{ t('products.predictionDescription') }}</small></div><ArrowRight :size="20" /></button>
        <button type="button" @click="router.push({ name: 'seconds' })"><span class="product-hub__icon product-hub__icon--seconds"><Gauge :size="25" /></span><div><strong>{{ t('products.seconds') }}</strong><small>{{ t('products.secondsDescription') }}</small></div><ArrowRight :size="20" /></button>
      </div>
    </div>
  </main>
</template>

<style scoped>
.product-hub { background: var(--background); }.product-hub .page-content { background: var(--surface); min-height: calc(100dvh - 56px); padding-bottom: 42px; }.product-hub__intro { padding: 25px 0 28px; }.product-hub__intro h1 { font-size: 26px; margin: 0; }.product-hub__intro p { color: var(--muted); font-size: 14px; margin: 8px 0 0; }.product-hub__list { display: grid; gap: 12px; }.product-hub__list button { align-items: center; background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow-soft); display: grid; gap: 13px; grid-template-columns: 50px 1fr auto; min-height: 91px; padding: 14px; text-align: left; width: 100%; }.product-hub__icon { align-items: center; border-radius: var(--radius); display: inline-flex; height: 50px; justify-content: center; width: 50px; }.product-hub__icon--earn { background: var(--positive-soft); color: var(--positive); }.product-hub__icon--loan { background: #fff0dc; color: #b96a12; }.product-hub__icon--new-coin { background: #eef5ff; color: #3975ca; }.product-hub__icon--prediction { background: #f7edff; color: #9a4ec2; }.product-hub__icon--seconds { background: #fff0dc; color: #b96a12; }.product-hub__list div { display: grid; gap: 5px; }.product-hub__list strong { font-size: 17px; }.product-hub__list small { color: var(--muted); font-size: 12px; }.product-hub__list > svg { color: var(--muted); }
</style>
