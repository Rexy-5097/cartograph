<script setup lang="ts">
// A decorative halftone field: a disc of dots, sparse at the centre, dense in
// a ring, fading at the rim, with a slow ripple travelling outward. The pointer
// probes it — dots near the cursor fall away and a crosshair follows.
//
// `field` draws the whole disc in the middle of its box; `dome` centres the
// disc on the top edge, so only its lower half shows.
//
// Content can sit in the field: the dots part around any element in the same
// section marked `data-clear`. The first time the field is seen, it blooms
// outward from its centre.
//
// At rest it keeps working. A field is swept like a radar screen: a beam
// turns about the disc's centre, and the dots it passes light up in the
// accent colour and fade. A dome sends out a sonar pulse instead, a ring of
// light spreading from its centre. While the pointer is away, the crosshair
// surveys the dots: it glides to a point, its brackets close in with a ping,
// and it moves on. Nothing new is ever drawn over content in a clearing.
//
// Purely presentational: aria-hidden, drawn only while on screen, and still
// (but still pointer-responsive) under prefers-reduced-motion or `paused`.

import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { pacer } from './motion'

const props = withDefaults(
  defineProps<{ variant?: 'field' | 'dome'; dot?: string; cross?: string; accent?: string; paused?: boolean }>(),
  { variant: 'field', dot: '#b7b39c', cross: '#222222', accent: '#cd8dbd', paused: false },
)

const TAU = Math.PI * 2
// The disc's shape, as fractions of its radius unless stated.
const SHAPE = {
  ringStart: 0.1, // density starts rising here...
  ringFull: 0.62, // ...and is full here
  rimStart: 0.88, // the fade at the edge starts here...
  rimEnd: 1.02, // ...and reaches nothing here
  cutoff: 0.07, // dots fainter than this are not drawn
  crossFrom: 0.58, // denser than this, a dot is drawn as a cross
  dotSize: 0.42, // a full-density dot, as a fraction of the grid step
}
// The ripple: how many waves fit across the radius, how fast they travel
// (radians per millisecond), and how deep they modulate the density.
const RIPPLE = { waves: 18, speed: 0.0011, depth: 0.2 }
// The pointer clears dots fully within `inner` px and not at all beyond `outer`.
const PROBE = { inner: 24, outer: 130 }
const CROSSHAIR = 8 // half-length of the crosshair arms, px
// Around a [data-clear] element, dots fade back in over this many px.
const CLEAR_FALLOFF = 36
// The first-sight bloom: how long the disc takes to grow to full size, ms.
const BLOOM_MS = 1600

// The field's sweep: ms per turn, and the angle (radians) its glow fades over.
const SWEEP = { period: 7000, trail: 1.5 }
// The dome's pulse: ms between pulses, how far one travels (in radii), the
// width of its leading edge and of the glow it leaves behind (in radii), and
// the share of its journey after which it fades: only once it is past the
// dense ring, which it would otherwise reach already faint.
const PULSE = { period: 3600, reach: 1.15, lead: 0.04, trail: 0.24, fadeFrom: 0.7 }
// A lit dot grows by up to this fraction, and is drawn in one of LIT_STEPS
// colours between `dot` and `accent`; below LIT_FROM it is not lit.
const LIT = { grow: 0.5, steps: 3, from: 0.12 }
// The idle survey, in ms and px. It keeps `clearance` px from content in a
// clearing; its ping and brackets reach at most `ping` px from its point, so
// they never touch that content.
const SURVEY = { glide: 1400, hop: 800, hold: 1900, near: 260, far: 0.25, clearance: 52, ping: 40 }
// While surveying, dots part around the crosshair, less widely than the pointer.
const SURVEY_PROBE = { inner: 14, outer: 72 }

type Rect = { x: number; y: number; w: number; h: number }
type Pt = { x: number; y: number }

const host = ref<HTMLDivElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)

let ctx: CanvasRenderingContext2D | null = null
let width = 0
let height = 0
let step = 11
let frame = 0
let visible = false
let reducedMotion = false
let clears: Rect[] = []
let bloomStart = -1
const pointer = { x: 0, y: 0, active: false }
// The surveyor: where the idle crosshair is, where it is going, and how.
const guide = { x: 0, y: 0, fromX: 0, fromY: 0, toX: 0, toY: 0, phase: 'none' as 'none' | 'glide' | 'hop' | 'hold', start: 0, alpha: 0 }
// The grid's dots that can ever be drawn (inside the disc, outside every
// clearing), with what does not change from frame to frame worked out once,
// on resize or when the clearings move: position, distance from the centre in
// full radii, the clearings' fade, the angle from the centre, and the resting
// density at full size.
let dotCount = 0
let dotX = new Float32Array(0)
let dotY = new Float32Array(0)
let dotR = new Float32Array(0)
let dotClear = new Float32Array(0)
let dotAngle = new Float32Array(0)
let dotRest = new Float32Array(0)
// One frame's dots, gathered before drawing so each colour is drawn in one
// pass: position, size, and `kind` (bit 0: a cross; the rest: its colour step).
let xs = new Float32Array(0)
let ys = new Float32Array(0)
let sizes = new Float32Array(0)
let kinds = new Uint8Array(0)
// The colour of each step from `dot` to `accent`, kept until either changes.
let palette = { key: '', colours: [] as string[] }
const pace = pacer()
const cleanups: Array<() => void> = []

const still = () => reducedMotion || props.paused

/** An element's layout box relative to `ancestor`, ignoring transforms. */
function layoutRect(el: HTMLElement, ancestor: HTMLElement): Rect | null {
  let x = 0
  let y = 0
  let n: HTMLElement | null = el
  while (n && n !== ancestor) {
    x += n.offsetLeft
    y += n.offsetTop
    n = n.offsetParent as HTMLElement | null
  }
  return n === ancestor ? { x, y, w: el.offsetWidth, h: el.offsetHeight } : null
}

/** Where the [data-clear] elements of this section sit, relative to the canvas. */
function measureClears() {
  const h = host.value
  const section = h?.offsetParent as HTMLElement | null
  if (!h || !section) return
  clears = [...section.querySelectorAll<HTMLElement>('[data-clear]')]
    .map((el) => layoutRect(el, section))
    .filter((r): r is Rect => r !== null)
    .map((r) => ({ ...r, x: r.x - h.offsetLeft, y: r.y - h.offsetTop }))
}

/** Distance from (x, y) to the nearest clear rect; 0 inside one. */
function clearDistance(x: number, y: number) {
  let best = Infinity
  for (const r of clears) {
    const dx = Math.max(r.x - x, 0, x - (r.x + r.w))
    const dy = Math.max(r.y - y, 0, y - (r.y + r.h))
    best = Math.min(best, Math.hypot(dx, dy))
  }
  return best
}

/** 0 before the field is first seen, easing to 1 as it blooms; 1 when still. */
function bloom(now: number) {
  if (still()) return 1
  if (bloomStart < 0) return 0
  const t = Math.min(1, (now - bloomStart) / BLOOM_MS)
  return 1 - Math.pow(1 - t, 3)
}

const smooth = (a: number, b: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)))
  return t * t * (3 - 2 * t)
}

/** A colour `t` of the way from hex colour `a` to hex colour `b`. */
function mix(a: string, b: string, t: number) {
  const pa = /^#([0-9a-f]{6})$/i.exec(a)
  const pb = /^#([0-9a-f]{6})$/i.exec(b)
  if (!pa || !pb) return t < 0.5 ? a : b
  const ca = parseInt(pa[1], 16)
  const cb = parseInt(pb[1], 16)
  const channel = (shift: number) => Math.round(((ca >> shift) & 255) * (1 - t) + ((cb >> shift) & 255) * t)
  return `rgb(${channel(16)}, ${channel(8)}, ${channel(0)})`
}

function resize() {
  const c = canvas.value
  const h = host.value
  if (!c || !h) return
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  width = h.clientWidth
  height = h.clientHeight
  step = width < 640 ? 9 : 11
  c.width = Math.round(width * dpr)
  c.height = Math.round(height * dpr)
  c.style.width = `${width}px`
  c.style.height = `${height}px`
  ctx = c.getContext('2d')
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0)
  measureClears()
  layoutDots()
  guide.phase = 'none'
}

/** The disc's centre and full radius for the current size and variant. */
function geometry() {
  const dome = props.variant === 'dome'
  // A dome may fill its box's height; a field keeps its disc a little inside.
  const full = dome ? Math.min(width * 0.42, height * 0.98) : Math.min(width, height * 1.25) * 0.47
  return { dome, cx: width / 2, cy: dome ? 0 : height / 2, full }
}

/** Find the dots that can ever be drawn, and work out what is fixed about each. */
function layoutDots() {
  const { cx, cy, full } = geometry()
  const capacity = (Math.ceil(width / step) + 1) * (Math.ceil(height / step) + 1)
  if (xs.length < capacity) {
    dotX = new Float32Array(capacity)
    dotY = new Float32Array(capacity)
    dotR = new Float32Array(capacity)
    dotClear = new Float32Array(capacity)
    dotAngle = new Float32Array(capacity)
    dotRest = new Float32Array(capacity)
    xs = new Float32Array(capacity)
    ys = new Float32Array(capacity)
    sizes = new Float32Array(capacity)
    kinds = new Uint8Array(capacity)
  }
  let n = 0
  for (let y = step / 2; y < height; y += step) {
    for (let x = step / 2; x < width; x += step) {
      // Beyond the rim at full size is beyond it while the disc blooms, too.
      const r = Math.hypot(x - cx, y - cy) / full
      if (r > SHAPE.rimEnd) continue
      const clear = clears.length ? smooth(0, CLEAR_FALLOFF, clearDistance(x, y)) : 1
      if (clear <= 0) continue
      dotX[n] = x
      dotY[n] = y
      dotR[n] = r
      dotClear[n] = clear
      dotAngle[n] = Math.atan2(y - cy, x - cx)
      dotRest[n] = shape(r) * clear
      n++
    }
  }
  dotCount = n
}

/** The disc's density at `r` full radii: sparse centre, dense ring, fading rim. */
const shape = (r: number) => smooth(SHAPE.ringStart, SHAPE.ringFull, r) * (1 - smooth(SHAPE.rimStart, SHAPE.rimEnd, r))

/** A point in the dense ring for the surveyor, clear of content; `from` asks for one near it. */
function waypoint(cx: number, cy: number, full: number, from?: Pt): Pt | null {
  for (let i = 0; i < 40; i++) {
    const a = Math.random() * TAU
    const r = full * (0.5 + Math.random() * 0.36)
    const x = cx + Math.cos(a) * r
    const y = cy + Math.sin(a) * r
    if (x < 24 || x > width - 24 || y < 24 || y > height - 24) continue
    if (clears.length && clearDistance(x, y) < SURVEY.clearance) continue
    if (from) {
      const d = Math.hypot(x - from.x, y - from.y)
      if (d < 50 || d > SURVEY.near || !pathClear(from, { x, y })) continue
    }
    return { x, y }
  }
  return null
}

/** Whether a glide from `a` to `b` keeps the crosshair off content. */
function pathClear(a: Pt, b: Pt) {
  if (!clears.length) return true
  for (let k = 1; k < 16; k++) {
    const t = k / 16
    if (clearDistance(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t) < CROSSHAIR + 22) return false
  }
  return true
}

/** Advances the surveyor: hold on a point, then glide to the next, or hop if content is in the way. */
function survey(time: number, cx: number, cy: number, full: number) {
  if (guide.phase === 'none') {
    const p = waypoint(cx, cy, full)
    if (p) Object.assign(guide, { x: p.x, y: p.y, phase: 'hold', start: time, alpha: 1 })
    return
  }
  const span = guide.phase === 'hold' ? SURVEY.hold : guide.phase === 'glide' ? SURVEY.glide : SURVEY.hop
  const t = (time - guide.start) / span
  if (t < 1) {
    if (guide.phase === 'glide') {
      const e = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2
      guide.x = guide.fromX + (guide.toX - guide.fromX) * e
      guide.y = guide.fromY + (guide.toY - guide.fromY) * e
    } else if (guide.phase === 'hop') {
      // Fades out where it is and in where it is going.
      guide.alpha = Math.abs(1 - 2 * t)
      guide.x = t < 0.5 ? guide.fromX : guide.toX
      guide.y = t < 0.5 ? guide.fromY : guide.toY
    }
    return
  }
  if (guide.phase !== 'hold') {
    Object.assign(guide, { x: guide.toX, y: guide.toY, alpha: 1, phase: 'hold', start: time })
    return
  }
  // Held long enough. Usually the next point is near; now and then it is
  // anywhere, so the survey reaches every part of the field.
  const here = { x: guide.x, y: guide.y }
  const near = Math.random() < SURVEY.far ? null : waypoint(cx, cy, full, here)
  const next = near ?? waypoint(cx, cy, full)
  if (!next) {
    guide.start = time
    return
  }
  Object.assign(guide, {
    fromX: here.x,
    fromY: here.y,
    toX: next.x,
    toY: next.y,
    phase: pathClear(here, next) ? 'glide' : 'hop',
    start: time,
  })
}

function draw(time: number) {
  if (!ctx) return
  const g = ctx
  g.clearRect(0, 0, width, height)

  const { dome, cx, cy, full } = geometry()
  const grown = bloom(performance.now())
  if (grown <= 0) return
  const moving = !still()
  // The surveyor works once the disc is grown, and only while the pointer is away.
  const surveying = moving && grown >= 1 && !pointer.active
  if (surveying) survey(time, cx, cy, full)

  // This frame's sweep beam, or this pulse's front (in radii) and its fade.
  const beam = (time / SWEEP.period) * TAU
  const pulse = (time % PULSE.period) / PULSE.period
  const front = pulse * PULSE.reach
  const fade = 1 - smooth(PULSE.fadeFrom, 1, pulse)

  let count = 0
  for (let i = 0; i < dotCount; i++) {
    const x = dotX[i]
    const y = dotY[i]
    // While the disc blooms, its radius grows to full and each dot's place in
    // the disc moves outward to its resting one.
    const r = dotR[i] / grown
    if (r > SHAPE.rimEnd) continue
    let v = grown >= 1 ? dotRest[i] : shape(r) * dotClear[i]
    if (moving) v *= 1 - RIPPLE.depth + RIPPLE.depth * Math.sin(r * RIPPLE.waves - time * RIPPLE.speed)
    if (pointer.active) v *= smooth(PROBE.inner, PROBE.outer, Math.hypot(x - pointer.x, y - pointer.y))
    else if (surveying && guide.alpha > 0) {
      const near = 1 - smooth(SURVEY_PROBE.inner, SURVEY_PROBE.outer, Math.hypot(x - guide.x, y - guide.y))
      v *= 1 - near * guide.alpha
    }
    if (v < SHAPE.cutoff) continue

    // Lit by the sweep or the pulse. Lighting scales a dot that is already
    // there, so where the dots have parted, nothing lights.
    let lit = 0
    if (moving) {
      if (dome) {
        const d = front - r // > 0 once the front has passed
        lit = (d >= 0 ? Math.max(0, 1 - d / PULSE.trail) : Math.max(0, 1 + d / PULSE.lead)) * fade
      } else {
        let lag = (beam - dotAngle[i]) % TAU
        if (lag < 0) lag += TAU
        lit = lag < SWEEP.trail ? (1 - lag / SWEEP.trail) ** 2 : 0
      }
      v = Math.min(1, v * (1 + LIT.grow * lit))
    }
    const shade = lit < LIT.from ? 0 : 1 + Math.min(LIT.steps - 1, Math.floor(((lit - LIT.from) / (1 - LIT.from)) * LIT.steps))
    xs[count] = x
    ys[count] = y
    sizes[count] = v * step * SHAPE.dotSize
    kinds[count] = (shade << 1) | (v > SHAPE.crossFrom ? 1 : 0)
    count++
  }

  // One pass per colour: squares filled, crosses stroked.
  const key = `${props.dot}|${props.accent}`
  if (palette.key !== key) {
    palette = { key, colours: Array.from({ length: LIT.steps + 1 }, (_, s) => (s ? mix(props.dot, props.accent, s / LIT.steps) : props.dot)) }
  }
  g.lineWidth = 1
  for (let shade = 0; shade <= LIT.steps; shade++) {
    const colour = palette.colours[shade]
    g.fillStyle = colour
    g.strokeStyle = colour
    g.beginPath()
    for (let i = 0; i < count; i++) {
      if (kinds[i] !== shade << 1) continue
      const s = sizes[i]
      g.rect(xs[i] - s / 2, ys[i] - s / 2, s, s)
    }
    g.fill()
    g.beginPath()
    for (let i = 0; i < count; i++) {
      if (kinds[i] !== ((shade << 1) | 1)) continue
      const s = sizes[i] / 2
      g.moveTo(xs[i] - s, ys[i] - s)
      g.lineTo(xs[i] + s, ys[i] + s)
      g.moveTo(xs[i] + s, ys[i] - s)
      g.lineTo(xs[i] - s, ys[i] + s)
    }
    g.stroke()
  }

  // Crosshair: on the pointer while it is over the field; while it is away,
  // with the surveyor; when still, at rest. Never over content in a clearing.
  if (pointer.active) crosshair(g, pointer.x, pointer.y, 1)
  else if (surveying) {
    crosshair(g, guide.x, guide.y, guide.alpha)
    reticle(g, time)
  } else crosshair(g, cx, dome ? Math.min(height * 0.42, full * 0.42) : cy, 1)
}

function crosshair(g: CanvasRenderingContext2D, x: number, y: number, alpha: number) {
  if (alpha <= 0 || (clears.length && clearDistance(x, y) < CROSSHAIR)) return
  g.globalAlpha = alpha
  g.strokeStyle = props.cross
  g.lineWidth = 1.5
  g.beginPath()
  g.moveTo(x - CROSSHAIR, y)
  g.lineTo(x + CROSSHAIR, y)
  g.moveTo(x, y - CROSSHAIR)
  g.lineTo(x, y + CROSSHAIR)
  g.stroke()
  g.globalAlpha = 1
}

/** While the surveyor holds a point, its brackets close in and a ping goes out. */
function reticle(g: CanvasRenderingContext2D, time: number) {
  if (guide.phase !== 'hold') return
  const held = time - guide.start
  const closing = 1 - Math.pow(1 - Math.min(1, held / 450), 3)
  // Fades out just before it moves on.
  const alpha = closing * Math.max(0, Math.min(1, (SURVEY.hold - held) / 300))
  if (alpha <= 0) return
  const d = 20 - 8 * closing // the brackets' corners, from the point
  const arm = 5
  g.globalAlpha = alpha
  g.strokeStyle = props.accent
  g.lineWidth = 1.5
  g.beginPath()
  for (const [sx, sy] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
    const bx = guide.x + sx * d
    const by = guide.y + sy * d
    g.moveTo(bx, by - sy * arm)
    g.lineTo(bx, by)
    g.lineTo(bx - sx * arm, by)
  }
  g.stroke()
  const ping = held / 900
  if (ping < 1) {
    g.globalAlpha = (1 - ping) * 0.7
    g.beginPath()
    g.arc(guide.x, guide.y, 8 + (SURVEY.ping - 8) * ping, 0, TAU)
    g.stroke()
  }
  g.globalAlpha = 1
}

function loop(time: number) {
  if (pace.due(time)) {
    const start = performance.now()
    draw(time)
    pace.took(performance.now() - start)
  }
  frame = visible && !still() ? requestAnimationFrame(loop) : 0
}

/** The field has come into view: start its bloom, once. */
function seen() {
  if (bloomStart < 0) bloomStart = performance.now()
}

/** Redraw once if still; otherwise make sure the animation loop is running. */
function wake() {
  if (still()) {
    cancelAnimationFrame(frame)
    frame = 0
    draw(0)
  } else if (!frame && visible) {
    frame = requestAnimationFrame(loop)
  }
}

watch(() => props.paused, wake)

onMounted(() => {
  const h = host.value
  if (!h) return
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)')
  reducedMotion = motion.matches
  const onMotion = () => {
    reducedMotion = motion.matches
    wake()
  }
  motion.addEventListener('change', onMotion)
  cleanups.push(() => motion.removeEventListener('change', onMotion))

  resize()
  const ro = new ResizeObserver(() => {
    resize()
    draw(performance.now())
  })
  ro.observe(h)
  cleanups.push(() => ro.disconnect())

  const io = new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting
    if (visible) seen()
    wake()
  })
  io.observe(h)
  cleanups.push(() => io.disconnect())

  // Content in a clearing can change size (fonts load, text wraps); remeasure.
  const section = h.offsetParent as HTMLElement | null
  const clearing = new ResizeObserver(() => {
    measureClears()
    layoutDots()
    guide.phase = 'none'
    if (still()) draw(0)
  })
  section?.querySelectorAll('[data-clear]').forEach((el) => clearing.observe(el))
  cleanups.push(() => clearing.disconnect())

  const move = (e: PointerEvent) => {
    const box = h.getBoundingClientRect()
    pointer.x = e.clientX - box.left
    pointer.y = e.clientY - box.top
    pointer.active = true
    if (still()) draw(0)
  }
  const leave = () => {
    pointer.active = false
    // The survey carries on from where the pointer left, if that is clear of content.
    const { x, y } = pointer
    if (x > 24 && x < width - 24 && y > 24 && y < height - 24 && clearDistance(x, y) >= SURVEY.clearance) {
      Object.assign(guide, { x, y, alpha: 1, phase: 'hold', start: performance.now() - SURVEY.hold * 0.6 })
    }
    if (still()) draw(0)
  }
  h.addEventListener('pointermove', move)
  h.addEventListener('pointerleave', leave)
  cleanups.push(() => {
    h.removeEventListener('pointermove', move)
    h.removeEventListener('pointerleave', leave)
  })

  draw(0)
})

onBeforeUnmount(() => {
  cancelAnimationFrame(frame)
  cleanups.forEach((fn) => fn())
})
</script>

<template>
  <div ref="host" class="cg-halftone" aria-hidden="true">
    <canvas ref="canvas" />
  </div>
</template>

<style scoped>
.cg-halftone {
  position: relative;
  width: 100%;
  height: 100%;
  touch-action: pan-y;
}

canvas {
  position: absolute;
  inset: 0;
  display: block;
}
</style>
