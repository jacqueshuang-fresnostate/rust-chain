<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight, ChevronsRight, X } from 'lucide-vue-next'
import { formatAmount, formatPrice } from '@/core/format'
import {
  MARGIN_CLOSE_MAX_PERCENTAGE,
  MARGIN_CLOSE_MIN_PERCENTAGE,
  marginClosePreviewAmount,
  normalizeMarginClosePercentage,
} from '@/core/marginClose'
import { useModalDialog } from '@/core/modalDialog'
import {
  isSlideConfirmComplete,
  slideProgressForKey,
  slideProgressFromClientX,
} from '@/core/slideToConfirm'

const props = defineProps<{
  open: boolean
  saving: boolean
  returnFocus?: HTMLElement | null
  symbol: string
  direction: 'long' | 'short'
  marginMode: 'cross' | 'isolated'
  leverage: number
  baseAsset: string
  quoteAsset: string
  markPrice: number | null
  positionQuantity: number | null
  estimatedPnl: number | null
  error?: string
}>()

const emit = defineEmits<{
  close: []
  confirm: [percentage: number]
}>()

const { t } = useI18n()
const dialog = ref<HTMLElement | null>(null)
const slideTrack = ref<HTMLElement | null>(null)
const slideProgress = ref(0)
const closePercentage = ref(MARGIN_CLOSE_MAX_PERCENTAGE)
const dragging = ref(false)
const confirmationSent = ref(false)
let activePointerId: number | null = null

const dialogOpen = computed(() => props.open)
const compactSymbol = computed(() => props.symbol.replace(/[\/_-]/g, '').toUpperCase())
const priceText = computed(() => validNumber(props.markPrice) ? formatPrice(props.markPrice!) : '--')
const positionQuantityText = computed(() => validNumber(props.positionQuantity)
  ? `${formatAmount(props.positionQuantity!)} ${props.baseAsset}`
  : '--')
const closableQuantity = computed(() => (
  marginClosePreviewAmount(props.positionQuantity, closePercentage.value)
))
const closableQuantityText = computed(() => validNumber(closableQuantity.value)
  ? `${formatAmount(closableQuantity.value)} ${props.baseAsset}`
  : '--')
const selectedEstimatedPnl = computed(() => (
  marginClosePreviewAmount(props.estimatedPnl, closePercentage.value)
))
const pnlText = computed(() => {
  if (!validNumber(selectedEstimatedPnl.value)) return '--'
  const value = selectedEstimatedPnl.value
  return `${value > 0 ? '+' : ''}${formatAmount(value)} ${props.quoteAsset}`
})
const pnlTone = computed(() => !validNumber(selectedEstimatedPnl.value)
  ? 'neutral'
  : selectedEstimatedPnl.value >= 0 ? 'positive' : 'negative')
const ratioStyle = computed(() => ({
  '--margin-close-ratio': `${closePercentage.value}%`,
}))
const handleStyle = computed(() => {
  const progress = slideProgress.value
  return {
    left: `calc(6px + ${progress * 100}% - ${progress * 62}px)`,
  }
})
const fillStyle = computed(() => {
  const progress = slideProgress.value
  return {
    width: `calc(56px + ${progress * 100}% - ${progress * 62}px)`,
  }
})

const { trapFocus, setReturnFocus } = useModalDialog(
  dialogOpen,
  dialog,
  '[data-dialog-initial]',
)

watch(() => props.open, (open) => {
  if (open) {
    setReturnFocus(props.returnFocus || null)
    closePercentage.value = MARGIN_CLOSE_MAX_PERCENTAGE
    resetSlide()
  } else {
    resetSlide()
  }
}, { flush: 'sync' })

watch(() => props.error, (error, previousError) => {
  if (error && error !== previousError) resetSlide()
})

function validNumber(value: number | null | undefined): value is number {
  return value !== null && value !== undefined && Number.isFinite(value)
}

function requestClose(): void {
  if (!props.saving) emit('close')
}

function handleDialogKeydown(event: KeyboardEvent): void {
  trapFocus(event, requestClose)
}

function resetSlide(): void {
  activePointerId = null
  dragging.value = false
  confirmationSent.value = false
  slideProgress.value = 0
}

function updateSlideFromPointer(event: PointerEvent): void {
  const track = slideTrack.value
  if (!track) return
  const bounds = track.getBoundingClientRect()
  slideProgress.value = slideProgressFromClientX(
    event.clientX,
    bounds.left,
    bounds.width,
    50,
    6,
  )
}

function handleSlidePointerDown(event: PointerEvent): void {
  if (props.saving || confirmationSent.value || event.button > 0) return
  // 只能从圆形手柄发起拖动；点击轨道末端不得把一次点按误判为平仓确认。
  if (!(event.target instanceof Element) || !event.target.closest('.margin-close-slide__handle')) return
  event.preventDefault()
  activePointerId = event.pointerId
  dragging.value = true
  slideTrack.value?.setPointerCapture(event.pointerId)
  updateSlideFromPointer(event)
}

function handleSlidePointerMove(event: PointerEvent): void {
  if (!dragging.value || activePointerId !== event.pointerId || props.saving) return
  event.preventDefault()
  updateSlideFromPointer(event)
}

function handleSlidePointerUp(event: PointerEvent): void {
  if (!dragging.value || activePointerId !== event.pointerId) return
  event.preventDefault()
  updateSlideFromPointer(event)
  if (slideTrack.value?.hasPointerCapture(event.pointerId)) {
    slideTrack.value.releasePointerCapture(event.pointerId)
  }
  activePointerId = null
  dragging.value = false
  if (isSlideConfirmComplete(slideProgress.value)) {
    requestConfirm()
  } else {
    resetSlide()
  }
}

function handleSlidePointerCancel(event: PointerEvent): void {
  if (activePointerId !== event.pointerId) return
  if (slideTrack.value?.hasPointerCapture(event.pointerId)) {
    slideTrack.value.releasePointerCapture(event.pointerId)
  }
  resetSlide()
}

function handleSlideKeydown(event: KeyboardEvent): void {
  if (props.saving || confirmationSent.value) return
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    if (isSlideConfirmComplete(slideProgress.value)) requestConfirm()
    return
  }
  const nextProgress = slideProgressForKey(slideProgress.value, event.key)
  if (nextProgress === null) return
  event.preventDefault()
  slideProgress.value = nextProgress
}

function handleRatioInput(): void {
  closePercentage.value = normalizeMarginClosePercentage(closePercentage.value)
  // 比例改变后，旧的确认进度不再代表当前意图，必须重新从起点确认。
  resetSlide()
}

function requestConfirm(): void {
  if (props.saving || confirmationSent.value) return
  confirmationSent.value = true
  slideProgress.value = 1
  emit('confirm', closePercentage.value)
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="margin-close-layer"
      data-pencil-source="ajSJF DGiNR"
    >
      <button
        class="margin-close-overlay"
        type="button"
        tabindex="-1"
        :disabled="saving"
        :aria-label="t('common.close')"
        @click="requestClose"
      />

      <section
        id="margin-close-dialog"
        ref="dialog"
        class="margin-close-sheet"
        role="dialog"
        aria-modal="true"
        aria-labelledby="margin-close-title"
        :aria-describedby="error ? 'margin-close-error' : undefined"
        :aria-busy="saving"
        tabindex="-1"
        @keydown="handleDialogKeydown"
      >
        <header class="margin-close-sheet__header">
          <h2 id="margin-close-title">{{ t('trade.marginCloseTitle') }}</h2>
          <button
            data-dialog-initial
            type="button"
            :disabled="saving"
            :aria-label="t('common.close')"
            @click="requestClose"
          >
            <X :size="24" aria-hidden="true" />
          </button>
        </header>

        <div class="margin-close-sheet__identity">
          <strong>{{ compactSymbol }} {{ t('trade.perpetualShort') }}</strong>
          <span :class="direction === 'long' ? 'is-long' : 'is-short'">
            {{ t(direction === 'long' ? 'orders.long' : 'orders.short') }}
          </span>
          <span>{{ t(marginMode === 'cross' ? 'trade.cross' : 'trade.isolated') }}</span>
          <span class="numeric">{{ leverage }}x</span>
        </div>

        <div class="margin-close-sheet__price">
          <div>
            <span>{{ t('trade.priceField', { asset: quoteAsset }) }}</span>
            <strong class="numeric">{{ priceText }}</strong>
          </div>
          <span>{{ t('trade.marginCloseMarketPrice') }}</span>
        </div>

        <div class="margin-close-sheet__latest">
          <span>{{ t('trade.marginCloseLatestPrice') }}</span>
          <strong class="numeric">{{ priceText }}<template v-if="priceText !== '--'"> {{ quoteAsset }}</template></strong>
        </div>

        <div class="margin-close-sheet__quantity">
          <span>{{ t('trade.marginCloseQuantity', { asset: baseAsset }) }}</span>
          <strong class="numeric">{{ t('trade.marginClosePercentage', { percentage: closePercentage }) }}</strong>
        </div>

        <div class="margin-close-sheet__ratio" :style="ratioStyle">
          <span class="margin-close-sheet__ratio-track" aria-hidden="true">
            <b />
            <i
              v-for="value in [1, 25, 50, 75, 100]"
              :key="value"
              :class="{ active: value <= closePercentage }"
              :style="{ left: `${value}%` }"
            />
          </span>
          <input
            v-model.number="closePercentage"
            class="margin-close-sheet__ratio-input"
            type="range"
            min="1"
            max="100"
            step="1"
            :disabled="saving"
            :aria-label="t('trade.marginCloseRatioLabel')"
            :aria-valuetext="t('trade.marginClosePercentage', { percentage: closePercentage })"
            @input="handleRatioInput"
          >
        </div>

        <dl class="margin-close-sheet__stats">
          <div>
            <dt>{{ t('trade.marginClosePositionAmount') }}</dt>
            <dd class="numeric">{{ positionQuantityText }}</dd>
          </div>
          <div>
            <dt>{{ t('trade.marginCloseAvailableAmount') }}</dt>
            <dd class="numeric">{{ closableQuantityText }}</dd>
          </div>
          <div>
            <dt>{{ t('trade.marginCloseEstimatedPnl') }}</dt>
            <dd class="numeric" :class="`is-${pnlTone}`">{{ pnlText }}</dd>
          </div>
        </dl>

        <div class="margin-close-sheet__message">
          <p v-if="error" id="margin-close-error" role="alert">{{ error }}</p>
          <p v-else>{{ t('trade.marginCloseSelectedPosition', { percentage: closePercentage }) }}</p>
        </div>

        <div
          ref="slideTrack"
          class="margin-close-slide"
          :class="{ 'is-dragging': dragging, 'is-saving': saving }"
          role="slider"
          tabindex="0"
          aria-valuemin="0"
          aria-valuemax="100"
          :aria-valuenow="Math.round(slideProgress * 100)"
          :aria-valuetext="t('trade.marginCloseSlideProgress', { progress: Math.round(slideProgress * 100) })"
          :aria-label="t('trade.marginCloseSlideAction')"
          :aria-disabled="saving"
          @pointerdown="handleSlidePointerDown"
          @pointermove="handleSlidePointerMove"
          @pointerup="handleSlidePointerUp"
          @pointercancel="handleSlidePointerCancel"
          @keydown="handleSlideKeydown"
        >
          <span class="margin-close-slide__fill" :style="fillStyle" aria-hidden="true" />
          <span class="margin-close-slide__copy">
            <strong>{{ saving ? t('orders.processing') : t('trade.marginCloseSlideAction') }}</strong>
            <small>{{ closableQuantityText === '--' ? t('trade.marginCloseSlideReady') : `≈ ${closableQuantityText}` }}</small>
          </span>
          <span class="margin-close-slide__direction" aria-hidden="true"><ChevronsRight :size="18" /></span>
          <span class="margin-close-slide__handle" :style="handleStyle" aria-hidden="true">
            <ChevronRight :size="24" />
          </span>
        </div>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.margin-close-layer,
.margin-close-layer * {
  box-sizing: border-box;
}

.margin-close-layer {
  align-items: end;
  bottom: 0;
  display: grid;
  height: 100vh;
  height: 100dvh;
  isolation: isolate;
  left: auto;
  overflow: hidden;
  overscroll-behavior: contain;
  position: fixed;
  right: 5.5vw;
  top: 0;
  width: min(100%, 448px);
  z-index: var(--layer-overlay, 80);
}

.margin-close-overlay {
  background: rgb(0 0 0 / 72%);
  border: 0;
  inset: 0;
  padding: 0;
  position: absolute;
  width: 100%;
}

.margin-close-overlay:disabled {
  opacity: 1;
}

.margin-close-sheet {
  --close-sheet-page: #ffffff;
  --close-sheet-field: #f2f4f3;
  --close-sheet-text: #101512;
  --close-sheet-muted: #728078;
  --close-sheet-line: #d9e2dd;
  --close-sheet-positive: #159e70;
  --close-sheet-positive-soft: #ddf9ee;
  --close-sheet-negative: #e94f37;
  --close-sheet-negative-soft: #fff0ec;
  --close-sheet-action: #ff3e73;
  --close-sheet-action-deep: #d9295b;
  background: var(--close-sheet-page);
  border: 0;
  border-radius: 24px 24px 0 0;
  box-shadow: 0 -6px 20px rgb(0 0 0 / 15%);
  color: var(--close-sheet-text);
  display: grid;
  gap: 9px;
  grid-template-rows:
    40px
    38px
    58px
    17px
    58px
    24px
    69px
    minmax(0, 32px)
    62px;
  height: min(
    calc(500px + env(safe-area-inset-bottom, 0px)),
    calc(100dvh - max(12px, env(safe-area-inset-top, 0px)))
  );
  justify-self: center;
  max-width: 448px;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 14px 20px calc(16px + env(safe-area-inset-bottom, 0px));
  position: relative;
  scrollbar-width: none;
  width: 100%;
  z-index: 1;
}

:global(html[data-theme='dark'] .margin-close-sheet) {
  --close-sheet-page: #0b0f0d;
  --close-sheet-field: #181e1a;
  --close-sheet-text: #f5f7f6;
  --close-sheet-muted: #8b9690;
  --close-sheet-line: #303a35;
  --close-sheet-positive: #37e6a6;
  --close-sheet-positive-soft: #123a2c;
  --close-sheet-negative: #ff654a;
  --close-sheet-negative-soft: #3b1b16;
  box-shadow: 0 -10px 32px rgb(0 0 0 / 55%);
}

.margin-close-sheet::-webkit-scrollbar {
  display: none;
}

.margin-close-sheet__header {
  align-items: center;
  display: flex;
  height: 40px;
  justify-content: space-between;
}

.margin-close-sheet__header h2 {
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -.03em;
  line-height: 35px;
  margin: 0;
}

.margin-close-sheet__header button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 50%;
  color: var(--close-sheet-muted);
  display: flex;
  height: 44px;
  justify-content: center;
  margin-inline-end: -4px;
  padding: 0;
  width: 44px;
}

.margin-close-sheet__identity {
  align-items: center;
  display: flex;
  gap: 7px;
  min-width: 0;
}

.margin-close-sheet__identity strong {
  flex: 0 1 auto;
  font-size: 18px;
  font-weight: 700;
  line-height: 26px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-close-sheet__identity > span {
  align-items: center;
  background: var(--close-sheet-field);
  border-radius: 7px;
  display: inline-flex;
  flex: 0 0 auto;
  font-size: 11px;
  font-weight: 700;
  height: 26px;
  justify-content: center;
  min-width: 27px;
  padding: 0 8px;
}

.margin-close-sheet__identity > span.is-long {
  background: var(--close-sheet-positive-soft);
  color: var(--close-sheet-positive);
}

.margin-close-sheet__identity > span.is-short {
  background: var(--close-sheet-negative-soft);
  color: var(--close-sheet-negative);
}

.margin-close-sheet__price {
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(0, 1fr) minmax(78px, 92px);
  min-width: 0;
}

.margin-close-sheet__price > div,
.margin-close-sheet__price > span,
.margin-close-sheet__quantity {
  background: var(--close-sheet-field);
  border-radius: 12px;
}

.margin-close-sheet__price > div,
.margin-close-sheet__quantity {
  display: grid;
  grid-template-rows: 16px 24px;
  padding: 8px 12px;
}

.margin-close-sheet__price > div > span,
.margin-close-sheet__quantity > span {
  color: var(--close-sheet-muted);
  font-size: 11px;
  font-weight: 500;
  line-height: 16px;
}

.margin-close-sheet__price strong,
.margin-close-sheet__quantity strong {
  font-size: 20px;
  font-weight: 600;
  line-height: 24px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-close-sheet__price > span {
  align-items: center;
  display: flex;
  font-size: 18px;
  font-weight: 600;
  justify-content: center;
}

.margin-close-sheet__latest {
  align-items: center;
  display: flex;
  gap: 5px;
  min-width: 0;
}

.margin-close-sheet__latest span {
  color: var(--close-sheet-muted);
  font-size: 12px;
  font-weight: 500;
}

.margin-close-sheet__latest strong {
  font-size: 14px;
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-close-sheet__ratio {
  align-items: center;
  display: flex;
  height: 24px;
  position: relative;
}

.margin-close-sheet__ratio-track {
  background: var(--close-sheet-line);
  border-radius: 999px;
  height: 3px;
  left: 8px;
  overflow: visible;
  pointer-events: none;
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
}

.margin-close-sheet__ratio-track b {
  background: var(--close-sheet-text);
  border-radius: inherit;
  display: block;
  height: 100%;
  max-width: 100%;
  width: var(--margin-close-ratio);
}

.margin-close-sheet__ratio-track i {
  background: var(--close-sheet-page);
  border: 1px solid var(--close-sheet-line);
  border-radius: 50%;
  display: block;
  height: 7px;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 7px;
}

.margin-close-sheet__ratio-track i.active {
  background: var(--close-sheet-text);
  border-color: var(--close-sheet-text);
}

.margin-close-sheet__ratio-input {
  appearance: none;
  background: transparent;
  cursor: grab;
  height: 24px;
  margin: 0;
  outline: 0;
  padding: 0;
  position: relative;
  touch-action: pan-y;
  width: 100%;
  z-index: 1;
}

.margin-close-sheet__ratio-input:active {
  cursor: grabbing;
}

.margin-close-sheet__ratio-input:disabled {
  cursor: wait;
  opacity: .62;
}

.margin-close-sheet__ratio-input::-webkit-slider-runnable-track {
  background: transparent;
  border: 0;
  height: 24px;
}

.margin-close-sheet__ratio-input::-moz-range-track {
  background: transparent;
  border: 0;
  height: 24px;
}

.margin-close-sheet__ratio-input::-webkit-slider-thumb {
  appearance: none;
  background: var(--close-sheet-page);
  border: 4px solid var(--close-sheet-text);
  border-radius: 50%;
  box-shadow: 0 2px 7px rgb(0 0 0 / 18%);
  height: 18px;
  margin-top: 3px;
  width: 18px;
}

.margin-close-sheet__ratio-input::-moz-range-thumb {
  background: var(--close-sheet-page);
  border: 4px solid var(--close-sheet-text);
  border-radius: 50%;
  box-shadow: 0 2px 7px rgb(0 0 0 / 18%);
  height: 10px;
  width: 10px;
}

.margin-close-sheet__ratio-input:focus-visible {
  border-radius: 12px;
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--close-sheet-positive) 22%, transparent);
  outline: 2px solid var(--close-sheet-positive);
  outline-offset: 2px;
}

.margin-close-sheet__stats {
  display: grid;
  gap: 6px;
  margin: 0;
}

.margin-close-sheet__stats > div {
  align-items: center;
  display: flex;
  justify-content: space-between;
  min-width: 0;
}

.margin-close-sheet__stats dt {
  color: var(--close-sheet-muted);
  font-size: 13px;
  font-weight: 500;
}

.margin-close-sheet__stats dd {
  font-size: 14px;
  font-weight: 600;
  margin: 0;
  max-width: 70%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.margin-close-sheet__stats dd.is-positive { color: var(--close-sheet-positive); }
.margin-close-sheet__stats dd.is-negative { color: var(--close-sheet-negative); }
.margin-close-sheet__stats dd.is-neutral { color: var(--close-sheet-text); }

.margin-close-sheet__message {
  align-items: center;
  display: flex;
  justify-content: center;
  min-height: 0;
}

.margin-close-sheet__message p {
  color: var(--close-sheet-muted);
  font-size: 10px;
  line-height: 14px;
  margin: 0;
  max-height: 28px;
  overflow: hidden;
  text-align: center;
}

.margin-close-sheet__message p[role='alert'] {
  color: var(--close-sheet-negative);
}

.margin-close-slide {
  background: var(--close-sheet-action);
  border-radius: 31px;
  color: #ffffff;
  cursor: grab;
  height: 62px;
  isolation: isolate;
  outline: 0;
  overflow: hidden;
  position: relative;
  touch-action: none;
  user-select: none;
}

.margin-close-slide.is-dragging {
  cursor: grabbing;
}

.margin-close-slide.is-saving {
  cursor: wait;
  opacity: .72;
}

.margin-close-slide__fill {
  background: var(--close-sheet-action-deep);
  border-radius: inherit;
  bottom: 0;
  left: 0;
  opacity: .72;
  position: absolute;
  top: 0;
  transition: width 180ms cubic-bezier(.2, .8, .2, 1);
}

.margin-close-slide.is-dragging .margin-close-slide__fill,
.margin-close-slide.is-dragging .margin-close-slide__handle {
  transition: none;
}

.margin-close-slide__copy {
  align-items: center;
  display: grid;
  inset: 0 62px;
  justify-items: center;
  pointer-events: none;
  position: absolute;
  z-index: 1;
}

.margin-close-slide__copy strong {
  align-self: end;
  font-size: 16px;
  font-weight: 700;
  line-height: 22px;
}

.margin-close-slide__copy small {
  align-self: start;
  font-size: 10px;
  font-weight: 600;
  line-height: 15px;
  opacity: .9;
}

.margin-close-slide__direction {
  align-items: center;
  display: flex;
  height: 62px;
  justify-content: center;
  opacity: .68;
  position: absolute;
  right: 16px;
  top: 0;
  width: 22px;
  z-index: 1;
}

.margin-close-slide__handle {
  align-items: center;
  background: #ffffff;
  border: 1px solid rgb(255 255 255 / 80%);
  border-radius: 50%;
  box-shadow: 0 4px 12px rgb(95 4 35 / 24%), inset 0 1px 0 rgb(255 255 255 / 90%);
  color: var(--close-sheet-action-deep);
  display: flex;
  height: 50px;
  justify-content: center;
  position: absolute;
  top: 6px;
  transition: left 180ms cubic-bezier(.2, .8, .2, 1), transform 120ms ease;
  width: 50px;
  z-index: 2;
}

.margin-close-slide.is-dragging .margin-close-slide__handle {
  transform: scale(.96);
}

.margin-close-slide:focus-visible {
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--close-sheet-action) 34%, transparent);
  outline: 2px solid var(--close-sheet-action);
  outline-offset: 3px;
}

.margin-close-sheet__header button:focus-visible {
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--close-sheet-positive) 24%, transparent);
  outline: 2px solid var(--close-sheet-positive);
  outline-offset: 1px;
}

@media (max-width: 820px) {
  .margin-close-layer {
    right: 0;
    width: 100%;
  }
}

@media (max-width: 340px) {
  .margin-close-sheet {
    padding-left: 16px;
    padding-right: 16px;
  }

  .margin-close-sheet__identity {
    gap: 5px;
  }

  .margin-close-sheet__identity strong {
    font-size: 16px;
  }

  .margin-close-sheet__identity > span {
    padding-inline: 6px;
  }
}

@media (prefers-reduced-motion: no-preference) {
  .margin-close-sheet {
    animation: margin-close-sheet-enter 260ms cubic-bezier(.2, .8, .2, 1) both;
  }
}

@media (prefers-reduced-motion: reduce) {
  .margin-close-sheet,
  .margin-close-slide__fill,
  .margin-close-slide__handle {
    animation: none;
    transition: none;
  }
}

@keyframes margin-close-sheet-enter {
  from { opacity: .85; transform: translateY(28px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
