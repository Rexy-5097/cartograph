<script setup lang="ts">
// The M08 labelled dataset, one dot per edge: 14,932 edges labelled across
// seven pinned repositories. They fill in a sweep when the field comes into
// view; one is marked as the single false positive among the 11,221 that
// could be verified; then 3,711 — the 24.9% that could not be verified from
// source — go hollow, gathered toward one corner because they concentrate
// where evidence is weakest. Their exact positions are illustrative; the
// counts are the report's.
//
// The canvas animates only during that sweep, then holds still. The pulse on
// the false positive is CSS: with `loop` (the landing page, which has a
// pause control) it keeps pulsing while on screen; elsewhere it pulses twice.
// Under prefers-reduced-motion, or with `paused`, the finished field is drawn
// at once.

import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { easeOut, prefersReduced, span, useInView } from './shared'

const props = withDefaults(defineProps<{ loop?: boolean; paused?: boolean; aspect?: number }>(), { loop: false, paused: false, aspect: 16 / 9 })

const EDGES = 14932
const UNVERIFIED = 3711

const root = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const { seen, visible } = useInView(root)
const count = ref(EDGES)
const fp = ref({ x: 0, y: 0, r: 3 })
const hollowShown = ref(true)
const marked = ref(true)

// A deterministic generator, so every visitor sees the same field.
function rng(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0
    return s / 4294967296
  }
}

let layout: { cols: number; rows: number; step: number; w: number; h: number; order: Int32Array; rank: Int32Array; hollow: Uint8Array; hollowRank: Float32Array; fpIndex: number } | null = null

function plan(w: number) {
  const h = w / props.aspect
  const cols = Math.ceil(Math.sqrt((EDGES * w) / h))
  const step = w / cols
  const rows = Math.ceil(EDGES / cols)
  const r = rng(5097)
  const key = new Float32Array(EDGES)
  const weak = new Float32Array(EDGES)
  for (let i = 0; i < EDGES; i++) {
    const c = (i % cols) / cols
    const rr = Math.floor(i / cols) / rows
    key[i] = c + rr + r() * 0.25
    weak[i] = Math.hypot(c - 0.92, rr - 0.9) + r() * 0.35
  }
  const ids = Array.from({ length: EDGES }, (_, i) => i)
  const order = Int32Array.from([...ids].sort((a, b) => key[a] - key[b]))
  const rank = new Int32Array(EDGES)
  order.forEach((id, n) => (rank[id] = n))
  const hollow = new Uint8Array(EDGES)
  ;[...ids].sort((a, b) => weak[a] - weak[b]).slice(0, UNVERIFIED).forEach((id) => (hollow[id] = 1))
  const hollowRank = new Float32Array(EDGES)
  for (let i = 0; i < EDGES; i++) hollowRank[i] = r()
  const fpIndex = Math.floor(rows * 0.31) * cols + Math.floor(cols * 0.38)
  layout = { cols, rows, step, w, h: rows * step, order, rank, hollow, hollowRank, fpIndex }
  fp.value = { x: ((fpIndex % cols) + 0.5) * step, y: (Math.floor(fpIndex / cols) + 0.5) * step, r: step * 0.36 }
}

/** Draw the field with `placed` dots, and hollowness `hv` (0–1) applied. */
function draw(placed: number, hv: number) {
  const c = canvas.value
  const L = layout
  if (!c || !L) return
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  const W = Math.round(L.w * dpr)
  const H = Math.round(L.h * dpr)
  if (c.width !== W || c.height !== H) {
    c.width = W
    c.height = H
  }
  const g = c.getContext('2d')
  if (!g) return
  const style = getComputedStyle(root.value!)
  const ink = style.color
  const dim = style.getPropertyValue('--ef-dim').trim() || 'rgba(128,128,128,0.6)'
  g.setTransform(dpr, 0, 0, dpr, 0, 0)
  g.clearRect(0, 0, L.w, L.h)
  const rad = L.step * 0.34
  g.fillStyle = ink
  g.beginPath()
  const hollowOnes: number[] = []
  for (let i = 0; i < EDGES; i++) {
    if (L.rank[i] >= placed || i === L.fpIndex) continue
    if (L.hollow[i] && L.hollowRank[i] < hv) {
      hollowOnes.push(i)
      continue
    }
    const x = ((i % L.cols) + 0.5) * L.step
    const y = (Math.floor(i / L.cols) + 0.5) * L.step
    g.moveTo(x + rad, y)
    g.arc(x, y, rad, 0, Math.PI * 2)
  }
  g.fill()
  if (hollowOnes.length) {
    g.strokeStyle = dim
    g.lineWidth = Math.max(0.6, L.step * 0.09)
    g.beginPath()
    for (const i of hollowOnes) {
      const x = ((i % L.cols) + 0.5) * L.step
      const y = (Math.floor(i / L.cols) + 0.5) * L.step
      g.moveTo(x + rad * 0.85, y)
      g.arc(x, y, rad * 0.85, 0, Math.PI * 2)
    }
    g.stroke()
  }
}

let frame = 0
let ro: ResizeObserver | null = null
let mo: MutationObserver | null = null
let state = { placed: EDGES, hv: 1 }

function finish() {
  cancelAnimationFrame(frame)
  state = { placed: EDGES, hv: 1 }
  count.value = EDGES
  hollowShown.value = true
  marked.value = true
  draw(EDGES, 1)
}

function play() {
  const start = performance.now()
  const FILL = 2200
  const PAUSE = 500
  const HOLLOW = 900
  marked.value = false
  hollowShown.value = false
  const step = (now: number) => {
    const t = now - start
    const placed = Math.round(EDGES * easeOut(span(t, 0, FILL)))
    const hv = span(t, FILL + PAUSE, FILL + PAUSE + HOLLOW)
    state = { placed, hv }
    count.value = placed
    if (t > FILL) marked.value = true
    if (hv > 0.5) hollowShown.value = true
    draw(placed, hv)
    if (t < FILL + PAUSE + HOLLOW) frame = requestAnimationFrame(step)
  }
  frame = requestAnimationFrame(step)
}

onMounted(() => {
  const el = root.value
  if (!el) return
  plan(el.clientWidth)
  const still = prefersReduced() || props.paused
  if (still) finish()
  else {
    state = { placed: 0, hv: 0 }
    count.value = 0
    marked.value = false
    hollowShown.value = false
    draw(0, 0)
  }
  ro = new ResizeObserver(() => {
    if (Math.abs(el.clientWidth - (layout?.w ?? 0)) < 1) return
    plan(el.clientWidth)
    draw(state.placed, state.hv)
  })
  ro.observe(el)
  // The docs pages switch between light and dark by a class on <html>.
  mo = new MutationObserver(() => draw(state.placed, state.hv))
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})

watch(seen, (s) => {
  if (!s || state.placed === EDGES) return
  if (prefersReduced() || props.paused) finish()
  else play()
})
watch(
  () => props.paused,
  (p) => {
    if (p && state.placed < EDGES) finish()
  },
)

onBeforeUnmount(() => {
  cancelAnimationFrame(frame)
  ro?.disconnect()
  mo?.disconnect()
})
</script>

<template>
  <figure ref="root" class="ef" :class="{ 'is-pulsing': marked && visible && !paused, 'is-looping': loop }">
    <div class="ef-field">
      <canvas ref="canvas" class="ef-canvas" role="img" :aria-label="`${EDGES.toLocaleString('en-GB')} dots, one per labelled edge; one marked as the false positive; ${UNVERIFIED.toLocaleString('en-GB')} hollow, not verifiable from source`" />
      <span class="ef-fp" :class="{ 'is-on': marked }" :style="{ left: `${fp.x}px`, top: `${fp.y}px` }" aria-hidden="true">
        <i />
        <b>1 false positive</b>
      </span>
    </div>
    <figcaption class="ef-legend">
      <span><i class="ef-key ef-key--dot" /><strong>{{ count.toLocaleString('en-GB') }}</strong> edges labelled</span>
      <span><i class="ef-key ef-key--fp" />1 false positive among 11,221 verified</span>
      <span :class="{ 'is-dim': !hollowShown }"><i class="ef-key ef-key--hollow" />3,711 (24.9%) not verifiable from source</span>
      <small>M08, seven pinned repositories. Every figure is an upper bound. Positions are illustrative; counts are the report's.</small>
    </figcaption>
  </figure>
</template>

<style scoped>
.ef {
  /* A host can fix the accent with --graphic-accent; otherwise it follows the theme. */
  --ef-accent: var(--graphic-accent, var(--cg-pink-deep, #8a4a79));
  --ef-dim: color-mix(in srgb, currentColor 45%, transparent);
  margin: 2rem 0;
}

:global(.dark) .ef {
  --ef-accent: var(--graphic-accent, var(--cg-pink, #cd8dbd));
}

.ef-field {
  position: relative;
}

.ef-canvas {
  display: block;
  width: 100%;
  height: auto;
}

/* The false positive, over the canvas: a dot that swells, a ring, a label. */
.ef-fp {
  position: absolute;
  width: 0;
  height: 0;
  opacity: 0;
  transition: opacity 0.4s var(--cg-ease, ease);
}

.ef-fp.is-on {
  opacity: 1;
}

.ef-fp i {
  position: absolute;
  left: -6px;
  top: -6px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--ef-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--ef-accent) 25%, transparent);
}

.ef-fp i::after {
  content: '';
  position: absolute;
  inset: -2px;
  border: 2px solid var(--ef-accent);
  border-radius: 50%;
  opacity: 0;
}

.ef.is-pulsing .ef-fp i::after {
  animation: ef-ring 1.6s var(--cg-ease, ease) 2;
}

.ef.is-pulsing.is-looping .ef-fp i::after {
  animation-iteration-count: infinite;
}

.ef-fp b {
  position: absolute;
  left: 14px;
  top: -34px;
  padding: 0.3rem 0.65rem;
  border-radius: 999px;
  font-family: var(--cg-font-mono, monospace);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
  color: #fff;
  background: var(--ef-accent);
}

@keyframes ef-ring {
  from {
    transform: scale(1);
    opacity: 0.9;
  }
  to {
    transform: scale(4.5);
    opacity: 0;
  }
}

.ef-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 0.6rem 1.5rem;
  margin-top: 1rem;
  font-size: 13px;
  line-height: 1.4;
}

.ef-legend span {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  transition: opacity 0.5s var(--cg-ease, ease);
}

.ef-legend span.is-dim {
  opacity: 0.35;
}

.ef-legend strong {
  font-variant-numeric: tabular-nums;
}

.ef-legend small {
  flex-basis: 100%;
  font-size: 12px;
  color: color-mix(in srgb, currentColor 62%, transparent);
}

.ef-key {
  display: inline-block;
  width: 9px;
  height: 9px;
  border-radius: 50%;
}

.ef-key--dot {
  background: currentColor;
}

.ef-key--fp {
  background: var(--ef-accent);
}

.ef-key--hollow {
  border: 1.5px solid var(--ef-dim);
}

@media (prefers-reduced-motion: reduce) {
  .ef-fp i::after {
    animation: none !important;
  }
}
</style>
