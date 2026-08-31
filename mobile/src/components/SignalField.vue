<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = withDefaults(defineProps<{
  light?: boolean
}>(), {
  light: false,
})

const MAX_SIGNAL_DPR = 2
const MAX_SIGNAL_PIXELS = 2_200_000
const REDUCED_SIGNAL_TIMESTAMP = 1800
const SIGNAL_FRAME_INTERVAL_MS = 1000 / 30
const canvasRef = ref<HTMLCanvasElement | null>(null)
const staticMode = ref(false)
let restart: (() => void) | null = null
let cleanup: (() => void) | null = null

onMounted(() => {
  const canvas = canvasRef.value
  if (!canvas) return
  const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  const constrained = document.documentElement.dataset.performanceTier === 'constrained'
  let reduced = motionQuery.matches
  // Keep the semantic static state active even when constrained CSS hides the canvas before resize.
  staticMode.value = reduced || constrained
  const context = canvas.getContext('2d')
  if (!context) {
    staticMode.value = true
    return
  }
  let intersecting = true
  let animationId = 0
  let resizeId = 0
  let lastDrawTime = 0
  let width = 0
  let height = 0
  let ratio = 1
  let pointer = { x: 0.68, y: 0.34, active: false }
  const easedPointer = { x: pointer.x, y: pointer.y }
  const particles = Array.from({ length: 28 }, (_, index) => ({
    x: ((index * 73) % 101) / 100,
    y: ((index * 47 + 19) % 97) / 96,
    size: index % 6 === 0 ? 2.2 : index % 3 === 0 ? 1.4 : 0.8,
    speed: 0.000006 + (index % 7) * 0.0000014,
    phase: index * 0.73,
  }))

  const resize = (): boolean => {
    const rect = canvas.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) return false
    width = rect.width
    height = rect.height
    const pixelCapRatio = Math.sqrt(MAX_SIGNAL_PIXELS / Math.max(1, width * height))
    ratio = Math.min(window.devicePixelRatio || 1, MAX_SIGNAL_DPR, pixelCapRatio)
    canvas.width = Math.max(1, Math.floor(width * ratio))
    canvas.height = Math.max(1, Math.floor(height * ratio))
    context.setTransform(ratio, 0, 0, ratio, 0, 0)
    return true
  }

  const draw = (timestamp = 0): void => {
    context.clearRect(0, 0, width, height)
    const time = reduced ? REDUCED_SIGNAL_TIMESTAMP : timestamp
    const colors = props.light
      ? ['rgba(0, 126, 84, .46)', 'rgba(0, 104, 204, .34)', 'rgba(218, 62, 52, .24)']
      : ['rgba(84, 255, 181, .72)', 'rgba(55, 157, 255, .5)', 'rgba(255, 91, 75, .34)']
    const ink = props.light ? 'rgba(8, 34, 24, .16)' : 'rgba(227, 255, 241, .14)'

    easedPointer.x += (pointer.x - easedPointer.x) * 0.065
    easedPointer.y += (pointer.y - easedPointer.y) * 0.065

    context.strokeStyle = ink
    context.lineWidth = 0.6
    for (let x = 0; x <= width; x += 34) {
      context.beginPath()
      context.moveTo(x + 0.5, 0)
      context.lineTo(x + 0.5, height)
      context.stroke()
    }
    for (let y = 0; y <= height; y += 34) {
      context.beginPath()
      context.moveTo(0, y + 0.5)
      context.lineTo(width, y + 0.5)
      context.stroke()
    }

    for (let line = 0; line < 4; line += 1) {
      context.beginPath()
      context.lineWidth = line === 0 ? 2 : line === 3 ? 0.75 : 1.15
      context.strokeStyle = colors[line % colors.length]
      for (let x = -12; x <= width + 12; x += 4) {
        const normalizedX = x / Math.max(1, width)
        const pull = Math.max(0, 1 - Math.abs(normalizedX - easedPointer.x) * 3.6)
        const base = height * (0.28 + line * 0.145)
        const wave = Math.sin(
          x * (0.017 + line * 0.0035) + time * (0.00058 + line * 0.00008),
        ) * (13 + line * 3)
        const pulse = Math.sin(x * 0.063 - time * 0.0011 + line) * 4
        const pointerPull = pull * (easedPointer.y * height - base) * (0.3 - line * 0.035)
        const y = base + wave + pulse + pointerPull
        if (x === -12) context.moveTo(x, y)
        else context.lineTo(x, y)
      }
      context.stroke()
    }

    particles.forEach((particle, index) => {
      const drift = reduced ? 0 : (time * particle.speed + particle.phase) % 1
      const x = ((particle.x + drift) % 1) * width
      const y = (
        particle.y + Math.sin(time * 0.0004 + particle.phase) * 0.025
      ) * height
      const pointerDistance = Math.hypot(
        x / width - easedPointer.x,
        y / height - easedPointer.y,
      )
      const alpha = Math.min(0.62, 0.16 + Math.max(0, 0.32 - pointerDistance) * 0.9)
      context.fillStyle = index % 9 === 0
        ? colors[2]
        : index % 4 === 0
          ? colors[1]
          : colors[0]
      context.globalAlpha = alpha
      context.fillRect(x, y, particle.size * 5, particle.size)
      context.globalAlpha = 1
    })

    const scanY = reduced ? height * 0.42 : ((time * 0.035) % (height + 70)) - 35
    const scan = context.createLinearGradient(0, scanY - 30, 0, scanY + 30)
    scan.addColorStop(0, 'rgba(0,0,0,0)')
    scan.addColorStop(0.5, props.light ? 'rgba(0,126,84,.11)' : 'rgba(84,255,181,.15)')
    scan.addColorStop(1, 'rgba(0,0,0,0)')
    context.fillStyle = scan
    context.fillRect(0, scanY - 30, width, 60)

    context.strokeStyle = props.light
      ? 'rgba(0, 95, 65, .34)'
      : 'rgba(215, 255, 235, .36)'
    context.lineWidth = 1
    context.beginPath()
    context.arc(
      easedPointer.x * width,
      easedPointer.y * height,
      pointer.active ? 18 : 10,
      0,
      Math.PI * 2,
    )
    context.stroke()
  }

  const shouldAnimate = (): boolean => (
    !document.hidden && intersecting && !reduced && !constrained && width > 0 && height > 0
  )

  const animate = (timestamp: number): void => {
    if (!shouldAnimate()) return
    if (timestamp - lastDrawTime >= SIGNAL_FRAME_INTERVAL_MS) {
      draw(timestamp)
      lastDrawTime = timestamp
    }
    animationId = requestAnimationFrame(animate)
  }

  const start = (): void => {
    cancelAnimationFrame(animationId)
    animationId = 0
    staticMode.value = reduced || constrained
    if (document.hidden || !intersecting || width <= 0 || height <= 0 || staticMode.value) return
    const timestamp = performance.now()
    draw(timestamp)
    lastDrawTime = timestamp
    animationId = requestAnimationFrame(animate)
  }

  const onPointerMove = (event: PointerEvent): void => {
    const rect = canvas.getBoundingClientRect()
    if (
      event.clientX < rect.left
      || event.clientX > rect.right
      || event.clientY < rect.top
      || event.clientY > rect.bottom
    ) {
      pointer = { ...pointer, active: false }
      return
    }
    pointer = {
      x: Math.min(1, Math.max(0, (event.clientX - rect.left) / Math.max(1, rect.width))),
      y: Math.min(1, Math.max(0, (event.clientY - rect.top) / Math.max(1, rect.height))),
      active: true,
    }
  }

  const onVisibilityChange = (): void => {
    if (document.hidden) {
      cancelAnimationFrame(animationId)
      animationId = 0
      return
    }
    if (resize()) start()
  }

  const onMotionChange = (event: MediaQueryListEvent): void => {
    reduced = event.matches
    start()
  }

  const intersectionObserver = typeof IntersectionObserver === 'undefined'
    ? null
    : new IntersectionObserver((entries) => {
        intersecting = entries.some((entry) => entry.target === canvas && entry.isIntersecting)
        if (intersecting) {
          if (resize()) start()
        } else {
          cancelAnimationFrame(animationId)
          animationId = 0
        }
      })

  const onResize = (): void => {
    cancelAnimationFrame(resizeId)
    resizeId = requestAnimationFrame(() => {
      resizeId = 0
      if (resize()) start()
    })
  }

  if (resize()) start()
  restart = start
  window.addEventListener('resize', onResize)
  if (!constrained) {
    window.addEventListener('pointermove', onPointerMove, { passive: true })
    window.addEventListener('pointerdown', onPointerMove, { passive: true })
  }
  document.addEventListener('visibilitychange', onVisibilityChange)
  motionQuery.addEventListener('change', onMotionChange)
  intersectionObserver?.observe(canvas)

  cleanup = () => {
    cancelAnimationFrame(animationId)
    cancelAnimationFrame(resizeId)
    window.removeEventListener('resize', onResize)
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerdown', onPointerMove)
    document.removeEventListener('visibilitychange', onVisibilityChange)
    motionQuery.removeEventListener('change', onMotionChange)
    intersectionObserver?.disconnect()
    restart = null
  }
})

watch(() => props.light, () => restart?.())

onBeforeUnmount(() => {
  cleanup?.()
  cleanup = null
})
</script>

<template>
  <div class="signal-field-shell" :class="{ 'is-static': staticMode }" role="presentation">
    <span class="signal-static-fallback" aria-hidden="true" />
    <canvas ref="canvasRef" class="signal-field" aria-hidden="true" />
  </div>
</template>

<style scoped>
.signal-field-shell.is-static .signal-field {
  display: none;
}

.signal-field-shell.is-static .signal-static-fallback {
  opacity: .82;
}
</style>
