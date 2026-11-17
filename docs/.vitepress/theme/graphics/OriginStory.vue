<script setup lang="ts">
// "From one file to three languages": the opening of the film, as a stage the
// reader scrolls through rather than a video. A 90s monitor types a one-file
// program; the same screen shows a page and a server, then a web of services;
// the set switches off, and today opens on one request crossing three
// languages — the problem the rest of the page answers.
//
// Scrolling drives it, so it moves only as fast as the reader does. The small
// motions that run on their own (the cursor, the screen's flicker and rolling
// bar, the packet on the request) stop with "Pause motion" (`paused`) and off
// screen. Under prefers-reduced-motion, and before JavaScript runs, there is
// no stage at all: the four moments sit side by side, finished.
//
// The industry history is broad strokes and claims nothing about Cartograph.
// The request in "Today" is the README's illustrative example, and says so.

import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { clamp01, easeInOut, easeOut, prefersReduced, span, useInView } from './shared'

const props = withDefaults(defineProps<{ paused?: boolean }>(), { paused: false })

const root = ref<HTMLElement | null>(null)
const track = ref<HTMLElement | null>(null)
const scrolly = ref(false)
const p = ref(1)
const { visible } = useInView(root)
const live = computed(() => scrolly.value && visible.value && !props.paused)

// ─── Scroll progress through the track, eased like the page's other scenes ─
let raf = 0
let last = 0
let target = 0
let shown = 0
const tick = (now: number) => {
  raf = 0
  const t = track.value
  if (!t) return
  const r = t.getBoundingClientRect()
  const run = r.height - window.innerHeight
  target = run > 0 ? clamp01(-r.top / run) : 0
  const k = last ? 1 - Math.exp(-Math.min(64, now - last) / 90) : 1
  last = now
  shown += (target - shown) * k
  if (Math.abs(target - shown) < 0.0004) {
    shown = target
    last = 0
  } else raf = requestAnimationFrame(tick)
  p.value = shown
}
const request = () => {
  if (!raf) raf = requestAnimationFrame(tick)
}
onMounted(() => {
  if (prefersReduced()) return
  scrolly.value = true
  p.value = 0
  window.addEventListener('scroll', request, { passive: true })
  window.addEventListener('resize', request)
  requestAnimationFrame(() => tick(performance.now()))
})
onBeforeUnmount(() => {
  cancelAnimationFrame(raf)
  window.removeEventListener('scroll', request)
  window.removeEventListener('resize', request)
})

// ─── The four moments, as windows of progress ──────────────────────────────
const ERAS = [
  { key: '1994', at: 0 },
  { key: '2004', at: 0.3 },
  { key: '2014', at: 0.5 },
  { key: 'Today', at: 0.78 },
]
const W = { a: [0, 0.3], b: [0.3, 0.5], c: [0.5, 0.775], d: [0.78, 1.01] } as const
/** How present a panel is: fades in and out at its window's edges. */
const presence = (w: readonly [number, number]) => (scrolly.value ? span(p.value, w[0] - 0.02, w[0] + 0.015) * (1 - span(p.value, w[1] - 0.015, w[1])) : 1)
const era = computed(() => ERAS.reduce((n, e, i) => (p.value >= e.at ? i : n), 0))
const at = (a: number, b: number) => (scrolly.value ? span(p.value, a, b) : 1)

// 1994: a session typed out as the reader scrolls.
const SESSION = [
  { text: 'C:\\> edit hello.c', prompt: true },
  { text: '#include <stdio.h>' },
  { text: '' },
  { text: 'int main(void) {' },
  { text: '    printf("hello, world\\n");' },
  { text: '    return 0;' },
  { text: '}' },
  { text: '' },
  { text: 'C:\\> cc hello.c && hello', prompt: true },
]
const NL = '\n'
// Every line's characters and its newline.
const TOTAL = SESSION.reduce((n, l) => n + l.text.length + 1, 0)
const typed = computed(() => Math.round(at(0.02, 0.21) * TOTAL))
/** The session as far as it has been typed, one string, newlines included. */
const termText = computed(() => {
  let left = typed.value
  const out: string[] = []
  for (const l of SESSION) {
    if (left < 0) break
    out.push(l.text.slice(0, Math.max(0, left)))
    left -= l.text.length + 1
  }
  return out.join(NL)
})
const greeted = computed(() => at(0.215, 0.225) >= 1)

// 2004 and 2014: systems drawn in phosphor.
type Box = { id: string; x: number; y: number; label: string }
const ERA2: Box[] = [
  { id: 'page', x: 130, y: 150, label: 'index.html' },
  { id: 'server', x: 390, y: 150, label: 'server' },
]
const ERA3: Box[] = [
  { id: 'web', x: 70, y: 70, label: 'web' },
  { id: 'mobile', x: 70, y: 230, label: 'mobile' },
  { id: 'api', x: 210, y: 150, label: 'api' },
  { id: 'auth', x: 350, y: 52, label: 'auth' },
  { id: 'orders', x: 370, y: 150, label: 'orders' },
  { id: 'queue', x: 350, y: 250, label: 'queue' },
  { id: 'db', x: 470, y: 90, label: 'db' },
  { id: 'worker', x: 470, y: 215, label: 'worker' },
]
const WIRES3: Array<[string, string]> = [
  ['web', 'api'],
  ['mobile', 'api'],
  ['api', 'auth'],
  ['api', 'orders'],
  ['api', 'queue'],
  ['orders', 'db'],
  ['auth', 'db'],
  ['queue', 'worker'],
  ['worker', 'db'],
]
const byId = (boxes: Box[]) => Object.fromEntries(boxes.map((b) => [b.id, b]))
const B2 = byId(ERA2)
const B3 = byId(ERA3)
const draw2 = computed(() => at(0.315, 0.4))
const draw3 = computed(() => at(0.515, 0.64))
const boxW = (b: Box) => Math.max(76, b.label.length * 10.5 + 30)

// The switch-off at the end of 2014.
const squash = computed(() => (scrolly.value ? easeInOut(span(p.value, 0.705, 0.74)) : 0))
const dot = computed(() => (scrolly.value ? easeInOut(span(p.value, 0.74, 0.772)) : 0))

// Today: one request across three languages (the README's illustrative example).
const open = computed(() => (scrolly.value ? easeOut(span(p.value, 0.78, 0.83)) : 1))
const reach = computed(() => at(0.82, 0.9))
const verdict = computed(() => at(0.9, 0.93))
const HOPS = [
  { lang: 'TypeScript', file: 'CheckoutButton.tsx', sym: 'onSubmit()' },
  { lang: 'HTTP', file: '', sym: 'POST /api/orders' },
  { lang: 'Python', file: 'orders.py', sym: 'create_order()' },
  { lang: 'SQL', file: '', sym: 'orders' },
]
</script>

<template>
  <section ref="root" class="og" :class="{ 'is-scrolly': scrolly, 'is-live': live }" aria-label="From one file to three languages">
    <div ref="track" class="og-track">
      <div class="og-stage">
        <!-- Where the reader is: four marks down the side. -->
        <ol v-if="scrolly" class="og-rail" aria-hidden="true">
          <li v-for="(e, i) in ERAS" :key="e.key" :class="{ 'is-on': i === era, 'is-past': i < era }">{{ e.key }}</li>
        </ol>

        <!-- 1994 -->
        <div class="og-panel" :style="{ opacity: presence(W.a) }" :aria-hidden="scrolly && presence(W.a) < 0.5">
          <div class="og-caption">
            <p class="og-year">1994</p>
            <p class="og-line">One file. One language.</p>
            <p class="og-sub" :style="{ opacity: at(0.12, 0.17) }">You could read the whole program.</p>
          </div>
          <div class="og-screen og-crt">
            <pre class="og-term">{{ termText }}<template v-if="greeted">{{ NL }}<span class="og-term-out">hello, world</span>{{ NL }}</template><span class="og-cursor" aria-hidden="true">█</span></pre>
          </div>
        </div>

        <!-- 2004 -->
        <div class="og-panel" :style="{ opacity: presence(W.b) }" :aria-hidden="!scrolly ? undefined : presence(W.b) < 0.5">
          <div class="og-caption">
            <p class="og-year">2004</p>
            <p class="og-line">A page and a server.</p>
          </div>
          <div class="og-screen og-crt og-crt--cool">
            <svg class="og-diagram" viewBox="0 0 520 300" role="img" aria-label="A page talking to a server over HTTP">
              <line :x1="B2.page.x + 60" y1="150" :x2="B2.page.x + 60 + (B2.server.x - 46 - B2.page.x - 60) * draw2" y2="150" class="og-wire" />
              <text x="262" y="132" text-anchor="middle" class="og-wire-label" :style="{ opacity: draw2 }">HTTP</text>
              <g v-for="(b, i) in ERA2" :key="b.id" :style="{ opacity: easeOut(draw2 * 2 - i * 0.6) }">
                <rect :x="b.x - boxW(b) / 2" :y="b.y - 22" :width="boxW(b)" height="44" class="og-box" />
                <text :x="b.x" :y="b.y + 6" text-anchor="middle" class="og-box-label">{{ b.label }}</text>
              </g>
            </svg>
          </div>
        </div>

        <!-- 2014, then the set switches off -->
        <div class="og-panel" :style="{ opacity: scrolly ? span(p, W.c[0] - 0.02, W.c[0] + 0.015) * (1 - span(p, 0.77, 0.78)) : 1 }" :aria-hidden="!scrolly ? undefined : presence(W.c) < 0.5">
          <div class="og-caption" :style="{ opacity: 1 - squash }">
            <p class="og-year">2014</p>
            <p class="og-line">Then services, APIs, frameworks.</p>
            <p class="og-sub" :style="{ opacity: at(0.6, 0.65) }">More than one language in one system.</p>
          </div>
          <div class="og-screen og-crt og-crt--cool" :style="{ transform: `scale(${1 - 0.985 * dot}, ${1 - 0.985 * squash})`, filter: squash > 0 ? `brightness(${1 + 1.5 * squash})` : undefined }">
            <svg class="og-diagram" viewBox="0 0 540 300" role="img" aria-label="A web of services: web, mobile, an API, auth, orders, a queue, a worker and a database">
              <line
                v-for="([a, b], i) in WIRES3"
                :key="a + b"
                :x1="B3[a].x"
                :y1="B3[a].y"
                :x2="B3[a].x + (B3[b].x - B3[a].x) * clamp01(draw3 * 1.8 - i * 0.09)"
                :y2="B3[a].y + (B3[b].y - B3[a].y) * clamp01(draw3 * 1.8 - i * 0.09)"
                class="og-wire"
              />
              <g v-for="(b, i) in ERA3" :key="b.id" :style="{ opacity: easeOut(draw3 * 2.4 - i * 0.18) }">
                <rect :x="b.x - boxW(b) / 2" :y="b.y - 20" :width="boxW(b)" height="40" class="og-box" />
                <text :x="b.x" :y="b.y + 6" text-anchor="middle" class="og-box-label">{{ b.label }}</text>
              </g>
            </svg>
          </div>
        </div>

        <!-- Today -->
        <div class="og-panel og-panel--today" :style="{ opacity: presence(W.d) }" :aria-hidden="!scrolly ? undefined : presence(W.d) < 0.5">
          <div class="og-caption">
            <p class="og-year">Today</p>
            <p class="og-line">One request crosses three languages.</p>
            <p class="og-sub" :style="{ opacity: verdict }">
              An import graph tells you which files import which. It doesn’t tell you what a request
              <em>reaches</em>.
            </p>
          </div>
          <div class="og-screen og-today" :style="{ clipPath: `inset(${(1 - open) * 50}% 0 ${(1 - open) * 50}% 0)` }">
            <ol class="og-hops">
              <li v-for="(h, i) in HOPS" :key="h.lang" :class="{ 'is-lit': reach * 4 > i }" :style="{ '--i': i }">
                <span class="og-hop-lang">{{ h.lang }}</span>
                <span class="og-hop-sym">{{ h.sym }}</span>
                <span v-if="h.file" class="og-hop-file">{{ h.file }}</span>
              </li>
            </ol>
            <div class="og-path" aria-hidden="true">
              <i class="og-path-line" :style="{ transform: `scaleX(${reach})` }" />
              <i v-if="reach >= 1" class="og-path-packet" />
            </div>
            <p class="og-note">Illustrative example, from the README</p>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.og {
  --og-phosphor: #86f7a0;
  --og-cool: #cfe6ff;
  position: relative;
  z-index: 1;
  color: var(--cg-light-pink);
  background: var(--cg-black);
  padding: 0 var(--cg-gutter) clamp(4rem, 2.5rem + 3.5vw, 7rem);
}

/* ---------- Without the stage: four finished moments ---------- */
.og-stage {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 3rem 2.5rem;
}

.og-panel {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

/* ---------- With the stage: pinned, one moment at a time ---------- */
.og.is-scrolly {
  padding-bottom: 0;
}

.og.is-scrolly .og-track {
  height: 440vh;
}

.og.is-scrolly .og-stage {
  position: sticky;
  top: 0;
  display: block;
  height: 100vh;
  height: 100svh;
  overflow: hidden;
}

.og.is-scrolly .og-panel {
  position: absolute;
  inset: 0;
  display: grid;
  grid-template-columns: minmax(0, 0.85fr) minmax(0, 1.15fr);
  align-items: center;
  gap: clamp(1.5rem, 1rem + 3vw, 4.5rem);
  padding: clamp(5rem, 4rem + 4vh, 7rem) 0 clamp(2rem, 1rem + 4vh, 4rem) clamp(2.25rem, 1.5rem + 2vw, 4rem);
  will-change: opacity;
}

/* ---------- Words ---------- */
.og-year {
  margin: 0 0 1rem;
  font-family: var(--cg-font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--cg-pink);
}

.og-line {
  margin: 0;
  font-size: clamp(1.4rem, 0.95rem + 1.6vw, 2.5rem);
  font-weight: 500;
  line-height: 1.12;
  text-transform: uppercase;
  text-wrap: balance;
}

.og-sub {
  max-width: 30rem;
  margin: 1.25rem 0 0;
  font-size: 15px;
  line-height: 1.55;
  color: rgba(237, 223, 238, 0.8);
}

.og-sub em {
  font-style: normal;
  color: var(--cg-pink);
}

/* ---------- The screens ---------- */
.og-screen {
  position: relative;
  aspect-ratio: 4 / 3;
  width: 100%;
  max-height: 72vh;
  margin-inline: auto;
}

.og.is-scrolly .og-screen {
  width: min(100%, calc(72svh * 4 / 3));
}

/* An old monitor: a curved, glowing glass with scanlines and a vignette. */
.og-crt {
  overflow: hidden;
  border-radius: 6% / 8%;
  background: radial-gradient(ellipse at 50% 45%, #10200f 0%, #050a06 75%);
  box-shadow:
    inset 0 0 70px rgba(0, 0, 0, 0.9),
    0 0 0 1px rgba(255, 255, 255, 0.06),
    0 0 60px rgba(120, 255, 150, 0.05);
  color: var(--og-phosphor);
  transform-origin: 50% 50%;
}

.og-crt--cool {
  background: radial-gradient(ellipse at 50% 45%, #141a22 0%, #07090c 75%);
  color: var(--og-cool);
  box-shadow:
    inset 0 0 70px rgba(0, 0, 0, 0.9),
    0 0 0 1px rgba(255, 255, 255, 0.06),
    0 0 60px rgba(150, 190, 255, 0.05);
}

.og-crt::before,
.og-crt::after {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.og-crt::before {
  z-index: 2;
  background: repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.45) 0 1px, transparent 1px 3px);
  opacity: 0.6;
}

.og-crt::after {
  z-index: 3;
  background: radial-gradient(ellipse at 50% 50%, transparent 55%, rgba(0, 0, 0, 0.7) 100%);
}

.og-term {
  position: absolute;
  inset: 7% 8%;
  margin: 0;
  font-family: var(--cg-font-mono);
  font-size: clamp(9px, 0.35rem + 0.8vw, 15px);
  line-height: 1.5;
  white-space: pre;
  text-shadow:
    0 0 6px currentColor,
    1px 0 0 rgba(255, 60, 60, 0.35),
    -1px 0 0 rgba(60, 140, 255, 0.35);
}

.og-term-line {
  color: #b6ffc6;
}

.og-term-line.is-prompt {
  color: var(--og-phosphor);
}

.og-term-out {
  color: #fff;
}

.og-cursor {
  margin-left: 1px;
}

.og-diagram {
  position: absolute;
  inset: 6%;
  width: 88%;
  height: 88%;
  overflow: visible;
  filter: drop-shadow(0 0 4px rgba(207, 230, 255, 0.35));
}

.og-wire {
  stroke: currentColor;
  stroke-width: 2;
  stroke-dasharray: 7 5;
}

.og-wire-label,
.og-box-label {
  fill: currentColor;
  font-family: var(--cg-font-mono);
  font-size: 15px;
  font-weight: 500;
}

.og-box {
  fill: #0a0d10;
  stroke: currentColor;
  stroke-width: 2;
}

/* ---------- Today: a clean, wide panel ---------- */
.og-today {
  aspect-ratio: 16 / 10;
  display: grid;
  align-content: center;
  padding: clamp(1.25rem, 0.5rem + 3vw, 3rem);
  border: 1px solid rgba(237, 223, 238, 0.14);
  border-radius: 0.75rem;
  background: #161616;
}

.og-hops {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: clamp(0.5rem, 0.2rem + 1vw, 1.25rem);
  margin: 0;
  padding: 0;
  list-style: none;
}

.og-hops li {
  display: grid;
  gap: 0.5rem;
  padding: clamp(0.55rem, 0.3rem + 0.7vw, 0.9rem);
  border: 1px solid rgba(237, 223, 238, 0.16);
  border-radius: 0.5rem;
  background: var(--cg-black);
  opacity: 0.45;
  transition:
    opacity 0.4s var(--cg-ease),
    border-color 0.4s var(--cg-ease);
}

.og-hops li.is-lit {
  opacity: 1;
  border-color: var(--cg-pink);
}

.og-hop-lang {
  font-family: var(--cg-font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--cg-pink);
}

.og-hop-sym {
  font-family: var(--cg-font-mono);
  font-size: clamp(10px, 0.3rem + 0.45vw, 13px);
  overflow-wrap: break-word;
}

.og-hop-file {
  font-family: var(--cg-font-mono);
  font-size: 9.5px;
  color: rgba(237, 223, 238, 0.55);
  overflow-wrap: break-word;
}

.og-path {
  position: relative;
  height: 2px;
  margin: clamp(1rem, 0.5rem + 2vw, 2rem) 0 0;
}

.og-path-line {
  position: absolute;
  inset: 0;
  background: var(--cg-pink);
  transform-origin: 0 50%;
}

.og-path-packet {
  position: absolute;
  top: -4px;
  left: 0;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--cg-light-pink);
  box-shadow: 0 0 14px var(--cg-pink);
  opacity: 0;
}

.og-note {
  margin: 1.25rem 0 0;
  font-family: var(--cg-font-mono);
  font-size: 10px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(237, 223, 238, 0.45);
}

/* ---------- The rail: where the reader is ---------- */
.og-rail {
  position: absolute;
  z-index: 2;
  left: clamp(2.25rem, 1.5rem + 2vw, 4rem);
  bottom: clamp(1.5rem, 1rem + 3vh, 3rem);
  display: flex;
  gap: 0;
  margin: 0;
  padding: 0;
  list-style: none;
  font-family: var(--cg-font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: rgba(237, 223, 238, 0.35);
}

.og-rail li {
  display: flex;
  align-items: center;
  transition: color 0.3s var(--cg-ease);
}

/* A short rule between marks, lit once passed. */
.og-rail li + li::before {
  content: '';
  width: clamp(1.25rem, 0.5rem + 2vw, 2.75rem);
  height: 1px;
  margin: 0 0.75rem;
  background: currentColor;
  opacity: 0.6;
}

.og-rail li.is-past {
  color: rgba(237, 223, 238, 0.65);
}

.og-rail li.is-on {
  color: var(--cg-pink);
}

/* ---------- Motion that runs by itself: only while live ---------- */
.og-cursor {
  opacity: 1;
}

.og.is-live .og-cursor {
  animation: og-blink 1s steps(1) infinite;
}

.og.is-live .og-crt {
  animation: og-flicker 3.2s steps(1) infinite;
}

.og.is-live .og-crt::before {
  background:
    linear-gradient(transparent 40%, rgba(255, 255, 255, 0.05) 50%, transparent 60%) 0 -100% / 100% 300% no-repeat,
    repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.45) 0 1px, transparent 1px 3px);
  animation: og-roll 6s linear infinite;
}

.og.is-live .og-path-packet {
  animation: og-packet 1.8s var(--cg-ease) infinite;
}

@keyframes og-blink {
  50% {
    opacity: 0;
  }
}

@keyframes og-flicker {
  0%,
  100% {
    opacity: 1;
  }
  47% {
    opacity: 0.96;
  }
  48% {
    opacity: 1;
  }
  83% {
    opacity: 0.97;
  }
}

@keyframes og-roll {
  to {
    background-position:
      0 200%,
      0 0;
  }
}

@keyframes og-packet {
  0% {
    left: 0;
    opacity: 0;
  }
  10% {
    opacity: 1;
  }
  90% {
    opacity: 1;
  }
  100% {
    left: calc(100% - 10px);
    opacity: 0;
  }
}

/* ---------- Narrow screens ---------- */
@media (max-width: 760px) {
  .og-stage {
    grid-template-columns: minmax(0, 1fr);
  }

  .og.is-scrolly .og-panel {
    grid-template-columns: minmax(0, 1fr);
    align-content: center;
    gap: 1.5rem;
    padding-left: 1.75rem;
  }

  .og.is-scrolly .og-screen {
    width: 100%;
  }

  .og-hops {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .og-today {
    aspect-ratio: auto;
  }
}
</style>
