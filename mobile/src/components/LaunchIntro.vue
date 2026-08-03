<script setup lang="ts">
import { gsap } from 'gsap'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import compactLogo from '@/assets/brand/hippo-logo-compact.png'
import {
  rememberLaunchIntro,
  shouldPlayLaunchIntro,
  type LaunchIntroStorage,
} from '@/core/launchIntro'

const SCROLL_LOCK_CLASS = 'launch-intro-active'
const AUTO_DISMISS_MS = 3000

function resolveSessionStorage(): LaunchIntroStorage | null {
  try {
    return window.sessionStorage
  } catch {
    return null
  }
}

const storage = resolveSessionStorage()
const isVisible = ref(shouldPlayLaunchIntro(storage))
const rootRef = ref<HTMLElement | null>(null)

let animationContext: gsap.Context | null = null
let timeline: gsap.core.Timeline | null = null
let autoDismissTimer: number | null = null
let hasFinished = false

if (isVisible.value) {
  rememberLaunchIntro(storage)
}

function unlockScroll() {
  document.documentElement.classList.remove(SCROLL_LOCK_CLASS)
}

function clearAutoDismissTimer() {
  if (autoDismissTimer === null) return
  window.clearTimeout(autoDismissTimer)
  autoDismissTimer = null
}

function releaseAnimation() {
  timeline?.kill()
  timeline = null
  animationContext?.revert()
  animationContext = null
}

function finishIntro() {
  if (hasFinished) return
  hasFinished = true
  clearAutoDismissTimer()
  unlockScroll()
  releaseAnimation()
  isVisible.value = false
}

onMounted(() => {
  if (!isVisible.value) return

  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    finishIntro()
    return
  }

  const root = rootRef.value
  if (!root) {
    finishIntro()
    return
  }

  document.documentElement.classList.add(SCROLL_LOCK_CLASS)
  autoDismissTimer = window.setTimeout(finishIntro, AUTO_DISMISS_MS)

  try {
    animationContext = gsap.context(() => {
      timeline = gsap.timeline({
        defaults: { ease: 'power3.out' },
        onComplete: finishIntro,
      })

      timeline
        .to('.launch-intro__edge', {
          duration: 0.32,
          opacity: 0.54,
          scaleY: 1,
          stagger: 0.06,
        }, 0)
        .to('.launch-intro__logo-window', {
          clipPath: 'inset(0 0% 0 0%)',
          duration: 0.72,
          ease: 'power4.out',
        }, 0.08)
        .fromTo('.launch-intro__logo-mark', {
          opacity: 0.46,
          scale: 0.965,
        }, {
          duration: 0.92,
          ease: 'expo.out',
          opacity: 1,
          scale: 1,
        }, 0.08)
        .to('.launch-intro__progress-fill', {
          duration: 0.66,
          ease: 'power3.inOut',
          scaleX: 1,
        }, 0.34)
        .to('.launch-intro__signature', {
          autoAlpha: 1,
          duration: 0.36,
          y: 0,
        }, 0.42)
        .to('.launch-intro__light-pass', {
          autoAlpha: 0.42,
          duration: 0.1,
        }, 0.32)
        .to('.launch-intro__light-pass', {
          duration: 0.76,
          ease: 'power2.inOut',
          xPercent: 850,
        }, 0.32)
        .to('.launch-intro__light-pass', {
          autoAlpha: 0,
          duration: 0.14,
        }, 0.92)
        .to('.launch-intro__brand', {
          autoAlpha: 0,
          duration: 0.22,
          ease: 'power2.in',
          y: -4,
        }, 1.28)
        .to('.launch-intro__edge', {
          duration: 0.18,
          opacity: 0,
        }, 1.3)
        .to('.launch-intro__curtain--left', {
          duration: 0.62,
          ease: 'expo.inOut',
          xPercent: -101,
        }, 1.4)
        .to('.launch-intro__curtain--right', {
          duration: 0.62,
          ease: 'expo.inOut',
          xPercent: 101,
        }, 1.4)
        .to(root, {
          autoAlpha: 0,
          duration: 0.04,
        }, 2.02)
    }, root)
  } catch {
    finishIntro()
  }
})

onBeforeUnmount(() => {
  clearAutoDismissTimer()
  unlockScroll()
  releaseAnimation()
})
</script>

<template>
  <div
    v-if="isVisible"
    ref="rootRef"
    class="launch-intro"
    aria-hidden="true"
  >
    <div class="launch-intro__curtain launch-intro__curtain--left" />
    <div class="launch-intro__curtain launch-intro__curtain--right" />

    <span class="launch-intro__edge launch-intro__edge--left" />
    <span class="launch-intro__edge launch-intro__edge--right" />

    <div class="launch-intro__brand">
      <div class="launch-intro__logo-window">
        <span
          class="launch-intro__logo-mark"
          :style="{ backgroundImage: `url(${compactLogo})` }"
        />
        <span class="launch-intro__light-pass" />
      </div>

      <span class="launch-intro__progress">
        <span class="launch-intro__progress-fill" />
      </span>

      <div class="launch-intro__signature">
        <span>HIPPO / EXCHANGE</span>
        <i />
        <span>EST. 2026</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
:global(html.launch-intro-active),
:global(html.launch-intro-active body) {
  overflow: hidden;
  overscroll-behavior: none;
}

.launch-intro {
  color: #f2f6f3;
  contain: layout paint style;
  inset: 0;
  overflow: hidden;
  pointer-events: auto;
  position: fixed;
  touch-action: none;
  z-index: var(--layer-launch);
}

.launch-intro__curtain {
  background: #080b0d;
  bottom: 0;
  position: absolute;
  top: 0;
  width: calc(50% + 1px);
  will-change: transform;
  z-index: 1;
}

.launch-intro__curtain::after {
  background: rgb(217 229 223 / 7%);
  content: "";
  height: 100%;
  opacity: .55;
  position: absolute;
  top: 0;
  width: 1px;
}

.launch-intro__curtain--left {
  left: 0;
}

.launch-intro__curtain--left::after {
  left: max(16px, calc(env(safe-area-inset-left) + 10px));
}

.launch-intro__curtain--right {
  right: 0;
}

.launch-intro__curtain--right::after {
  right: max(16px, calc(env(safe-area-inset-right) + 10px));
}

.launch-intro__edge {
  height: 42px;
  opacity: 0;
  position: absolute;
  top: calc(50% - 21px);
  transform: scaleY(.18);
  transform-origin: center;
  width: 1px;
  z-index: 3;
}

.launch-intro__edge--left {
  background: var(--signal-green);
  box-shadow: 0 0 16px rgb(73 247 176 / 24%);
  left: max(16px, calc(env(safe-area-inset-left) + 10px));
}

.launch-intro__edge--right {
  background: var(--signal-coral);
  box-shadow: 0 0 16px rgb(255 112 80 / 22%);
  right: max(16px, calc(env(safe-area-inset-right) + 10px));
}

.launch-intro__brand {
  align-items: center;
  display: flex;
  flex-direction: column;
  left: 50%;
  max-width: calc(
    100% - max(44px, calc(env(safe-area-inset-left) + env(safe-area-inset-right) + 32px))
  );
  position: absolute;
  top: 48%;
  transform: translate(-50%, -50%);
  width: 342px;
  z-index: 4;
}

.launch-intro__logo-window {
  clip-path: inset(0 50% 0 50%);
  height: clamp(62px, 20vw, 82px);
  overflow: hidden;
  position: relative;
  width: min(76vw, 310px);
}

.launch-intro__logo-mark {
  background-position: center;
  background-repeat: no-repeat;
  background-size: 100% auto;
  inset: 0;
  position: absolute;
  will-change: opacity, transform;
}

.launch-intro__light-pass {
  background: rgb(244 248 246 / 54%);
  bottom: 8px;
  box-shadow:
    -8px 0 22px rgb(71 200 255 / 24%),
    8px 0 26px rgb(73 247 176 / 18%);
  left: -18%;
  opacity: 0;
  position: absolute;
  top: 8px;
  transform: skewX(-12deg);
  visibility: hidden;
  width: 14%;
}

.launch-intro__progress {
  background: rgb(217 229 223 / 13%);
  display: block;
  height: 1px;
  margin-top: 15px;
  overflow: hidden;
  width: min(38vw, 156px);
}

.launch-intro__progress-fill {
  background: #d9e5df;
  display: block;
  height: 100%;
  transform: scaleX(0);
  transform-origin: left center;
  will-change: transform;
}

.launch-intro__signature {
  align-items: center;
  display: flex;
  font-family: var(--data-font);
  font-size: 8px;
  justify-content: center;
  margin-top: 12px;
  opacity: 0;
  transform: translateY(4px);
  visibility: hidden;
  white-space: nowrap;
}

.launch-intro__signature span:first-child {
  color: rgb(217 229 223 / 72%);
}

.launch-intro__signature span:last-child {
  color: rgb(217 229 223 / 42%);
}

.launch-intro__signature i {
  background: var(--signal-coral);
  display: block;
  height: 2px;
  margin: 0 8px;
  width: 2px;
}

@media (max-width: 340px) {
  .launch-intro__brand {
    max-width: calc(
      100% - max(36px, calc(env(safe-area-inset-left) + env(safe-area-inset-right) + 24px))
    );
  }

  .launch-intro__logo-window {
    width: min(78vw, 272px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .launch-intro {
    display: none;
  }
}
</style>
