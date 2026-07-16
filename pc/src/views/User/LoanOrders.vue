<template>
  <div class="space-y-6">
    <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <h2 class="flex items-center gap-2 text-2xl font-bold">
        <Icon icon="mdi:file-document-multiple-outline" class="h-6 w-6 text-primary" />
        {{ t('loan.my_orders') }}
      </h2>
      <button type="button" class="inline-flex items-center gap-2 rounded-lg bg-muted px-4 py-2 text-sm font-semibold hover:bg-muted/80" :disabled="loading" @click="loadOrders">
        <Icon icon="mdi:refresh" class="h-4 w-4" />
        {{ t('loan.refresh') }}
      </button>
    </div>

    <div class="rounded-xl border border-border bg-card shadow-sm">
      <div class="flex gap-2 overflow-x-auto border-b border-border px-4">
        <button
          v-for="tab in statusTabs"
          :key="tab.value || 'all'"
          type="button"
          class="border-b-2 px-3 py-4 text-sm font-semibold transition-colors whitespace-nowrap"
          :class="activeStatus === tab.value ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
          @click="setStatus(tab.value)"
        >
          {{ t(tab.labelKey) }}
        </button>
      </div>

      <div class="p-5">
        <div v-if="loading" class="rounded-lg border border-dashed border-border py-12 text-center text-sm text-muted-foreground">
          {{ t('common.loading') }}
        </div>

        <div v-else-if="orders.length === 0" class="rounded-lg border border-dashed border-border py-12 text-center text-sm text-muted-foreground">
          {{ t('loan.no_order_records') }}
        </div>

        <div v-else class="overflow-x-auto">
          <table class="w-full table-fixed text-sm">
            <colgroup>
              <col class="w-[5%]" />
              <col class="w-[18%]" />
              <col class="w-[7%]" />
              <col class="w-[16%]" />
              <col class="w-[16%]" />
              <col class="w-[10%]" />
              <col class="w-[16%]" />
              <col class="w-[12%]" />
            </colgroup>
            <thead class="border-b border-border text-xs font-semibold text-muted-foreground">
              <tr>
                <th class="px-3 py-3 text-left"><span class="sr-only">{{ t('common.expand') }}</span></th>
                <th class="px-4 py-3 text-left whitespace-nowrap">{{ t('loan.product') }}</th>
                <th class="px-2 py-3 text-left whitespace-nowrap">{{ t('loan.type') }}</th>
                <th class="px-4 py-3 text-right whitespace-nowrap">{{ t('loan.amount') }}</th>
                <th class="px-4 py-3 text-right whitespace-nowrap">{{ t('loan.repayment') }}</th>
                <th class="px-4 py-3 text-left whitespace-nowrap">{{ t('loan.status') }}</th>
                <th class="px-4 py-3 text-left whitespace-nowrap">{{ t('loan.create_time') }}</th>
                <th class="px-4 py-3 text-right whitespace-nowrap">{{ t('loan.actions') }}</th>
              </tr>
            </thead>
            <tbody>
              <template v-for="order in orders" :key="order.id">
                <tr class="border-b border-border/60 align-middle transition-colors hover:bg-muted/10" :class="isOrderExpanded(order.id) ? 'bg-muted/10' : ''">
                  <td class="px-3 py-4">
                    <button
                      type="button"
                      class="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground"
                      :aria-expanded="isOrderExpanded(order.id)"
                      :aria-label="isOrderExpanded(order.id) ? t('common.collapse') : t('common.expand')"
                      :title="isOrderExpanded(order.id) ? t('common.collapse') : t('common.expand')"
                      @click="toggleOrderExpanded(order.id)"
                    >
                      <Icon icon="mdi:chevron-right" class="h-4 w-4 transition-transform" :class="isOrderExpanded(order.id) ? 'rotate-90' : ''" />
                    </button>
                  </td>
                  <td class="px-4 py-4">
                    <span class="block truncate font-semibold">{{ orderProductName(order) }}</span>
                  </td>
                  <td class="px-2 py-4 whitespace-nowrap">{{ loanTypeText(order.loan_type) }}</td>
                  <td class="px-4 py-4 text-right">
                    <span class="inline-flex items-baseline justify-end gap-2 whitespace-nowrap font-mono">
                      <span>{{ formatAmount(order.amount) }}</span>
                      <span class="text-xs text-muted-foreground">{{ order.asset_symbol }}</span>
                    </span>
                  </td>
                  <td class="px-4 py-4 text-right">
                    <span class="inline-flex items-baseline justify-end gap-2 whitespace-nowrap font-mono font-semibold">
                      <span>{{ formatAmount(orderPayableRepayment(order)) }}</span>
                      <span class="text-xs font-normal text-muted-foreground">{{ order.asset_symbol }}</span>
                    </span>
                  </td>
                  <td class="px-4 py-4">
                    <span class="rounded-full px-2.5 py-1 text-xs font-semibold" :class="statusClass(order.status)">
                      {{ statusText(order.status) }}
                    </span>
                  </td>
                  <td class="px-4 py-4 whitespace-nowrap text-muted-foreground">{{ formatTime(order.created_at) }}</td>
                  <td class="px-4 py-4 text-right">
                    <div class="inline-flex justify-end gap-2">
                      <button
                        v-if="order.status === 'pending'"
                        type="button"
                        class="rounded bg-muted px-3 py-1.5 text-xs font-semibold hover:bg-muted/80 disabled:opacity-50"
                        :disabled="actionLoadingId === order.id"
                        @click="cancelOrder(order)"
                      >
                        {{ t('loan.cancel_order') }}
                      </button>
                      <button
                        v-if="order.status === 'disbursed'"
                        type="button"
                        class="rounded bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                        :disabled="actionLoadingId === order.id"
                        @click="repayOrder(order)"
                      >
                        {{ t('loan.repay_now') }}
                      </button>
                      <span v-if="order.status !== 'pending' && order.status !== 'disbursed'" class="text-xs text-muted-foreground">--</span>
                    </div>
                  </td>
                </tr>
                <tr v-if="isOrderExpanded(order.id)" class="border-b border-border/60">
                  <td colspan="8" class="bg-muted/10 px-4 py-0">
                    <div class="grid gap-x-8 gap-y-4 border-t border-border/50 py-5 sm:grid-cols-2 xl:grid-cols-4">
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.total_interest') }}</p>
                        <p class="font-mono text-sm font-semibold">{{ formatAmount(orderPayableInterest(order)) }} {{ order.asset_symbol }}</p>
                      </div>
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.interest_rate') }}</p>
                        <p class="text-sm font-semibold">{{ percentText(order.interest_rate) }}</p>
                      </div>
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.select_term') }}</p>
                        <p class="text-sm font-semibold">{{ order.term_days }} {{ t('loan.days') }}</p>
                      </div>
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.interest_mode') }}</p>
                        <p class="text-sm font-semibold">{{ interestModeText(order.interest_calculation_mode) }}</p>
                      </div>
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.collateral') }}</p>
                        <p class="font-mono text-sm font-semibold">{{ collateralText(order) }}</p>
                      </div>
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.status') }}</p>
                        <p class="text-sm font-semibold">{{ statusText(order.status) }}</p>
                      </div>
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.create_time') }}</p>
                        <p class="text-sm font-semibold">{{ formatTime(order.created_at) }}</p>
                      </div>
                      <div class="space-y-1">
                        <p class="text-xs text-muted-foreground">{{ t('loan.repayment') }}</p>
                        <p class="font-mono text-sm font-semibold">{{ formatAmount(orderPayableRepayment(order)) }} {{ order.asset_symbol }}</p>
                      </div>
                    </div>
                  </td>
                </tr>
              </template>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Icon } from '@iconify/vue'
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useToast } from 'vue-toastification'

import { cancelLoanOrder, fetchLoanOrders, localizedLoanName, type InstallmentsOrder, type LoanOrderStatus, type LoanType, repayLoanOrder } from '@/api/loan'
import { formatNumber } from '@/utils/format'
import { estimateLoanOrderInterest, estimateLoanOrderRepayment, parseLoanNumber } from '@/utils/loan'

const { t, locale } = useI18n()
const toast = useToast()

type StatusTab = { value: LoanOrderStatus | ''; labelKey: string }

const statusTabs: StatusTab[] = [
  { value: '', labelKey: 'loan.status_all' },
  { value: 'pending', labelKey: 'loan.status_pending' },
  { value: 'disbursed', labelKey: 'loan.status_disbursed' },
  { value: 'repaid', labelKey: 'loan.status_repaid' },
  { value: 'rejected', labelKey: 'loan.status_rejected' },
  { value: 'cancelled', labelKey: 'loan.status_cancelled' },
]

const orders = ref<InstallmentsOrder[]>([])
const activeStatus = ref<LoanOrderStatus | ''>('')
const loading = ref(false)
const actionLoadingId = ref<number | null>(null)
const expandedOrderIds = ref<Set<number>>(new Set())

onMounted(loadOrders)

function setStatus(status: LoanOrderStatus | '') {
  activeStatus.value = status
  loadOrders()
}

async function loadOrders() {
  loading.value = true
  try {
    const response = await fetchLoanOrders(activeStatus.value || undefined)
    orders.value = response.data.data.content
  } catch (error) {
    toast.error(errorMessage(error, t('loan.load_orders_failed')))
  } finally {
    loading.value = false
  }
}

async function cancelOrder(order: InstallmentsOrder) {
  if (!window.confirm(t('loan.cancel_confirm'))) return
  actionLoadingId.value = order.id
  try {
    await cancelLoanOrder(order.id)
    toast.success(t('loan.cancel_success'))
    await loadOrders()
  } catch (error) {
    toast.error(errorMessage(error, t('loan.cancel_failed')))
  } finally {
    actionLoadingId.value = null
  }
}

async function repayOrder(order: InstallmentsOrder) {
  if (!window.confirm(t('loan.repay_confirm', { amount: `${formatAmount(orderPayableRepayment(order))} ${order.asset_symbol}` }))) return
  actionLoadingId.value = order.id
  try {
    await repayLoanOrder(order.id)
    toast.success(t('loan.pay_success'))
    await loadOrders()
  } catch (error) {
    toast.error(errorMessage(error, t('loan.pay_failed')))
  } finally {
    actionLoadingId.value = null
  }
}

function loanTypeText(type: LoanType) {
  return t(type === 'collateralized' ? 'loan.type_collateralized' : 'loan.type_credit')
}

function orderProductName(order: InstallmentsOrder) {
  return localizedLoanName(order.product_name_json, order.product_name, String(locale.value || ''))
}

function statusText(status: LoanOrderStatus) {
  return t(`loan.status_${status}`)
}

function statusClass(status: LoanOrderStatus) {
  if (status === 'repaid') return 'bg-green-500/10 text-green-500'
  if (status === 'disbursed') return 'bg-blue-500/10 text-blue-500'
  if (status === 'pending') return 'bg-yellow-500/10 text-yellow-500'
  if (status === 'rejected') return 'bg-red-500/10 text-red-500'
  return 'bg-muted text-muted-foreground'
}

function isOrderExpanded(orderId: number) {
  return expandedOrderIds.value.has(orderId)
}

function toggleOrderExpanded(orderId: number) {
  const next = new Set(expandedOrderIds.value)
  if (next.has(orderId)) {
    next.delete(orderId)
  } else {
    next.add(orderId)
  }
  expandedOrderIds.value = next
}

function orderPayableInterest(order: InstallmentsOrder) {
  return estimateLoanOrderInterest(order)
}

function orderPayableRepayment(order: InstallmentsOrder) {
  return estimateLoanOrderRepayment(order)
}

function interestModeText(mode: string) {
  return t(mode === 'actual_days' ? 'loan.interest_mode_actual_days' : 'loan.interest_mode_full_term')
}

function percentText(value: string | number) {
  const parsed = parseLoanNumber(value)
  if (parsed === null) return '--'
  return `${formatNumber(parsed * 100, 'amount')}%`
}

function collateralText(order: InstallmentsOrder) {
  if (!order.collateral_asset_symbol) return '--'
  return `${formatAmount(order.collateral_amount)} ${order.collateral_asset_symbol}`
}

function formatAmount(value: string | number | null | undefined) {
  return formatNumber(Number(value ?? 0), 'amount')
}

function formatTime(value?: number | null) {
  if (!value) return '--'
  return new Date(value).toLocaleString()
}

function errorMessage(error: unknown, fallback: string) {
  const responseMessage = (error as { response?: { data?: { message?: unknown } } })?.response?.data?.message
  if (typeof responseMessage === 'string' && responseMessage.trim()) return responseMessage
  return error instanceof Error ? error.message : fallback
}
</script>
