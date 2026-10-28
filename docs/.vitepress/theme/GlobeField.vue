<script setup lang="ts">
// The hero's centrepiece: a globe of code glyphs, rotating. Its characters
// flicker and bright bands rain down its face — a nod to falling code — and
// it spins faster while the page scrolls. Around it a tilted orbit carries the
// benchmark figures (decorative copies: the Measured section states them with
// their scope). Edges run from the globe's rim to the elements marked
// `data-globe-edge` in the same section — the hero's taglines — with a packet
// travelling each one, so the globe sits as the hub of a small graph.
//
// One canvas, redrawn per frame only while the hero is on screen. Glyphs dim
// behind any `data-globe-edge` text so the headline keeps its contrast. The
// orbit's labels are drawn last, as solid pills, so no glyph ever crosses
// their text; those on the far side are dimmed rather than hidden. Under
// prefers-reduced-motion or `paused`, a single still frame.

import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { pacer } from './motion'

const props = withDefaults(defineProps<{ paused?: boolean }>(), { paused: false })

const GLYPHS = '01{}<>[]()/=+*#:;$&|~0123456789ABCDEF'
const PINK = '205, 141, 189'
const LIGHT = '237, 223, 238'
const TILT = 0.32 // the globe leans toward the viewer, radians about x
const SPIN = 0.00018 // resting rotation, radians per ms
const ORBIT = { rx: 1.62, ry: 0.36, tilt: -0.22, rate: 0.55 } // radii in globe radii; rate × spin
// The orbit's labels: figures the README publishes, with their scope.
const FIGURES = [
  '14,932 edges labelled',
  '1 false positive in 11,221',
  '24.9% unverifiable',
  '0.907 HttpCall recall · Airflow',
  '0.744 OrmAccess recall · Airflow',
  'Calibration error 0.18',
]
const LABEL_FONT = '600 10.5px "Geist Mono Variable", ui-monospace, monospace'
const LABEL_H = 22 // a label's pill height, px
const FAR = 0.4 // a label on the orbit's far side: the strength of its outline and text

type Pt = { x: number; y: number; z: number; ch: string }
type Rect = { l: number; t: number; r: number; b: number }

const host = ref<HTMLDivElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)

let ctx: CanvasRenderingContext2D | null = null
let width = 0
let height = 0
let points: Pt[] = []
let angle = 0.6
let orbitAngle = 0
let boost = 0
let last = 0
let frame = 0
let visible = false
let reducedMotion = false
let dpr = 1
// Each label, pill and text together, is drawn once into a small canvas of its
// own and placed whole. Drawn afresh every frame, the text snapped to whole
// device pixels while its pill moved smoothly, so the words shook in the pill.
// Each has a near and a far version, both opaque: a translucent far label let
// the globe's flickering glyphs show through its text.
const labelSprites = new Map<string, HTMLCanvasElement | null>()
// The globe's glyphs are drawn once into a sheet, a cell for each glyph, size
// and weight, and stamped from it. Setting ~560 glyphs as text every frame
// cost more than everything else the globe draws.
const SIZES = [8, 9, 10, 11, 12, 13, 14] // px; a glyph's size follows its depth
const UNIQUE = [...new Set(GLYPHS)]
const GLYPH_INDEX = new Map(UNIQUE.map((ch, i) => [ch, i]))
let sheet: HTMLCanvasElement | null = null
let cell = 0 // a sheet cell's side, device px
// The tagline boxes the edges run to, measured on resize and when the fonts
// load rather than every frame.
let links: Array<{ box: Rect }> = []
const pace = pacer()
const cleanups: Array<() => void> = []

const still = () => reducedMotion || props.paused
const glyph = () => GLYPHS[Math.floor(Math.random() * GLYPHS.length)]

/** Points spread evenly over a sphere (a Fibonacci lattice), each holding a glyph. */
function populate(count: number) {
  const golden = Math.PI * (3 - Math.sqrt(5))
  points = Array.from({ length: count }, (_, i) => {
    const y = 1 - (i / (count - 1)) * 2
    const r = Math.sqrt(1 - y * y)
    const t = i * golden
    return { x: Math.cos(t) * r, y, z: Math.sin(t) * r, ch: GLYPHS[i % GLYPHS.length] }
  })
}

function resize() {
  const c = canvas.value
  const h = host.value
  if (!c || !h) return
  dpr = Math.min(window.devicePixelRatio || 1, 2)
  width = h.clientWidth
  height = h.clientHeight
  c.width = Math.round(width * dpr)
  c.height = Math.round(height * dpr)
  ctx = c.getContext('2d')
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0)
  labelSprites.clear()
  drawSheet()
  measureTargets()
  populate(width < 640 ? 300 : 560)
}

/** Draw every glyph, at every size, in both weights, into the sheet. */
function drawSheet() {
  const s = document.createElement('canvas')
  const g = s.getContext('2d')
  if (!g) {
    sheet = null
    return
  }
  cell = Math.ceil(SIZES[SIZES.length - 1] * 1.5 * dpr)
  s.width = cell * UNIQUE.length
  s.height = cell * SIZES.length * 2
  g.textAlign = 'center'
  g.textBaseline = 'middle'
  // Rows: the regular weight in pink at each size, then the bright weight in
  // light pink at each size.
  for (let bright = 0; bright < 2; bright++) {
    g.fillStyle = `rgb(${bright ? LIGHT : PINK})`
    SIZES.forEach((size, row) => {
      g.font = `${bright ? 600 : 500} ${size * dpr}px "Geist Mono Variable", ui-monospace, monospace`
      const y = (bright * SIZES.length + row) * cell + cell / 2
      UNIQUE.forEach((ch, i) => g.fillText(ch, i * cell + cell / 2, y))
    })
  }
  sheet = s
}

/** The tagline elements' boxes relative to the canvas, from layout, so the scroll's scale does not move them. */
function measureTargets() {
  const h = host.value
  const section = h?.offsetParent as HTMLElement | null
  if (!h || !section) {
    links = []
    return
  }
  links = []
  for (const el of section.querySelectorAll<HTMLElement>('[data-globe-edge]')) {
    let x = 0
    let y = 0
    let n: HTMLElement | null = el
    while (n && n !== section) {
      x += n.offsetLeft
      y += n.offsetTop
      n = n.offsetParent as HTMLElement | null
    }
    if (n !== section || el.offsetWidth === 0) continue
    const l = x - h.offsetLeft
    const t = y - h.offsetTop
    links.push({ box: { l, t, r: l + el.offsetWidth, b: t + el.offsetHeight } })
  }
}

/** A label's pill and text, near or far, drawn once at the canvas's pixel density. */
function labelSprite(text: string, far: boolean): HTMLCanvasElement | null {
  const key = `${far ? 'far' : 'near'}:${text}`
  if (labelSprites.has(key)) return labelSprites.get(key) ?? null
  const strength = far ? FAR : 1
  let sprite: HTMLCanvasElement | null = document.createElement('canvas')
  const g = sprite.getContext('2d')
  if (g) {
    g.font = LABEL_FONT
    const w = Math.ceil(g.measureText(text).width) + 20
    // A pixel's margin on every side holds the outline. Resizing a canvas
    // resets its context, so everything is set after.
    sprite.width = Math.ceil((w + 2) * dpr)
    sprite.height = Math.ceil((LABEL_H + 2) * dpr)
    g.setTransform(dpr, 0, 0, dpr, dpr, dpr)
    // An opaque fill: nothing drawn under the label shows through it.
    g.fillStyle = 'rgb(34, 34, 34)'
    g.strokeStyle = `rgba(${PINK}, ${0.7 * strength})`
    g.lineWidth = 1
    g.beginPath()
    g.roundRect(0, 0, w, LABEL_H, LABEL_H / 2)
    g.fill()
    g.stroke()
    g.font = LABEL_FONT
    g.fillStyle = `rgba(${LIGHT}, ${strength})`
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    g.fillText(text, w / 2, LABEL_H / 2 + 0.5)
  } else {
    sprite = null
  }
  labelSprites.set(key, sprite)
  return sprite
}

function draw(time: number) {
  if (!ctx) return
  const g = ctx
  g.clearRect(0, 0, width, height)

  const moving = !still()
  const dt = last ? Math.min(64, time - last) : 16
  last = time
  if (moving) {
    angle += SPIN * dt * (1 + boost)
    orbitAngle += SPIN * ORBIT.rate * dt * (1 + boost)
    boost *= 0.94
  }

  const portrait = width < height
  const R = portrait ? width * 0.3 : Math.min(width * 0.135, height * 0.25)
  const cx = width / 2
  const cy = height / 2
  const boxes = links.map((l) => l.box)
  const behindText = (x: number, y: number) => boxes.some((b) => x > b.l - 12 && x < b.r + 12 && y > b.t - 8 && y < b.b + 8)

  // The orbit, as an ellipse; its far half is drawn before the globe.
  const cosT = Math.cos(ORBIT.tilt)
  const sinT = Math.sin(ORBIT.tilt)
  const onOrbit = (th: number) => {
    const ex = Math.cos(th) * R * ORBIT.rx
    const ey = Math.sin(th) * R * ORBIT.ry
    return { x: cx + ex * cosT - ey * sinT, y: cy + ex * sinT + ey * cosT, front: Math.sin(th) > 0 }
  }
  const orbitArc = (from: number, to: number, alpha: number) => {
    g.save()
    g.translate(cx, cy)
    g.rotate(ORBIT.tilt)
    g.beginPath()
    g.ellipse(0, 0, R * ORBIT.rx, R * ORBIT.ry, 0, from, to)
    g.setLineDash([2, 7])
    g.strokeStyle = `rgba(${PINK}, ${alpha})`
    g.lineWidth = 1.2
    g.stroke()
    g.restore()
  }
  // A narrow screen carries four figures, not six, so they do not crowd.
  const figures = width < 640 ? FIGURES.filter((_, k) => k !== 2 && k !== 5) : FIGURES
  const labels = figures.map((text, k) => ({ text, ...onOrbit(orbitAngle + (k * Math.PI * 2) / figures.length) }))
  const drawLabel = (l: { text: string; x: number; y: number; front: boolean }) => {
    const sprite = labelSprite(l.text.toUpperCase(), !l.front)
    if (!sprite) return
    const w = sprite.width / dpr
    const h = sprite.height / dpr
    // Kept inside the canvas: at the orbit's ends a label slides inward.
    const x = Math.max(w / 2 + 7, Math.min(width - w / 2 - 7, l.x))
    // Placed on whole device pixels, so the label is copied, not resampled.
    const left = Math.round((x - w / 2) * dpr) / dpr
    const top = Math.round((l.y - h / 2) * dpr) / dpr
    g.drawImage(sprite, left, top, w, h)
  }
  orbitArc(Math.PI, Math.PI * 2, 0.22)

  // A faint wireframe gives the glyphs a definite sphere: the rim, and the
  // equator and two latitudes, which rotation about y leaves where they are.
  g.setLineDash([])
  g.lineWidth = 1
  g.strokeStyle = `rgba(${PINK}, 0.3)`
  g.beginPath()
  g.arc(cx, cy, R, 0, Math.PI * 2)
  g.stroke()
  for (const lat of [-Math.PI / 6, 0, Math.PI / 6]) {
    g.strokeStyle = `rgba(${PINK}, ${lat === 0 ? 0.22 : 0.14})`
    g.beginPath()
    g.ellipse(cx, cy - R * Math.sin(lat) * Math.cos(TILT), R * Math.cos(lat), R * Math.cos(lat) * Math.sin(TILT), 0, 0, Math.PI * 2)
    g.stroke()
  }

  // The globe: rotate each point about y, lean it about x, project.
  const ca = Math.cos(angle)
  const sa = Math.sin(angle)
  const cb = Math.cos(TILT)
  const sb = Math.sin(TILT)
  const band = (((time * 0.00022) % 2.8) - 1.4) // a bright band falling down the face
  // Each glyph is stamped from the sheet at its own opacity.
  const size = cell / dpr
  for (const p of points) {
    const x1 = p.x * ca + p.z * sa
    const z1 = -p.x * sa + p.z * ca
    const y2 = p.y * cb - z1 * sb
    const z2 = p.y * sb + z1 * cb
    const sx = cx + x1 * R
    const sy = cy + y2 * R
    const near = (z2 + 1) / 2 // 0 at the back, 1 facing us
    const rain = z2 > 0 ? Math.exp(-((y2 - band) ** 2) / 0.012) : 0
    if (moving && Math.random() < 0.006 + rain * 0.08) p.ch = glyph()
    let alpha = 0.1 + 0.62 * near ** 1.6 + rain * 0.7
    if (z2 < 0) alpha *= 0.55
    if (behindText(sx, sy)) alpha *= 0.22
    if (alpha < 0.04 || !sheet) continue
    const bright = rain > 0.45 ? 1 : 0
    const row = bright * SIZES.length + Math.round(6 * near) // sizes 8 to 14 px, by depth
    const column = GLYPH_INDEX.get(p.ch) ?? 0
    g.globalAlpha = Math.min(1, alpha)
    g.drawImage(sheet, column * cell, row * cell, cell, cell, sx - size / 2, sy - size / 2, size, size)
  }
  g.globalAlpha = 1

  // Edges from the globe's rim to each tagline, with a packet travelling out.
  links.forEach(({ box }, k) => {
    // Meet a block beside the globe at its inner edge; one below it, just above.
    const below = box.t > cy + R * 1.2
    const ax = box.r < cx ? box.r + 14 : box.l > cx ? box.l - 14 : (box.l + box.r) / 2
    const ay = below ? box.t - 10 : box.b < cy ? box.b - (box.b - box.t) * 0.3 : box.t + (box.b - box.t) * 0.5
    const dx = ax - cx
    const dy = ay - cy
    const len = Math.hypot(dx, dy) || 1
    const sx = cx + (dx / len) * R * 1.04
    const sy = cy + (dy / len) * R * 1.04
    const bend = 0.18 * (k % 2 ? 1 : -1)
    const qx = (sx + ax) / 2 - (dy / len) * len * bend
    const qy = (sy + ay) / 2 + (dx / len) * len * bend
    g.setLineDash([])
    g.strokeStyle = `rgba(${PINK}, 0.5)`
    g.lineWidth = 1.2
    g.beginPath()
    g.moveTo(sx, sy)
    g.quadraticCurveTo(qx, qy, ax, ay)
    g.stroke()
    g.fillStyle = `rgb(${PINK})`
    for (const [x, y, r] of [[sx, sy, 2.5], [ax, ay, 3.5]]) {
      g.beginPath()
      g.arc(x, y, r, 0, Math.PI * 2)
      g.fill()
    }
    if (moving) {
      const t = (time * 0.00032 + k * 0.29) % 1.6
      if (t < 1) {
        const u = 1 - t
        const px = u * u * sx + 2 * u * t * qx + t * t * ax
        const py = u * u * sy + 2 * u * t * qy + t * t * ay
        g.fillStyle = `rgb(${LIGHT})`
        g.beginPath()
        g.arc(px, py, 2.6, 0, Math.PI * 2)
        g.fill()
      }
    }
  })

  // The orbit's near half crosses in front of the globe. The labels come last,
  // over everything, the far ones first so the near ones cross over them; no
  // glyph, edge or packet is ever drawn across a label's text.
  orbitArc(0, Math.PI, 0.5)
  labels.filter((l) => !l.front).forEach(drawLabel)
  labels.filter((l) => l.front).forEach(drawLabel)
}

function loop(time: number) {
  if (pace.due(time)) {
    const start = performance.now()
    draw(time)
    pace.took(performance.now() - start)
  }
  frame = visible && !still() ? requestAnimationFrame(loop) : 0
}

/** Draw one still frame, or make sure the loop is running. */
function wake() {
  if (still() || !visible) {
    cancelAnimationFrame(frame)
    frame = 0
    last = 0
    draw(performance.now())
  } else if (!frame) {
    last = 0
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

  // On screen and not yet scrolled past (the sticky hero stays "visible"
  // while the content above it covers it).
  let inView = false
  const update = () => {
    const next = inView && window.scrollY < window.innerHeight * 1.3
    if (next !== visible) {
      visible = next
      wake()
    }
  }
  const io = new IntersectionObserver(([entry]) => {
    inView = entry.isIntersecting
    update()
  })
  io.observe(h)
  cleanups.push(() => io.disconnect())

  // Scrolling spins the globe faster; the boost decays once scrolling stops.
  let lastY = window.scrollY
  const onScroll = () => {
    const y = window.scrollY
    if (!still()) boost = Math.min(9, boost + Math.abs(y - lastY) * 0.05)
    lastY = y
    update()
  }
  window.addEventListener('scroll', onScroll, { passive: true })
  cleanups.push(() => window.removeEventListener('scroll', onScroll))

  // Redraw once the mono face has loaded, so the glyphs are the right shape
  // (and the labels are drawn in it), and measure the taglines in it.
  document.fonts?.ready.then(() => {
    labelSprites.clear()
    drawSheet()
    measureTargets()
    draw(performance.now())
  })
  draw(performance.now())
})

onBeforeUnmount(() => {
  cancelAnimationFrame(frame)
  cleanups.forEach((fn) => fn())
})
</script>

<template>
  <div ref="host" class="cg-globe" aria-hidden="true">
    <canvas ref="canvas" />
  </div>
</template>

<style scoped>
.cg-globe {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
}
</style>
