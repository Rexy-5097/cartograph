<script setup lang="ts">
// The site's home page. Every statement on it is taken from the README and the
// documentation, and every figure is one the README already publishes from
// committed benchmark results, with the same scope and caveats. Nothing here
// describes behaviour the code does not have.
//
// Motion lives in ./motion.ts and in the CSS under `.cg-motion`, a class the
// page carries only once it has mounted and only while motion is allowed —
// so without JavaScript, with motion paused, or under prefers-reduced-motion,
// everything is simply in place.

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { withBase } from 'vitepress'
import CartographMark from './CartographMark.vue'
import HalftoneField from './HalftoneField.vue'
import GlobeField from './GlobeField.vue'
import SplitWords from './SplitWords.vue'
import { counters, cursor, idleOffscreen, magnetic, marqueeVelocity, reveals, scrollScene } from './motion'

const REPO = 'https://github.com/Rexy-5097/cartograph'

/** A page on this site. Directory paths end in `/`; any other gets `.html`. */
const page = (path: string, hash = '') => withBase((path.endsWith('/') ? path : `${path}.html`) + hash)
/** A file in the repository, on GitHub. */
const repo = (path: string) => `${REPO}/blob/main/${path}`

const menu = [
  { text: 'Technical writeup', href: page('/writeup/technical-writeup') },
  { text: 'Resolver', href: page('/resolver/') },
  { text: 'Benchmarks', href: page('/benchmarks/') },
  { text: 'Architecture', href: page('/architecture/') },
  { text: 'Decisions', href: page('/adr/') },
  { text: 'Command line', href: page('/development/cli') },
  { text: 'Releases', href: page('/releases/') },
]

const statement =
  'A local-first engine that derives a cross-language architecture graph from source, and shows the evidence behind every edge it draws.'

const surfaces = [
  {
    key: 'CLI',
    title: 'The command line',
    tag: 'cartograph . · trace · blast · diff',
    body: 'What a repository contains, how one relationship crosses the stack, what depends on an artefact, and what a change did to the architecture.',
    link: { text: 'Command-line reference', href: page('/development/cli') },
    tone: 'black',
  },
  {
    key: 'MAP',
    title: 'MAP — the desktop application',
    tag: 'Tauri · Sigma',
    body: 'Draws the architecture graph and shows the evidence behind any edge.',
    link: { text: 'The desktop boundary', href: page('/adr/ADR-0016-desktop-boundary') },
    tone: 'green',
  },
  {
    key: 'MCP',
    title: 'cartograph-mcp',
    tag: 'map · trace · blast · diff',
    body: 'An MCP server exposing the same graph over one repository identity, granted when it is launched.',
    link: { text: 'MCP boundary and authorization', href: page('/adr/ADR-0020-mcp-boundary-and-authorization') },
    tone: 'pink',
  },
  {
    key: 'PR review',
    title: 'Pull-request review',
    tag: 'cartograph diff --markdown',
    body: 'A workflow that posts what a change did to the architecture as a pull-request comment. It is configured for this repository’s own pull requests; no reusable action is published.',
    link: { text: 'cartograph diff', href: page('/development/cli', '#cartograph-diff-before-after') },
    tone: 'light',
  },
  {
    key: 'ASK',
    title: 'ASK — explanation over evidence',
    tag: 'Opt-in per repository',
    body: 'Explains already-derived evidence, and every claim must cite that evidence verbatim. No in-product way to supply an API key ships yet, so in practice it shows the evidence unexplained.',
    link: { text: 'The citation contract', href: page('/adr/ADR-0021-ask-boundary-and-citation-contract') },
    tone: 'khaki',
  },
]

const steps = [
  {
    title: 'Parse',
    body: 'tree-sitter reads TypeScript, TSX and Python: symbols, imports, calls, HTTP requests, route declarations, and ORM models with the tables they name. Only the source under the path it is given — not Git history, not a database.',
    href: page('/architecture/parser'),
  },
  {
    title: 'Resolve',
    body: 'Dynamically built URLs are partially evaluated, four route-declaration dialects are normalised to one shape, client calls are matched to the handlers that serve them, and models to their tables. Every stage is deterministic.',
    href: page('/resolver/'),
  },
  {
    title: 'Prove',
    body: 'Every edge carries its provenance, a confidence and the evidence for it. An ambiguous match produces no edge; a segment that cannot be determined statically stays {unknown}.',
    href: page('/graph/'),
  },
]

const principles = [
  {
    title: 'No model in the graph',
    body: 'No language model constructs graph structure — not as a fallback, not as a hint, not behind a flag. A model may explain a finished subgraph, and nothing else.',
    href: page('/adr/ADR-0007-no-llm-graph-construction'),
    link: 'ADR-0007',
    tone: 'green',
  },
  {
    title: 'Refuse rather than guess',
    body: 'Ambiguous candidates produce no edge, a prefix that cannot be determined statically stays {unknown}, and a call to an absolute host is never joined to a local route.',
    href: page('/resolver/matching'),
    link: 'Matching',
    tone: 'pink',
  },
  {
    title: 'Evidence is first-class',
    body: 'Cartograph stores the claim and how it was known. The domain model will not construct an edge without its provenance, confidence and evidence.',
    href: page('/adr/ADR-0004-evidence-first-class'),
    link: 'ADR-0004',
    tone: 'light',
  },
  {
    title: 'No unmeasured number',
    body: 'No accuracy or performance claim without a benchmark run on this codebase, and failures are published alongside successes.',
    href: page('/benchmarks/'),
    link: 'Benchmarks',
    tone: 'khaki',
  },
  {
    title: 'Local-first',
    body: 'No source code leaves the machine. AI is opt-in per repository and off by default; Cartograph is fully useful with no API key and no network.',
    href: page('/security/'),
    link: 'Security',
    tone: 'black',
  },
]

/** The governing principle, one clause to a line, words numbered across lines. */
const creed = ['Deterministic analysis first.', 'Evidence second.', 'Inference third.', 'The model last, and never in the graph.']
const creedWords = (() => {
  let n = 0
  return creed.map((line) => line.split(' ').map((text) => ({ text, i: n++ })))
})()

const corpus = ['Superset', 'PostHog', 'Zulip', 'Onyx', 'Airflow', 'AutoGPT', 'Full-stack FastAPI']

const install = `cargo install --git https://github.com/Rexy-5097/cartograph cartograph-cli
cartograph .`

const trace = `$ cartograph trace enqueueConnectionTest

enqueueConnectionTest  (function)  .../ui/openapi-gen/requests/services.gen.ts:834
  │
  ↓ HTTP-CALL   confidence 0.98   provenance route-matcher
  │   evidence   POST /api/v2/connections/enqueue-test matched POST /api/v2/connections/enqueue-test
  │              at .../routes/public/connections.py:383 (enqueue_connection_test);
  │              exact HTTP method; every path segment static and equal
  │
enqueue_connection_test  (function)  .../core_api/routes/public/connections.py:383
  │
  ↓ ORM-ACCESS   confidence 0.80   provenance orm-resolution
  │   evidence   ConnectionTestRequest(...) in enqueue_connection_test at .../connections.py:427
  │
ConnectionTestRequest  (class)  .../airflow/models/connection_test.py:82
  │
  ↓ QUERIES   confidence 0.80   provenance orm-resolution
  │   evidence   ConnectionTestRequest maps to table "connection_test_request"
  │
connection_test_request  (table)

This path crosses the HTTP boundary and reaches a database table.`
const traceLines = trace.split('\n')

const MOTION_KEY = 'cartograph:motion-paused'

const root = ref<HTMLElement | null>(null)
const ring = ref<HTMLElement | null>(null)
const menuButton = ref<HTMLButtonElement | null>(null)
const menuOpen = ref(false)
const scrolled = ref(false)
const headerHidden = ref(false)
const surface = ref(0)
const principle = ref(0)
const copied = ref(false)
/** The viewer paused the page's motion with the header control. */
const still = ref(false)
/** The system asks for reduced motion. */
const reduced = ref(false)
const mounted = ref(false)
const motionOn = computed(() => mounted.value && !still.value && !reduced.value)
const isStill = () => still.value || reduced.value
/** The loading screen is in the page (shown only while <html> has .cg-loading). */
const loader = ref(true)
/** The loading screen has just cleared: the hero's entrances transition in. */
const arriving = ref(false)
const lockup = ref<HTMLElement | null>(null)
const loaderBg = ref<HTMLElement | null>(null)
/** The page's script is running: the loading screen's mark may build. */
const loaderReady = ref(false)

// The loading screen's timing, ms. The mark builds once the page's script is
// running, so the page starting up cannot stall it halfway; it builds for at
// least `build` while the fonts arrive, but never waits past `cap`. The name
// writes out over `name`; a beat of `hold`; the name retracts over `shrink`;
// then the mark alone moves into the header over `move` as the screen clears.
const INTRO = { build: 1400, cap: 3000, name: 650, hold: 350, shrink: 550, move: 850 }
// Entrances ease out; the retraction and the move ease in and out, so the
// mark sets off gently and settles gently.
const INTRO_EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'
const INTRO_GLIDE = 'cubic-bezier(0.65, 0, 0.35, 1)'
// Half the mark's width: the lockup scales about the mark's centre.
const MARK_HALF = 13

const cleanups: Array<() => void> = []

function toggleMotion() {
  still.value = !still.value
  try {
    localStorage.setItem(MOTION_KEY, still.value ? '1' : '0')
  } catch {
    // Storage may be unavailable; the choice then lasts for this visit only.
  }
}

async function copyInstall() {
  try {
    await navigator.clipboard.writeText(install)
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
  } catch {
    // Clipboard access refused; the command stays selectable as text.
  }
}

watch(menuOpen, async (open) => {
  document.documentElement.style.overflow = open ? 'hidden' : ''
  await nextTick()
  if (open) root.value?.querySelector<HTMLAnchorElement>('.cg-menu a')?.focus()
  else menuButton.value?.focus()
})

// Resuming motion: recompute scroll-linked state at once rather than on the
// next scroll.
watch(still, () => window.dispatchEvent(new Event('scroll')))

/**
 * The loading screen: the mark builds in the middle while the page loads, the
 * name writes out beside it and then retracts, and the mark alone moves into
 * the header logo's place as the screen clears; there the header's own name
 * writes out (CSS, on arrival). Any click, key, scroll or touch skips to the
 * end.
 */
async function playIntro() {
  const box = lockup.value
  const bg = loaderBg.value
  const logo = root.value?.querySelector<HTMLElement>('.cg-logo')
  const name = box?.querySelector('span')
  if (!box || !bg || !logo || !name || isStill()) return endIntro()

  let skipped = false
  let skip = () => {}
  const skipping = new Promise<void>((resolve) => (skip = resolve))
  const running: Animation[] = []
  const onInput = () => {
    skipped = true
    running.forEach((a) => a.finish())
    skip()
  }
  const inputs = ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const
  inputs.forEach((type) => window.addEventListener(type, onInput, { passive: true }))
  const pause = (ms: number) => Promise.race([new Promise<void>((r) => setTimeout(r, Math.max(0, ms))), skipping])
  const play = (el: Element, keyframes: Keyframe[], options: KeyframeAnimationOptions) => {
    const a = el.animate(keyframes, { easing: INTRO_EASE, fill: 'forwards', ...options })
    running.push(a)
    return Promise.race([a.finished.then(() => undefined, () => undefined), skipping])
  }

  try {
    // Loading: the mark builds itself (in CSS, from the first paint) while the
    // fonts arrive, so the name below is set in its own face.
    loaderReady.value = true
    const built = Promise.all([document.fonts?.ready, pause(INTRO.build)])
    await Promise.race([built, pause(INTRO.cap)])
    if (skipped) return

    // The name writes out, and the pair moves to be centred together.
    // Transforms are about the mark's centre (CSS), from the lockup's place.
    const k = parseFloat(getComputedStyle(box).getPropertyValue('--k')) || 2.6
    const half = box.offsetHeight / 2
    const alone = `translate(${-MARK_HALF}px, ${-half}px) scale(${k})`
    const centred = `translate(${-MARK_HALF - k * (box.offsetWidth / 2 - MARK_HALF)}px, ${-half}px) scale(${k})`
    await Promise.all([
      play(box, [{ transform: alone }, { transform: centred }], { duration: INTRO.name }),
      play(name, [{ clipPath: 'inset(0 100% 0 0)', transform: 'translateX(-0.5em)' }, { clipPath: 'inset(0 0% 0 0)', transform: 'none' }], {
        duration: INTRO.name,
        delay: 60,
      }),
    ])
    await pause(INTRO.hold)
    if (skipped) return

    // Back to the mark alone: the name retracts, and the mark re-centres.
    const hidden = { clipPath: 'inset(0 100% 0 0)', transform: 'translateX(-0.5em)' }
    await Promise.all([
      play(box, [{ transform: centred }, { transform: alone }], { duration: INTRO.shrink, easing: INTRO_GLIDE }),
      play(name, [{ clipPath: 'inset(0 0% 0 0)', transform: 'none' }, hidden], { duration: INTRO.shrink * 0.85, easing: INTRO_GLIDE }),
    ])
    if (skipped) return

    // Into the header: at scale 1, on top of the logo it stands in for. The
    // lockup's own place is read unrounded (offsetLeft rounds, and the centre
    // of the screen often falls on half a pixel), so it lands exactly.
    const target = logo.getBoundingClientRect()
    const screen = (box.offsetParent ?? document.body).getBoundingClientRect()
    const place = getComputedStyle(box)
    const dx = target.left - (screen.left + parseFloat(place.left))
    const dy = target.top - (screen.top + parseFloat(place.top))
    const home = `translate(${dx}px, ${dy}px) scale(1)`
    await Promise.all([
      play(box, [{ transform: alone }, { transform: home }], { duration: INTRO.move, easing: INTRO_GLIDE }),
      play(bg, [{ opacity: 1 }, { opacity: 0 }], { duration: INTRO.move * 0.75, delay: INTRO.move * 0.2, easing: 'ease' }),
    ])
  } finally {
    inputs.forEach((type) => window.removeEventListener(type, onInput))
    endIntro()
  }
}

/** Clear the loading screen and let the hero arrive. */
async function endIntro() {
  arriving.value = true
  await nextTick()
  document.documentElement.classList.remove('cg-loading')
  loader.value = false
  setTimeout(() => (arriving.value = false), 2800)
}

onMounted(async () => {
  const el = root.value
  if (!el) return
  // The head script (config.mts) decided before the first paint whether the
  // loading screen plays; arriving by in-site navigation, it never does.
  const loading = document.documentElement.classList.contains('cg-loading')
  if (!loading) loader.value = false

  try {
    still.value = localStorage.getItem(MOTION_KEY) === '1'
  } catch {
    still.value = false
  }
  const media = window.matchMedia('(prefers-reduced-motion: reduce)')
  reduced.value = media.matches
  const onMedia = () => (reduced.value = media.matches)
  media.addEventListener('change', onMedia)
  cleanups.push(() => media.removeEventListener('change', onMedia))

  let lastY = window.scrollY
  const onScroll = () => {
    const y = window.scrollY
    scrolled.value = y > 24
    if (!menuOpen.value) {
      if (y > lastY + 6 && y > 160) headerHidden.value = true
      else if (y < lastY - 6) headerHidden.value = false
    }
    lastY = y
  }
  onScroll()
  window.addEventListener('scroll', onScroll, { passive: true })
  cleanups.push(() => window.removeEventListener('scroll', onScroll))

  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && menuOpen.value) menuOpen.value = false
  }
  window.addEventListener('keydown', onKey)
  cleanups.push(() => window.removeEventListener('keydown', onKey))

  mounted.value = true
  await nextTick()

  cleanups.push(
    scrollScene(el, isStill),
    reveals(el),
    counters(el, isStill),
    magnetic(el, isStill),
    marqueeVelocity([...el.querySelectorAll<HTMLElement>('.cg-marquee ul')], isStill),
    idleOffscreen(el),
  )
  if (ring.value) cleanups.push(cursor(el, ring.value, isStill))
  if (loading) playIntro()
})

onBeforeUnmount(() => {
  document.documentElement.style.overflow = ''
  cleanups.forEach((fn) => fn())
})
</script>

<template>
  <div ref="root" class="cg" :class="{ 'cg-js': mounted, 'cg-motion': motionOn, 'is-still': still, 'is-arriving': arriving }">
    <div ref="ring" class="cg-cursor" aria-hidden="true" />

    <!-- The loading screen: in the page until it has played, and shown only
         while <html> has .cg-loading, set before the first paint (config.mts). -->
    <div v-if="loader" class="cg-loader" :class="{ 'is-ready': loaderReady }" aria-hidden="true">
      <div ref="loaderBg" class="cg-loader-bg" />
      <div ref="lockup" class="cg-loader-lockup">
        <CartographMark loading />
        <span>Cartograph</span>
      </div>
    </div>

    <!-- Header -->
    <header class="cg-header" :class="{ 'is-solid': scrolled || menuOpen, 'is-hidden': headerHidden && !menuOpen }">
      <a class="cg-logo" :href="withBase('/')" aria-label="Cartograph — home">
        <CartographMark idle />
        <span>Cartograph</span>
      </a>
      <div class="cg-header-actions">
        <!-- WCAG 2.2.2: everything that moves on its own can be paused. -->
        <button class="cg-motion-toggle" type="button" :aria-pressed="still" @click="toggleMotion">
          <i aria-hidden="true">{{ still ? '▶' : '‖' }}</i>
          <span>Pause motion</span>
        </button>
        <button
          ref="menuButton"
          class="cg-menu-button"
          type="button"
          :aria-expanded="menuOpen"
          aria-controls="cg-menu"
          @click="menuOpen = !menuOpen"
        >
          <span>{{ menuOpen ? 'Close' : 'Menu' }}</span>
          <i class="cg-plus" :class="{ 'is-open': menuOpen }" aria-hidden="true" />
        </button>
      </div>
    </header>

    <Transition name="cg-menu">
      <nav v-show="menuOpen" id="cg-menu" class="cg-menu" aria-label="Site">
        <ol class="cg-menu-list">
          <li v-for="(item, i) in menu" :key="item.text" :style="{ '--i': i }">
            <a :href="item.href" @click="menuOpen = false">
              <small>{{ String(i + 1).padStart(2, '0') }}</small>
              {{ item.text }}
            </a>
          </li>
        </ol>
        <div class="cg-menu-aside">
          <div>
            <p class="cg-label">Source</p>
            <a class="cg-menu-link" :href="REPO">Cartograph on GitHub <span aria-hidden="true">↗</span></a>
          </div>
          <div>
            <p class="cg-label">Get started</p>
            <a class="cg-menu-link" :href="page('/releases/v0.1.0')" @click="menuOpen = false">Install and commands</a>
            <a class="cg-menu-link" :href="page('/development/cli')" @click="menuOpen = false">Command-line reference</a>
          </div>
          <div>
            <p class="cg-label">Status</p>
            <p class="cg-menu-small">v0.1.0 · M00–M16 accepted · M17 in progress</p>
          </div>
        </div>
      </nav>
    </Transition>

    <!-- While the menu is open, only the header and the menu take focus. -->
    <main :inert="menuOpen || undefined">
      <!-- The hero stays pinned while the statement slides up over it. -->
      <div class="cg-stage" data-scroll="top">
        <section class="cg-hero">
          <!-- The globe is the hub; edges run from it to each data-globe-edge. -->
          <!-- Held still under the loading screen, where it cannot be seen. -->
          <GlobeField :paused="still || loader" />
          <h1 class="cg-hero-title">
            <span class="cg-hero-left" data-globe-edge>
              <span class="cg-line"><span style="--d: 0.05s">Compute the</span></span>
              <span class="cg-line"><span style="--d: 0.15s">architecture</span></span>
            </span>
            <span class="cg-hero-right" data-globe-edge>
              <span class="cg-line"><span style="--d: 0.3s">Prove the</span></span>
              <span class="cg-line"><span style="--d: 0.4s">relationships</span></span>
            </span>
          </h1>
          <div class="cg-hero-foot">
            <span data-globe-edge>Local-first architectural intelligence</span>
            <span data-globe-edge>TypeScript · TSX · Python</span>
            <span data-globe-edge>v0.1.0 · M17 in progress</span>
          </div>
        </section>

        <section class="cg-intro">
          <SplitWords class="cg-statement" fill :lines="[statement]" />
          <div class="cg-intro-side" data-reveal>
            <p>
              Cartograph reads a repository’s TypeScript, TSX and Python — symbols, imports, calls,
              HTTP requests, route declarations and ORM models — and joins them across languages that
              share no symbol table. One analysis core serves a CLI, a desktop application, an MCP
              server and a pull-request review workflow.
            </p>
            <a class="cg-button" :href="page('/writeup/technical-writeup')">
              <i aria-hidden="true">+</i>Read the writeup
            </a>
          </div>
          <div class="cg-badge" aria-hidden="true" data-idle>
            <svg viewBox="0 0 120 120">
              <defs>
                <path id="cg-badge-ring" d="M60 60m-46 0a46 46 0 1 1 92 0a46 46 0 1 1-92 0" />
              </defs>
              <text>
                <textPath href="#cg-badge-ring">No model in the graph · Evidence on every edge ·</textPath>
              </text>
            </svg>
            <CartographMark class="cg-badge-mark" />
          </div>
        </section>
      </div>

      <!-- Manilla sheet: surfaces, approach, field -->
      <div class="cg-sheet" data-scroll="enter">
        <section class="cg-section">
          <div class="cg-head">
            <div>
              <p class="cg-label" data-reveal>Surfaces</p>
              <SplitWords class="cg-h" :lines="['One analysis core.', 'Five surfaces.']" />
            </div>
            <a class="cg-button" :href="page('/releases/v0.1.0')" data-reveal>
              <i aria-hidden="true">+</i>Install and commands
            </a>
          </div>

          <div class="cg-acc" data-reveal>
            <article
              v-for="(s, i) in surfaces"
              :key="s.key"
              class="cg-acc-item"
              :class="[`tone-${s.tone}`, { 'is-open': surface === i }]"
            >
              <button
                class="cg-acc-strip"
                type="button"
                :aria-expanded="surface === i"
                :aria-controls="`cg-surface-${i}`"
                @click="surface = i"
              >
                <span class="cg-acc-strip-label">{{ s.key }}</span>
                <i aria-hidden="true">+</i>
              </button>
              <div :id="`cg-surface-${i}`" class="cg-acc-panel" :hidden="surface !== i">
                <div class="cg-acc-top">
                  <h3>{{ s.title }}</h3>
                  <span class="cg-index">{{ String(i + 1).padStart(2, '0') }}</span>
                </div>
                <div class="cg-acc-bottom">
                  <p class="cg-acc-tag">{{ s.tag }}</p>
                  <p>{{ s.body }}</p>
                  <a class="cg-button" :href="s.link.href"><i aria-hidden="true">+</i>{{ s.link.text }}</a>
                </div>
              </div>
            </article>
          </div>
        </section>

        <section class="cg-section">
          <div class="cg-head">
            <div>
              <p class="cg-label" data-reveal>How it works</p>
              <SplitWords class="cg-h" :lines="['Source in. Evidence out.']" />
            </div>
          </div>
          <div class="cg-steps" data-reveal data-idle>
            <a v-for="(s, i) in steps" :key="s.title" class="cg-step" :href="s.href" :style="{ '--i': i }">
              <div class="cg-step-top">
                <h3>{{ s.title }}</h3>
                <span class="cg-index">{{ String(i + 1).padStart(2, '0') }}</span>
              </div>
              <p>{{ s.body }}</p>
              <!--
                Each icon builds itself in order once its card is in (--d is a
                shape's place in that order), then keeps a small idle motion
                that says what the step does. The idle shapes are invisible
                unless motion is on.
              -->
              <svg v-if="i === 0" class="cg-step-icon" viewBox="0 0 120 120" aria-hidden="true">
                <!-- Parse: the tree assembles from its root; then tokens flow down it. -->
                <rect class="cg-ln" pathLength="1" x="52" y="14" width="16" height="12" style="--d: 0s" />
                <path class="cg-ln" pathLength="1" d="M60 26v16" style="--d: 0.3s" />
                <path class="cg-ln" pathLength="1" d="M60 42H24" style="--d: 0.5s" />
                <path class="cg-ln" pathLength="1" d="M60 42H96" style="--d: 0.5s" />
                <path class="cg-ln" pathLength="1" d="M24 42v14" style="--d: 0.75s" />
                <path class="cg-ln" pathLength="1" d="M60 42v14" style="--d: 0.75s" />
                <path class="cg-ln" pathLength="1" d="M96 42v14" style="--d: 0.75s" />
                <rect class="cg-pop" x="16" y="56" width="16" height="12" style="--d: 0.95s" />
                <rect class="cg-pop" x="52" y="56" width="16" height="12" style="--d: 1.05s" />
                <rect class="cg-pop" x="88" y="56" width="16" height="12" style="--d: 1.15s" />
                <path class="cg-ln" pathLength="1" d="M60 68v14" style="--d: 1.3s" />
                <path class="cg-ln" pathLength="1" d="M60 82H44" style="--d: 1.5s" />
                <path class="cg-ln" pathLength="1" d="M60 82H76" style="--d: 1.5s" />
                <path class="cg-ln" pathLength="1" d="M44 82v12" style="--d: 1.7s" />
                <path class="cg-ln" pathLength="1" d="M76 82v12" style="--d: 1.7s" />
                <rect class="cg-pop" x="36" y="94" width="16" height="12" style="--d: 1.85s" />
                <rect class="cg-pop" x="68" y="94" width="16" height="12" style="--d: 1.95s" />
                <circle class="cg-tok cg-tok--l" cx="60" cy="26" r="2.2" />
                <circle class="cg-tok cg-tok--ml" cx="60" cy="26" r="2.2" />
                <circle class="cg-tok cg-tok--r" cx="60" cy="26" r="2.2" />
                <circle class="cg-tok cg-tok--mr" cx="60" cy="26" r="2.2" />
              </svg>
              <svg v-else-if="i === 1" class="cg-step-icon" viewBox="0 0 120 120" aria-hidden="true">
                <!-- Resolve: a boundary rises, the call crosses it; then a match lands. -->
                <path class="cg-grow-y" d="M60 10v100" stroke-dasharray="3 5" style="--d: 0s" />
                <circle class="cg-ln cg-src" pathLength="1" cx="26" cy="60" r="14" style="--d: 0.35s" />
                <path class="cg-ln" pathLength="1" d="M20 60h12" style="--d: 0.75s" />
                <path class="cg-ln" pathLength="1" d="M38 52A24 24 0 0 1 82 52" style="--d: 0.95s" />
                <path class="cg-ln" pathLength="1" d="M82.7 46 82 52 77.2 48.4" style="--d: 1.45s" />
                <circle class="cg-ln cg-dst" pathLength="1" cx="94" cy="60" r="14" style="--d: 1.55s" />
                <path class="cg-ln" pathLength="1" d="M88 60h12M94 54v12" style="--d: 1.9s" />
                <g class="cg-hop"><circle cx="36" cy="61.6" r="2.4" /></g>
              </svg>
              <svg v-else class="cg-step-icon" viewBox="0 0 120 120" aria-hidden="true">
                <!-- Prove: the frame closes in, the check is drawn; then it keeps scanning. -->
                <path class="cg-corner" d="M14 30V14h16" style="--cx: -8px; --cy: -8px; --d: 0s" />
                <path class="cg-corner" d="M90 14h16v16" style="--cx: 8px; --cy: -8px; --d: 0.08s" />
                <path class="cg-corner" d="M106 90v16H90" style="--cx: 8px; --cy: 8px; --d: 0.16s" />
                <path class="cg-corner" d="M30 106H14V90" style="--cx: -8px; --cy: 8px; --d: 0.24s" />
                <rect class="cg-ln" pathLength="1" x="32" y="32" width="56" height="56" style="--d: 0.5s" />
                <path class="cg-grow-0" d="M32 32l56 56" stroke-dasharray="3 5" style="--d: 1.05s" />
                <path class="cg-ln cg-check" pathLength="1" d="M46 62l10 10 20-24" style="--d: 1.35s" />
                <line class="cg-scan" x1="34" y1="34" x2="86" y2="34" />
              </svg>
            </a>
          </div>
        </section>

        <section class="cg-field">
          <HalftoneField variant="field" dot="#aaa58c" cross="#222222" accent="#8a4a79" :paused="still" />
          <!--
            The governing principle, large, in the middle of the field. Its box
            opens from the centre and its edges draw, then the lines rise word
            by word. The dots part around it (data-clear).
          -->
          <div class="cg-creed-wrap">
            <div class="cg-creed" data-split data-clear>
              <ol class="cg-creed-lines">
                <li
                  v-for="(line, l) in creedWords"
                  :key="l"
                  :class="{ 'is-last': l === creedWords.length - 1 }"
                  :style="{ '--l': l }"
                >
                  <span class="cg-index" aria-hidden="true">{{ String(l + 1).padStart(2, '0') }}</span>
                  <span
                    ><template v-for="(w, k) in line" :key="w.i"
                      ><span class="cg-word"><span class="cg-word-in" :style="{ '--w': w.i }">{{ w.text }}</span></span
                      >{{ k < line.length - 1 ? ' ' : '' }}</template
                    ></span
                  >
                </li>
              </ol>
              <i class="cg-edge cg-edge--t" /><i class="cg-edge cg-edge--r" /><i class="cg-edge cg-edge--b" /><i
                class="cg-edge cg-edge--l"
              />
            </div>
          </div>
        </section>
      </div>

      <!-- Proof -->
      <section class="cg-proof cg-section">
        <div class="cg-head">
          <div>
            <p class="cg-label" data-reveal>Real output</p>
            <SplitWords class="cg-h" :lines="['One relationship,', 'all the way down.']" />
          </div>
          <a class="cg-button" :href="page('/benchmarks/')" data-reveal>
            <i aria-hidden="true">+</i>All benchmarks
          </a>
        </div>

        <div class="cg-proof-stage">
          <figure class="cg-terminal" data-reveal>
            <div class="cg-terminal-bar" aria-hidden="true">
              <i /><i /><i />
              <span>apache/airflow · 9b43d6a</span>
            </div>
            <pre><code><span v-for="(line, i) in traceLines" :key="i" class="cg-tl" :style="{ '--l': i }">{{ line || ' ' }}</span><span class="cg-caret" aria-hidden="true" :style="{ '--l': traceLines.length }" /></code></pre>
            <figcaption>
              Apache Airflow at commit 9b43d6a. Directory paths are shortened to “...” and the
              per-hop observed line is omitted; everything else is verbatim.
            </figcaption>
          </figure>

          <div class="cg-feature tone-light" data-reveal>
            <span class="cg-tag">Apache Airflow · 9b43d6a</span>
            <h3>A TypeScript function to a database table, across three languages.</h3>
            <p>
              An HTTP boundary, a Python handler and a SQLAlchemy model, with no shared symbol table
              — and every hop carries the evidence for itself.
            </p>
            <a class="cg-button" :href="page('/resolver/')">
              <i aria-hidden="true">+</i>How the resolver works
            </a>
          </div>

          <div class="cg-feature-row">
            <a
              class="cg-feature cg-feature--small tone-khaki"
              :href="page('/benchmarks/m07-report')"
              data-reveal
              style="--i: 0"
            >
              <span class="cg-tag">Benchmarks</span>
              <h3>M07 — validation against seven production repositories</h3>
              <span class="cg-button"><i aria-hidden="true">+</i>Read more</span>
            </a>
            <a
              class="cg-feature cg-feature--small tone-pink"
              :href="page('/benchmarks/cross-platform-parity')"
              data-reveal
              style="--i: 1"
            >
              <span class="cg-tag">Benchmarks</span>
              <h3>Cross-platform parity</h3>
              <span class="cg-button"><i aria-hidden="true">+</i>Read more</span>
            </a>
          </div>
        </div>
      </section>

      <!-- Measured -->
      <section class="cg-measured cg-section">
        <div class="cg-head">
          <div>
            <p class="cg-label" data-reveal>Measured</p>
            <SplitWords class="cg-h" :lines="['Failures included.']" />
          </div>
          <a class="cg-button" :href="page('/benchmarks/m08-report')" data-reveal>
            <i aria-hidden="true">+</i>The M08 report
          </a>
        </div>

        <div class="cg-cards">
          <a class="cg-card tone-light" :href="page('/benchmarks/m08-report')" data-reveal style="--i: 0">
            <span class="cg-card-rule" aria-hidden="true" />
            <span class="cg-tag cg-tag--plain">M08 · seven pinned repositories</span>
            <p class="cg-card-big">
              <span class="cg-sr">14,932</span><span aria-hidden="true" data-count="14,932">14,932</span>
              edges labelled. One false positive.
            </p>
            <p class="cg-card-small">
              One false positive among 11,221 independently verified edges. 24.9% (3,711) could not
              be verified from source, and they concentrate where evidence is weakest — so every
              figure is an upper bound.
            </p>
          </a>
          <a class="cg-card tone-green" :href="page('/benchmarks/m08-confidence-policy')" data-reveal style="--i: 1">
            <svg class="cg-orbit" viewBox="0 0 320 320" aria-hidden="true" data-idle>
              <g class="cg-orbit-rings">
                <circle cx="160" cy="160" r="150" />
                <circle cx="160" cy="160" r="112" stroke-dasharray="2 6" />
                <circle cx="160" cy="160" r="76" />
                <circle cx="160" cy="160" r="42" stroke-dasharray="2 6" />
              </g>
              <g class="cg-orbit-sat"><circle cx="160" cy="48" r="4" /></g>
              <g class="cg-orbit-sat cg-orbit-sat--slow"><circle cx="160" cy="10" r="3" /></g>
              <circle class="cg-orbit-core" cx="160" cy="160" r="9" />
              <g class="cg-orbit-pill" transform="translate(206 44)">
                <rect x="0" y="0" width="86" height="24" rx="12" />
                <text x="43" y="16">SOURCE</text>
              </g>
              <g class="cg-orbit-pill" transform="translate(26 244)">
                <rect x="0" y="0" width="98" height="24" rx="12" />
                <text x="49" y="16">EVIDENCE</text>
              </g>
            </svg>
            <p class="cg-card-big cg-card-big--small">Confidence values are uncalibrated priors, not probabilities.</p>
            <p class="cg-card-small">
              Calibration error 0.18, toward under-confidence. They must not be thresholded as if
              they were probabilities.
            </p>
          </a>
        </div>

        <dl class="cg-stats">
          <div data-reveal style="--i: 0">
            <dt><span class="cg-sr">0.907</span><span aria-hidden="true" data-count="0.907">0.907</span></dt>
            <dd>HttpCall recall · Airflow only · 117 of 129 operations its generated client declares that a route serves</dd>
          </div>
          <div data-reveal style="--i: 1">
            <dt><span class="cg-sr">0.744</span><span aria-hidden="true" data-count="0.744">0.744</span></dt>
            <dd>OrmAccess recall · Airflow only · 1,401 of 1,883 import-bound uses of models that declare a table</dd>
          </div>
          <div data-reveal style="--i: 2">
            <dt>3 of 5</dt>
            <dd>End-to-end chains · Airflow only · chains whose every hop is independently readable</dd>
          </div>
        </dl>
        <p class="cg-footnote" data-reveal>
          Recall was measured in one repository, inside a stated scope. It is not recall across the
          corpus and must not be quoted as such.
        </p>
      </section>

      <!-- Principles -->
      <section class="cg-principles cg-section">
        <div>
          <p class="cg-label" data-reveal>Principles</p>
          <SplitWords class="cg-display" :lines="['Unknown stays', 'unknown.']" />
          <div class="cg-principles-copy" data-reveal>
            <p>
              A plausible-looking guess is worse than a gap, because it is indistinguishable from a
              derived fact. Where the evidence is thin, Cartograph refuses. Contributions are
              welcome — real repositories that break the resolver most of all.
            </p>
            <a class="cg-button" :href="repo('CONTRIBUTING.md')">
              <i aria-hidden="true">+</i>Contributing guide
            </a>
          </div>
        </div>

        <ol class="cg-values" data-reveal>
          <li
            v-for="(p, i) in principles"
            :key="p.title"
            class="cg-value"
            :class="[`tone-${p.tone}`, { 'is-open': principle === i }]"
            :style="{ '--i': i }"
            @mouseenter="principle = i"
          >
            <button type="button" :aria-expanded="principle === i" @click="principle = i" @focus="principle = i">
              <span class="cg-index">{{ String(i + 1).padStart(2, '0') }}</span>
              <span class="cg-value-title">{{ p.title }}</span>
            </button>
            <div class="cg-value-body" :inert="principle !== i || undefined">
              <div>
                <p>{{ p.body }}</p>
                <a :href="p.href">{{ p.link }} →</a>
              </div>
            </div>
          </li>
        </ol>
      </section>

      <!-- Corpus -->
      <section class="cg-corpus" data-scroll="enter">
        <div class="cg-section cg-corpus-head">
          <p class="cg-label" data-reveal>Corpus</p>
          <SplitWords class="cg-h" :lines="['Measured on seven pinned', 'production repositories.']" />
        </div>
        <div class="cg-marquee" data-idle>
          <ul aria-label="The seven repositories in the benchmark corpus">
            <li v-for="name in corpus" :key="name">{{ name }}</li>
          </ul>
          <ul aria-hidden="true">
            <li v-for="name in corpus" :key="name">{{ name }}</li>
          </ul>
        </div>
      </section>
    </main>

    <!-- Footer -->
    <footer class="cg-footer" :inert="menuOpen || undefined">
      <!-- The dome holds the one place the install command appears; its dots part around it. -->
      <div class="cg-dome">
        <HalftoneField variant="dome" dot="#c9c6b8" cross="#eddfee" accent="#cd8dbd" :paused="still" />
        <div class="cg-try" data-clear>
          <p class="cg-label" data-reveal>Try it</p>
          <SplitWords class="cg-try-title" :lines="['Run it on your', 'own repository.']" />
          <div class="cg-try-cmd" data-reveal style="--i: 1">
            <pre><code>{{ install }}</code></pre>
            <button class="cg-copy" type="button" @click="copyInstall">{{ copied ? 'Copied' : 'Copy' }}</button>
            <span class="cg-sr" aria-live="polite">{{ copied ? 'Command copied' : '' }}</span>
          </div>
          <div class="cg-try-links" data-reveal style="--i: 2">
            <a class="cg-button" :href="page('/releases/v0.1.0')"><i aria-hidden="true">+</i>Install and commands</a>
            <a class="cg-button" :href="page('/development/cli')"><i aria-hidden="true">+</i>Command-line reference</a>
          </div>
        </div>
      </div>
      <div class="cg-footer-grid">
        <div data-reveal style="--i: 0">
          <p class="cg-label">Get started</p>
          <ul>
            <li><a :href="page('/releases/v0.1.0')">Install and commands</a></li>
            <li><a :href="page('/development/cli')">Command-line reference</a></li>
            <li><a :href="page('/releases/')">Releases</a></li>
          </ul>
        </div>
        <div data-reveal style="--i: 1">
          <p class="cg-label">Status</p>
          <p>Milestones M00–M16 accepted.<br />M17, the public launch, in progress.<br />Version 0.1.0.</p>
        </div>
        <div data-reveal style="--i: 2">
          <p class="cg-label">Documentation</p>
          <ul>
            <li><a :href="page('/writeup/technical-writeup')">Technical writeup</a></li>
            <li><a :href="page('/resolver/')">Resolver</a></li>
            <li><a :href="page('/benchmarks/')">Benchmarks</a></li>
            <li><a :href="page('/architecture/')">Architecture</a></li>
            <li><a :href="page('/adr/')">Decisions</a></li>
            <li><a :href="page('/security/')">Security</a></li>
          </ul>
        </div>
        <div data-reveal style="--i: 3">
          <p class="cg-label">Project</p>
          <ul>
            <li><a :href="REPO">Source on GitHub</a></li>
            <li><a :href="repo('ROADMAP.md')">Roadmap</a></li>
            <li><a :href="repo('CHANGELOG.md')">Changelog</a></li>
            <li><a :href="repo('CONTRIBUTING.md')">Contributing</a></li>
            <li><a :href="repo('PROJECT_RULES.md')">Project rules</a></li>
          </ul>
        </div>
      </div>
      <div class="cg-wordmark" aria-hidden="true" data-split>
        <span v-for="(ch, i) in 'CARTOGRAPH'" :key="i" class="cg-word"><span class="cg-word-in" :style="{ '--w': i }">{{ ch }}</span></span>
      </div>
      <div class="cg-footer-base">
        <span>© Cartograph contributors</span>
        <a :href="repo('LICENSE')">Apache-2.0 licence</a>
        <span>Built with <a href="https://vitepress.dev/">VitePress</a></span>
      </div>
    </footer>
  </div>
</template>

<style scoped>
/* ---------- Base ---------- */

.cg {
  --pad-section: clamp(4rem, 2.5rem + 3.5vw, 7rem);
  font-family: var(--cg-font);
  color: var(--cg-light-pink);
  background: var(--cg-black);
  overflow-x: clip;
  -webkit-font-smoothing: antialiased;
}

/* Zero specificity, so a tone class on a link still sets its colour. */
:where(.cg) a {
  color: inherit;
  text-decoration: none;
}

/* The ring takes the text colour, which every block already sets to 4.5:1
   against its own background; a fixed pink fails 3:1 on the light ones. */
.cg :focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 3px;
}

.cg-sr {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}

.cg-section {
  padding: var(--pad-section) var(--cg-gutter);
}

.cg-label {
  margin: 0 0 0.75rem;
  font-family: var(--cg-font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  line-height: 1.3;
  text-transform: uppercase;
}

.cg-h {
  margin: 0;
  font-size: clamp(1.15rem, 0.85rem + 1.05vw, 1.75rem);
  font-weight: 500;
  line-height: 1.2;
  text-transform: uppercase;
}

.cg-index {
  font-family: var(--cg-font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.04em;
}

.cg-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 1.5rem;
  margin-bottom: clamp(2rem, 1.4rem + 1.8vw, 3.25rem);
}

/* The "+ LABEL" button. It leans toward a fine pointer (--mx, --my). */
.cg-button {
  display: inline-flex;
  align-items: center;
  gap: 0.75rem;
  font-family: var(--cg-font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  white-space: nowrap;
  cursor: pointer;
  translate: var(--mx, 0) var(--my, 0);
  transition: translate 0.4s var(--cg-ease);
}

.cg-button i {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border: 1px solid currentColor;
  border-radius: 0.3em;
  font-style: normal;
  font-size: 15px;
  font-weight: 400;
  transition:
    background-color 0.3s var(--cg-ease),
    color 0.3s var(--cg-ease),
    transform 0.4s var(--cg-ease);
}

.cg-button:hover i,
a:hover > .cg-button i {
  background: var(--cg-pink);
  border-color: var(--cg-pink);
  color: var(--cg-black);
  transform: rotate(90deg);
}

/* ---------- Header and menu ---------- */

.cg-header {
  position: fixed;
  inset: 0 0 auto;
  z-index: 60;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 72px;
  padding: 0 var(--cg-gutter);
  transition:
    background-color 0.4s var(--cg-ease),
    height 0.4s var(--cg-ease),
    transform 0.6s var(--cg-ease);
}

.cg-header.is-solid {
  height: 60px;
  background: var(--cg-black);
}

.cg-header.is-hidden:not(:focus-within) {
  transform: translateY(-100%);
}

/* The loading screen's lockup shares the logo's rules, so it lands on it exactly. */
.cg-logo,
.cg-loader-lockup {
  display: inline-flex;
  align-items: center;
  gap: 0.6rem;
  color: var(--cg-pink) !important;
  font-size: 1.2rem;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

/* The mark's own hover and focus motion is in CartographMark.vue. */
.cg-logo .cg-mark,
.cg-loader-lockup .cg-mark {
  width: 26px;
  height: 26px;
}

/* ---------- Loading screen ---------- */

.cg-loader {
  display: none;
}

html.cg-loading .cg-loader {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: block;
}

.cg-loader-bg {
  position: absolute;
  inset: 0;
  background-color: var(--cg-black);
}

/* Scaled up and centred on the mark alone; playIntro moves it from here.
   The origin is the mark's centre (MARK_HALF in the script). */
.cg-loader-lockup {
  --k: 2.6;
  position: absolute;
  top: 50%;
  left: 50%;
  transform-origin: 13px 50%;
  transform: translate(-13px, -50%) scale(var(--k));
}

.cg-loader-lockup > span {
  clip-path: inset(0 100% 0 0);
}

/* Until the page's script runs, the mark waits at the start of its build:
   starting up would otherwise stall the build halfway, on the main thread. */
.cg-loader:not(.is-ready) :deep(.cg-mark *) {
  animation-play-state: paused;
}

@media (max-width: 640px) {
  .cg-loader-lockup {
    --k: 1.6;
  }
}

/* While it plays, the header logo waits under it and the hero holds its
   entrances; once it clears, they arrive. */
html.cg-loading .cg-logo {
  visibility: hidden;
}

html.cg-loading .cg-header-actions,
html.cg-loading .cg-hero-foot {
  opacity: 0;
}

html.cg-loading .cg-line > span {
  animation: none;
  transform: translateY(105%);
}

html.cg-loading .cg-hero :deep(.cg-globe) {
  opacity: 0;
  transform: scale(0.82);
}

/* The mark has landed: the header's name writes out beside it, as it did on
   the screen, and then the page arrives. */
.cg.is-arriving .cg-logo > span {
  animation: cg-logo-name 0.6s var(--cg-ease) backwards;
}

@keyframes cg-logo-name {
  from {
    clip-path: inset(0 100% 0 0);
    transform: translateX(-0.5em);
  }
}

.cg.is-arriving .cg-line > span {
  animation-delay: calc(var(--d, 0s) + 0.4s);
}

.cg.is-arriving .cg-header-actions {
  transition: opacity 0.6s ease 0.5s;
}

.cg.is-arriving .cg-hero-foot {
  transition: opacity 0.8s ease 1s;
}

.cg.is-arriving .cg-hero :deep(.cg-globe) {
  transition:
    opacity 1s ease 0.35s,
    transform 1.6s var(--cg-ease) 0.35s;
}

.cg-header-actions {
  display: flex;
  align-items: center;
  gap: clamp(1rem, 0.5rem + 1.5vw, 2rem);
}

.cg-motion-toggle,
.cg-menu-button {
  display: inline-flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.5rem 0;
  color: var(--cg-light-pink);
  font-family: var(--cg-font);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.cg-menu-button {
  gap: 0.8rem;
}

.cg-motion-toggle i {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border: 1px solid currentColor;
  border-radius: 0.3em;
  font-style: normal;
  font-size: 9px;
  transition:
    background-color 0.3s var(--cg-ease),
    color 0.3s var(--cg-ease);
}

.cg-motion-toggle[aria-pressed='true'] i {
  background: var(--cg-light-pink);
  color: var(--cg-black);
}

.cg-plus {
  position: relative;
  width: 16px;
  height: 16px;
  transition: transform 0.5s var(--cg-ease);
}

.cg-plus::before,
.cg-plus::after {
  content: '';
  position: absolute;
  inset: 50% 0 auto;
  height: 1.5px;
  margin-top: -0.75px;
  background: var(--cg-pink);
}

.cg-plus::after {
  transform: rotate(90deg);
}

.cg-plus.is-open {
  transform: rotate(135deg);
}

.cg-menu-button:hover .cg-plus:not(.is-open) {
  transform: rotate(90deg);
}

.cg-menu {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: grid;
  grid-template-columns: 1.4fr 1fr;
  align-items: end;
  gap: 3rem;
  padding: 110px var(--cg-gutter) var(--cg-gutter);
  overflow-y: auto;
  background: var(--cg-black);
  clip-path: inset(0 0 0 0);
  transition: clip-path 0.8s var(--cg-ease);
}

.cg-menu-enter-from,
.cg-menu-leave-to {
  clip-path: inset(0 0 100% 0);
}

.cg-menu-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.cg-menu-list a {
  display: flex;
  align-items: baseline;
  gap: 1rem;
  padding: 0.2rem 0;
  color: var(--cg-pink);
  font-size: clamp(2rem, 1.1rem + 3.2vw, 4.25rem);
  font-weight: 500;
  line-height: 1.05;
  text-transform: uppercase;
  transition:
    color 0.3s var(--cg-ease),
    transform 0.5s var(--cg-ease);
}

.cg-menu-list a:hover {
  color: var(--cg-light-pink);
  transform: translateX(0.75rem);
}

.cg-menu-list small {
  min-width: 2ch;
  font-family: var(--cg-font-mono);
  font-size: 11px;
  font-weight: 600;
  color: var(--cg-light-pink);
}

.cg-menu-aside {
  display: grid;
  gap: 2rem;
  padding-bottom: 0.5rem;
}

.cg-menu-link {
  display: block;
  width: fit-content;
  padding: 0.15rem 0;
  font-size: 15px;
  font-weight: 500;
  border-bottom: 1px solid rgba(237, 223, 238, 0.3);
  transition:
    color 0.3s var(--cg-ease),
    border-color 0.3s var(--cg-ease);
}

.cg-menu-link + .cg-menu-link {
  margin-top: 0.4rem;
}

.cg-menu-link:hover {
  color: var(--cg-pink);
  border-color: var(--cg-pink);
}

.cg-menu-small {
  margin: 0;
  font-family: var(--cg-font-mono);
  font-size: 12px;
  line-height: 1.6;
}

/* ---------- Stage: a pinned hero, and the statement over it ---------- */

.cg-stage {
  position: relative;
}

.cg-hero {
  position: sticky;
  top: 0;
  z-index: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-height: 100svh;
  padding: 120px var(--cg-gutter) var(--cg-gutter);
  overflow: hidden;
}

/* The two halves sit in opposite corners, with the globe between them. */
.cg-hero-title {
  margin: 0;
  color: var(--cg-pink);
  font-size: clamp(2.2rem, 0.8rem + 4.1vw, 5rem);
  font-weight: 500;
  line-height: 1;
  letter-spacing: -0.01em;
  text-transform: uppercase;
}

.cg-hero-left,
.cg-hero-right {
  position: absolute;
  z-index: 1;
  display: flex;
  flex-direction: column;
}

.cg-hero-left {
  top: clamp(104px, 15vh, 170px);
  left: var(--cg-gutter);
}

.cg-hero-right {
  right: var(--cg-gutter);
  bottom: clamp(96px, 15vh, 160px);
  align-items: flex-end;
  text-align: right;
}

.cg-line {
  display: block;
  overflow: hidden;
  padding-bottom: 0.04em;
}

.cg-line > span {
  display: block;
  animation: cg-rise 1.2s var(--cg-ease) backwards;
  animation-delay: var(--d, 0s);
}

@keyframes cg-rise {
  from {
    transform: translateY(105%);
  }
}

.cg-hero-foot {
  position: absolute;
  inset: auto var(--cg-gutter) var(--cg-gutter);
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  font-family: var(--cg-font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(237, 223, 238, 0.72);
  animation: cg-fade 1.2s 0.7s var(--cg-ease) backwards;
}

/* The globe's edges end at these; lift them over its canvas. */
.cg-hero-foot span {
  position: relative;
  z-index: 1;
}

@keyframes cg-fade {
  from {
    opacity: 0;
  }
}

/* ---------- Statement ---------- */

.cg-intro {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
  align-items: end;
  gap: 3rem 4rem;
  padding: var(--pad-section) var(--cg-gutter) calc(var(--pad-section) * 1.6);
  background: var(--cg-black);
}

.cg-statement {
  margin: 0;
  font-size: clamp(1.4rem, 0.95rem + 1.45vw, 2.2rem);
  font-weight: 500;
  line-height: 1.2;
  text-transform: uppercase;
}

.cg-intro-side {
  max-width: 34rem;
}

.cg-intro-side p {
  margin: 0 0 2rem;
  font-size: 15px;
  line-height: 1.55;
  color: rgba(237, 223, 238, 0.86);
}

.cg-badge {
  position: absolute;
  right: var(--cg-gutter);
  bottom: calc(var(--pad-section) * 0.35);
  width: 128px;
  height: 128px;
  color: var(--cg-light-pink);
}

.cg-badge > svg:first-child {
  width: 100%;
  height: 100%;
  animation: cg-spin 26s linear infinite;
}

.cg-badge text {
  fill: currentColor;
  font-family: var(--cg-font-mono);
  font-size: 8.4px;
  font-weight: 600;
  letter-spacing: 0.18em;
  text-transform: uppercase;
}

.cg-badge-mark {
  position: absolute;
  inset: 50% auto auto 50%;
  width: 40px;
  height: 40px;
  transform: translate(-50%, -50%);
  color: var(--cg-pink);
}

@keyframes cg-spin {
  to {
    transform: rotate(360deg);
  }
}

/* ---------- Manilla sheet ---------- */

.cg-sheet {
  position: relative;
  z-index: 1;
  color: var(--cg-black);
  background: var(--cg-manilla);
  border-radius: 1.25rem 1.25rem 0 0;
  transform-origin: 50% 0;
}

/* Accordion of surfaces. */
.cg-acc {
  display: flex;
  height: clamp(460px, 64vh, 640px);
}

.cg-acc-item {
  position: relative;
  flex: 0 0 64px;
  overflow: hidden;
  transition: flex-basis 0.8s var(--cg-ease), flex-grow 0.8s var(--cg-ease);
}

.cg-acc-item.is-open {
  flex: 1 1 0;
}

.cg-acc-item:not(.is-open):hover {
  flex-basis: 84px;
}

.cg-acc-strip {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: space-between;
  padding: 1.1rem 0 1.4rem;
  color: inherit;
  cursor: pointer;
}

.cg-acc-strip i {
  order: -1;
  font-style: normal;
  font-size: 18px;
  line-height: 1;
  transition: transform 0.5s var(--cg-ease);
}

.cg-acc-strip:hover i {
  transform: rotate(90deg);
}

.cg-acc-strip-label {
  writing-mode: vertical-rl;
  transform: rotate(180deg);
  font-family: var(--cg-font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

/* Hidden, not transparent: a transparent button would stay a tab stop. */
.is-open > .cg-acc-strip {
  visibility: hidden;
}

.cg-acc-panel {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  min-width: min(560px, 80vw);
  padding: clamp(1.25rem, 0.9rem + 1.2vw, 2rem);
}

.cg-acc-panel[hidden] {
  display: none;
}

.cg-acc-top {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
}

.cg-acc-top h3 {
  margin: 0;
  max-width: 18ch;
  font-family: var(--cg-font-mono);
  font-size: clamp(1rem, 0.8rem + 0.7vw, 1.35rem);
  font-weight: 600;
  line-height: 1.25;
  text-transform: uppercase;
}

.cg-acc-bottom {
  max-width: 44ch;
}

.cg-acc-bottom p {
  margin: 0 0 1.25rem;
  font-family: var(--cg-font-mono);
  font-size: 12.5px;
  line-height: 1.6;
}

.cg-acc-bottom .cg-acc-tag {
  margin-bottom: 0.75rem;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

/* Tones shared by every coloured block. */
.tone-black {
  background: var(--cg-black);
  color: var(--cg-light-pink);
}
.tone-green {
  background: var(--cg-green);
  color: var(--cg-light-pink);
}
.tone-pink {
  background: var(--cg-pink);
  color: var(--cg-black);
}
.tone-light {
  background: var(--cg-light-pink);
  color: var(--cg-black);
}
.tone-khaki {
  background: var(--cg-khaki);
  color: var(--cg-black);
}

/* Steps. */
.cg-steps {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  border: 1px solid rgba(34, 34, 34, 0.18);
}

.cg-step {
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: 430px;
  padding: clamp(1.25rem, 0.9rem + 1.2vw, 2rem);
  overflow: hidden;
}

/* A fill that rises from the bottom edge on hover. */
.cg-step::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: -1;
  background: rgba(255, 255, 255, 0.45);
  transform: scaleY(0);
  transform-origin: 50% 100%;
  transition: transform 0.6s var(--cg-ease);
}

.cg-step:hover::before {
  transform: scaleY(1);
}

.cg-step + .cg-step {
  border-left: 1px solid rgba(34, 34, 34, 0.18);
}

.cg-step-top {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  margin-bottom: 1.5rem;
}

.cg-step h3 {
  margin: 0;
  font-size: clamp(1.05rem, 0.9rem + 0.5vw, 1.35rem);
  font-weight: 500;
  text-transform: uppercase;
}

.cg-step p {
  max-width: 40ch;
  margin: 0;
  font-family: var(--cg-font-mono);
  font-size: 12.5px;
  line-height: 1.65;
}

.cg-step-icon {
  /* When this icon starts building (after its card is in), and when its idle
     motion starts (after it has built). */
  --base: calc(0.45s + var(--i, 0) * 0.18s);
  --idle: calc(var(--base) + 2.6s);
  align-self: flex-end;
  width: clamp(96px, 7vw + 40px, 132px);
  margin-top: auto;
  padding-top: 2rem;
  fill: none;
  stroke: var(--cg-black);
  stroke-width: 1.1;
  transition: transform 0.6s var(--cg-ease);
}

.cg-step-icon .cg-pop,
.cg-step-icon .cg-grow-y,
.cg-step-icon .cg-grow-0,
.cg-step-icon .cg-check,
.cg-step-icon .cg-src,
.cg-step-icon .cg-dst,
.cg-step-icon .cg-tok {
  transform-box: fill-box;
  transform-origin: center;
}

.cg-step-icon .cg-grow-0 {
  transform-origin: 0 0;
}

/* The hop circles the arc's centre. */
.cg-step-icon .cg-hop {
  transform-box: view-box;
  transform-origin: 60px 61.6px;
}

.cg-step-icon .cg-tok,
.cg-step-icon .cg-hop circle {
  fill: var(--cg-black);
  stroke: none;
}

.cg-step-icon .cg-scan {
  stroke: var(--cg-pink-deep);
}

/* Idle shapes exist only while motion is on. */
.cg-step-icon .cg-tok,
.cg-step-icon .cg-hop,
.cg-step-icon .cg-scan {
  opacity: 0;
}

.cg-step:hover .cg-step-icon {
  transform: translateY(-8px) rotate(-3deg);
}

/* Halftone field. */
.cg-field {
  position: relative;
  height: clamp(440px, 82vh, 860px);
  border-top: 1px solid rgba(34, 34, 34, 0.12);
}

/* The governing principle, centred in the field. */
.cg-creed-wrap {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 0 var(--cg-gutter);
  pointer-events: none;
}

.cg-creed {
  /* The words wait for the box to open (see .cg-word-in in style.css). */
  --split-delay: 0.9s;
  position: relative;
  max-width: 46rem;
  padding: clamp(1.25rem, 0.7rem + 1.8vw, 2.5rem) clamp(1.25rem, 0.5rem + 2.6vw, 3rem);
  background-color: rgba(237, 232, 218, 0.92);
  pointer-events: auto;
}

.cg-creed-lines {
  display: grid;
  gap: 0.3em;
  margin: 0;
  padding: 0;
  list-style: none;
}

.cg-creed-lines li {
  display: grid;
  grid-template-columns: clamp(1.75rem, 1.2rem + 1.5vw, 2.75rem) minmax(0, 1fr);
  align-items: baseline;
  color: var(--cg-black);
  font-size: clamp(1.3rem, 0.7rem + 2.1vw, 2.5rem);
  font-weight: 500;
  line-height: 1.06;
  letter-spacing: -0.01em;
  text-transform: uppercase;
}

/* The last clause is the one that is never relaxed. */
.cg-creed-lines li.is-last {
  color: var(--cg-pink-deep);
}

.cg-creed-lines .cg-index {
  align-self: start;
  padding-top: 0.5em;
  color: var(--cg-black);
}

.cg-edge {
  position: absolute;
  background: var(--cg-black);
}

.cg-edge--t {
  inset: 0 0 auto;
  height: 1px;
  transform-origin: 0 50%;
}

.cg-edge--r {
  inset: 0 0 0 auto;
  width: 1px;
  transform-origin: 50% 0;
}

.cg-edge--b {
  inset: auto 0 0;
  height: 1px;
  transform-origin: 100% 50%;
}

.cg-edge--l {
  inset: 0 auto 0 0;
  width: 1px;
  transform-origin: 50% 100%;
}

/* ---------- Proof (white) ---------- */

.cg-proof,
.cg-measured {
  position: relative;
  z-index: 1;
  color: var(--cg-black);
  background: var(--cg-white);
}

.cg-proof-stage {
  display: grid;
  grid-template-columns: repeat(12, minmax(0, 1fr));
  grid-template-rows: auto clamp(5rem, 9vw, 9rem) auto auto;
}

.cg-terminal {
  grid-column: 1 / -1;
  grid-row: 1 / 3;
  margin: 0;
  padding: clamp(1.25rem, 0.6rem + 2.4vw, 3rem);
  padding-bottom: clamp(6rem, 11vw, 11rem);
  color: var(--cg-light-pink);
  background: var(--cg-black);
  overflow-x: auto;
}

.cg-terminal-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 1.75rem;
}

.cg-terminal-bar i {
  width: 9px;
  height: 9px;
  border: 1px solid rgba(237, 223, 238, 0.4);
  border-radius: 50%;
}

.cg-terminal-bar span {
  margin-left: 0.75rem;
  font-family: var(--cg-font-mono);
  font-size: 10.5px;
  letter-spacing: 0.04em;
  color: rgba(237, 223, 238, 0.6);
}

.cg-terminal pre {
  margin: 0;
  font-family: var(--cg-font-mono);
  font-size: clamp(10.5px, 0.45rem + 0.42vw, 13.5px);
  line-height: 1.65;
  white-space: pre;
}

.cg-tl {
  display: block;
}

.cg-caret {
  display: inline-block;
  width: 0.6em;
  height: 1.15em;
  margin-top: 0.2em;
  background: var(--cg-pink);
  vertical-align: text-bottom;
}

.cg-terminal figcaption {
  max-width: 60ch;
  margin-top: 1.75rem;
  font-family: var(--cg-font-mono);
  font-size: 11px;
  line-height: 1.6;
  color: rgba(237, 223, 238, 0.6);
}

.cg-feature {
  grid-column: 7 / -1;
  grid-row: 2 / 4;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1rem;
  padding: clamp(1.25rem, 0.8rem + 1.5vw, 2.25rem);
}

.cg-feature h3 {
  margin: 0;
  font-size: clamp(1.15rem, 0.85rem + 1vw, 1.75rem);
  font-weight: 500;
  line-height: 1.2;
  text-transform: uppercase;
}

.cg-feature p {
  margin: 0 0 0.5rem;
  max-width: 48ch;
  font-size: 14.5px;
  line-height: 1.55;
}

.cg-feature-row {
  grid-column: 7 / -1;
  grid-row: 4;
  display: grid;
  grid-template-columns: 1fr 1fr;
}

.cg-feature--small {
  grid-column: auto;
  grid-row: auto;
  min-height: 220px;
  justify-content: space-between;
}

.cg-feature--small h3 {
  font-size: clamp(0.95rem, 0.8rem + 0.45vw, 1.2rem);
  transition: translate 0.5s var(--cg-ease);
}

.cg-feature--small:hover h3 {
  translate: 0 -4px;
}

.cg-tag {
  display: inline-block;
  padding: 0.35rem 0.6rem;
  font-family: var(--cg-font-mono);
  font-size: 9.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  background: var(--cg-white);
  color: var(--cg-black);
  border-radius: 0.3em;
}

.cg-tag--plain {
  padding: 0;
  background: none;
}

/* ---------- Measured ---------- */

.cg-measured {
  padding-top: 0;
}

.cg-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: clamp(1rem, 0.5rem + 1.5vw, 2rem);
}

.cg-card {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 1.25rem;
  min-height: clamp(420px, 38vw, 560px);
  padding: clamp(1.25rem, 0.8rem + 1.5vw, 2.25rem);
  overflow: hidden;
}

.cg-card .cg-tag--plain {
  position: absolute;
  top: clamp(1.25rem, 0.8rem + 1.5vw, 2.25rem);
  left: clamp(1.25rem, 0.8rem + 1.5vw, 2.25rem);
}

.cg-card-rule {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    linear-gradient(currentColor, currentColor) 70% 0 / 1px 100% no-repeat,
    linear-gradient(90deg, currentColor, currentColor) 0 22% / 100% 1px no-repeat;
  opacity: 0.25;
  transition: background-position 1.2s var(--cg-ease);
}

.cg-card:hover .cg-card-rule {
  background-position:
    62% 0,
    0 30%;
}

.cg-card-big {
  position: relative;
  margin: 0;
  max-width: 14ch;
  font-size: clamp(2rem, 1rem + 3.4vw, 4.25rem);
  font-weight: 500;
  line-height: 1;
  text-transform: uppercase;
  font-variant-numeric: tabular-nums;
  transition: color 0.4s var(--cg-ease);
}

.cg-card-big--small {
  max-width: 20ch;
  font-size: clamp(1.25rem, 0.9rem + 1.2vw, 2rem);
  line-height: 1.15;
}

.cg-card-small {
  position: relative;
  max-width: 52ch;
  margin: 0;
  font-family: var(--cg-font-mono);
  font-size: 12px;
  line-height: 1.6;
}

.cg-orbit {
  position: absolute;
  top: 8%;
  left: 50%;
  width: min(62%, 340px);
  transform: translateX(-50%);
  fill: none;
  stroke: rgba(237, 223, 238, 0.5);
  stroke-width: 1;
}

.cg-orbit-rings,
.cg-orbit-sat {
  transform-box: view-box;
  transform-origin: 160px 160px;
}

.cg-orbit-rings {
  animation: cg-spin 80s linear infinite;
}

.cg-orbit-sat {
  animation: cg-spin 9s linear infinite;
}

.cg-orbit-sat--slow {
  animation-duration: 23s;
  animation-direction: reverse;
}

.cg-orbit-sat circle,
.cg-orbit-core {
  fill: var(--cg-pink);
  stroke: none;
}

.cg-orbit-sat--slow circle {
  fill: var(--cg-light-pink);
}

.cg-orbit-pill rect {
  fill: var(--cg-green);
  stroke: var(--cg-pink);
}

.cg-orbit-pill text {
  fill: var(--cg-light-pink);
  stroke: none;
  font-family: var(--cg-font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-anchor: middle;
}

.cg-card:hover .cg-card-big {
  color: var(--cg-pink-deep);
}

.cg-card.tone-green:hover .cg-card-big {
  color: var(--cg-pink);
}

.cg-card.tone-green:hover .cg-orbit-sat {
  animation-duration: 3s;
}

.cg-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: clamp(1rem, 0.5rem + 1.5vw, 2rem);
  margin: clamp(2.5rem, 2rem + 2vw, 4rem) 0 0;
}

.cg-stats > div {
  position: relative;
  padding-top: 1.25rem;
}

/* The rule above each figure draws across as it enters. */
.cg-stats > div::before {
  content: '';
  position: absolute;
  inset: 0 0 auto;
  height: 1px;
  background: rgba(34, 34, 34, 0.2);
  transform-origin: 0 50%;
}

.cg-stats dt {
  font-size: clamp(2rem, 1.2rem + 2.4vw, 3.5rem);
  font-weight: 500;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}

.cg-stats dd {
  margin: 0.9rem 0 0;
  max-width: 40ch;
  font-family: var(--cg-font-mono);
  font-size: 11.5px;
  line-height: 1.6;
  color: rgba(34, 34, 34, 0.72);
}

.cg-footnote {
  margin: 2rem 0 0;
  font-family: var(--cg-font-mono);
  font-size: 11px;
  line-height: 1.6;
  color: rgba(34, 34, 34, 0.72);
}

/* ---------- Principles ---------- */

.cg-principles {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: clamp(2rem, 1rem + 3vw, 4.5rem);
  color: var(--cg-black);
  background: var(--cg-manilla);
}

.cg-display {
  margin: 0 0 2rem;
  font-size: clamp(2.6rem, 1rem + 5vw, 5.6rem);
  font-weight: 500;
  line-height: 0.98;
  letter-spacing: -0.01em;
  text-transform: uppercase;
}

.cg-principles-copy {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1.5rem;
  max-width: 38rem;
}

.cg-principles-copy p {
  margin: 0;
  font-size: 15px;
  line-height: 1.55;
}

.cg-copy {
  padding: 0.3rem 0.65rem;
  font-family: var(--cg-font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: inherit;
  border: 1px solid rgba(237, 223, 238, 0.4);
  border-radius: 0.3em;
  transition:
    background-color 0.3s var(--cg-ease),
    color 0.3s var(--cg-ease);
}

.cg-copy:hover {
  background: var(--cg-light-pink);
  color: var(--cg-black);
}

.cg-values {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.cg-value button {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.4rem;
  width: 100%;
  padding: 1rem clamp(1rem, 0.8rem + 0.8vw, 1.5rem);
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.cg-value-title {
  font-family: var(--cg-font-mono);
  font-size: clamp(0.85rem, 0.75rem + 0.35vw, 1.05rem);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.cg-value-body {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows 0.7s var(--cg-ease);
}

.cg-value-body > div {
  overflow: hidden;
  padding: 0 clamp(1rem, 0.8rem + 0.8vw, 1.5rem);
}

.cg-value.is-open .cg-value-body {
  grid-template-rows: 1fr;
}

.cg-value-body p {
  max-width: 46ch;
  margin: clamp(3rem, 2rem + 5vw, 8rem) 0 1rem;
  font-family: var(--cg-font-mono);
  font-size: 12.5px;
  line-height: 1.65;
}

.cg-value-body a {
  display: inline-block;
  margin-bottom: 1.5rem;
  font-family: var(--cg-font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  border-bottom: 1px solid currentColor;
}

/* ---------- Corpus (green) ---------- */

.cg-corpus {
  position: relative;
  z-index: 2;
  margin-top: -1.25rem;
  color: var(--cg-light-pink);
  background: var(--cg-green);
  border-radius: 1.25rem 1.25rem 0 0;
  transform-origin: 50% 0;
}

.cg-corpus-head {
  padding-bottom: clamp(2rem, 1.5rem + 2vw, 3.5rem);
}

.cg-marquee {
  display: flex;
  overflow: hidden;
  border-block: 1px solid rgba(237, 223, 238, 0.12);
}

.cg-marquee ul {
  display: flex;
  flex: none;
  gap: clamp(3rem, 2rem + 4vw, 7rem);
  margin: 0;
  padding: 2.25rem clamp(1.5rem, 1rem + 2vw, 3.5rem);
  list-style: none;
  animation: cg-marquee 42s linear infinite;
}

.cg-marquee li {
  flex: none;
  color: #c9c6b8;
  font-size: clamp(1.1rem, 0.9rem + 0.8vw, 1.6rem);
  font-weight: 600;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  white-space: nowrap;
  transition: color 0.3s var(--cg-ease);
}

.cg-marquee li:hover {
  color: var(--cg-pink);
}

@keyframes cg-marquee {
  to {
    transform: translateX(-100%);
  }
}

/* ---------- Footer ---------- */

.cg-footer {
  position: relative;
  z-index: 2;
  color: var(--cg-light-pink);
  background: var(--cg-green);
}

.cg-dome {
  position: relative;
  height: clamp(560px, 46vw, 700px);
}

/* Centred by margins rather than a transform, so the dots can measure it. */
.cg-try {
  position: absolute;
  top: clamp(3rem, 2rem + 3vw, 5.5rem);
  left: 0;
  right: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.25rem;
  width: min(47rem, calc(100% - 2 * var(--cg-gutter)));
  margin-inline: auto;
  text-align: center;
}

.cg-try .cg-label {
  margin: 0;
  color: var(--cg-pink);
}

.cg-try-title {
  margin: 0;
  font-size: clamp(1.9rem, 1rem + 3.2vw, 3.6rem);
  font-weight: 500;
  line-height: 1;
  letter-spacing: -0.01em;
  text-transform: uppercase;
}

.cg-try-cmd {
  display: flex;
  align-items: center;
  gap: 1rem;
  width: 100%;
  padding: 0.9rem 1rem 0.9rem 1.25rem;
  text-align: left;
  background: #182317;
  border: 1px solid rgba(237, 223, 238, 0.18);
}

.cg-try-cmd pre {
  flex: 1;
  margin: 0;
  font-family: var(--cg-font-mono);
  font-size: 12.5px;
  line-height: 1.7;
  white-space: pre-wrap;
  overflow-wrap: break-word;
}

.cg-try-links {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.75rem 1.75rem;
}

.cg-footer-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 2.5rem;
  padding: clamp(3rem, 2rem + 3vw, 5rem) var(--cg-gutter) 3rem;
}

.cg-footer-grid p:not(.cg-label) {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
}

.cg-footer-grid .cg-label {
  color: var(--cg-pink);
}

.cg-footer-grid ul {
  margin: 0;
  padding: 0;
  list-style: none;
}

/* Footer links underline from the left on hover. */
.cg-footer-grid li a,
.cg-footer-base a {
  display: inline-block;
  padding: 0.15rem 0;
  background: linear-gradient(currentColor, currentColor) 0 100% / 0 1px no-repeat;
  transition:
    background-size 0.5s var(--cg-ease),
    color 0.3s var(--cg-ease);
}

.cg-footer-grid li a {
  font-size: 14px;
  font-weight: 500;
  text-transform: uppercase;
}

.cg-footer-grid li a:hover,
.cg-footer-base a:hover {
  color: var(--cg-pink);
  background-size: 100% 1px;
}

.cg-wordmark {
  display: flex;
  justify-content: space-between;
  padding: 0 var(--cg-gutter);
  color: rgba(237, 223, 238, 0.09);
  font-size: clamp(3.5rem, 0.5rem + 15.2vw, 15rem);
  font-weight: 600;
  line-height: 0.82;
  letter-spacing: -0.02em;
  user-select: none;
}

.cg-footer-base {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  padding: 1.5rem var(--cg-gutter);
  border-top: 1px solid rgba(237, 223, 238, 0.12);
  font-family: var(--cg-font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

/* ---------- Cursor ring ---------- */

.cg-cursor {
  position: fixed;
  top: 0;
  left: 0;
  z-index: 100;
  display: none;
  width: 34px;
  height: 34px;
  margin: -17px 0 0 -17px;
  border: 1.5px solid #ffffff;
  border-radius: 50%;
  pointer-events: none;
  mix-blend-mode: difference;
  opacity: 0;
  transition:
    opacity 0.3s var(--cg-ease),
    width 0.4s var(--cg-ease),
    height 0.4s var(--cg-ease),
    margin 0.4s var(--cg-ease),
    background-color 0.4s var(--cg-ease);
}

.cg-motion .cg-cursor {
  display: block;
}

.cg-cursor.is-on {
  opacity: 1;
}

.cg-cursor.is-over {
  width: 64px;
  height: 64px;
  margin: -32px 0 0 -32px;
  background: #ffffff;
}

/* ---------- Motion ----------
   Starting states exist only under .cg-motion: without JavaScript, with motion
   paused, or under prefers-reduced-motion, everything is already in place. */

.cg-motion [data-reveal] {
  opacity: 0;
  transform: translateY(36px);
  transition:
    opacity 1s var(--cg-ease) calc(var(--i, 0) * 110ms),
    transform 1.2s var(--cg-ease) calc(var(--i, 0) * 110ms);
}

.cg-motion [data-reveal].is-in,
.cg-motion [data-reveal]:focus-within {
  opacity: 1;
  transform: none;
}

/* Split words — headings and the statement — are styled in style.css: they
   are rendered inside SplitWords, where this component's scoped rules do not
   reach. */

/* The hero's two halves part and fade as the statement rises over them. */
.cg-motion .cg-hero-left {
  transform: translate3d(calc(var(--p, 0) * -24vw), 0, 0);
}

.cg-motion .cg-hero-right {
  transform: translate3d(calc(var(--p, 0) * 24vw), 0, 0);
}

/* Gone by the time the statement has covered the middle of the screen. */
.cg-motion .cg-hero-title {
  opacity: max(0, calc(1 - var(--p, 0) * 1.5));
}

.cg-motion .cg-hero-foot {
  opacity: max(0, calc(1 - var(--p, 0) * 4));
}

/* The globe pushes in as it fades (and spins faster while it does). */
.cg-motion .cg-hero :deep(.cg-globe) {
  opacity: max(0, calc(1 - var(--p, 0) * 1.2));
  transform: scale(calc(1 + var(--p, 0) * 0.3));
}

/* The manilla and green panels rise and widen into place. --rise is theirs
   alone and does not inherit (style.css), so it restyles only the panel. */
.cg-motion .cg-sheet,
.cg-motion .cg-corpus {
  transform: translate3d(0, calc((1 - var(--rise, 1)) * 12vh), 0) scale(calc(0.92 + var(--rise, 1) * 0.08));
  will-change: transform;
}

/* The accordion's open panel lifts its contents in. */
.cg-motion .cg-acc-top,
.cg-motion .cg-acc-bottom > * {
  animation: cg-rise-in 0.9s var(--cg-ease) backwards;
}

.cg-motion .cg-acc-bottom > :nth-child(1) {
  animation-delay: 0.2s;
}
.cg-motion .cg-acc-bottom > :nth-child(2) {
  animation-delay: 0.28s;
}
.cg-motion .cg-acc-bottom > :nth-child(3) {
  animation-delay: 0.36s;
}

@keyframes cg-rise-in {
  from {
    opacity: 0;
    transform: translateY(18px);
  }
}

/* Step cards follow one another in, and their icons draw themselves. */
.cg-motion .cg-steps .cg-step {
  opacity: 0;
  transform: translateY(28px);
  transition:
    opacity 0.9s var(--cg-ease) calc(var(--i) * 140ms),
    transform 1.1s var(--cg-ease) calc(var(--i) * 140ms);
}

.cg-motion .cg-steps.is-in .cg-step,
.cg-motion .cg-steps:focus-within .cg-step {
  opacity: 1;
  transform: none;
}

/* The icons build themselves shape by shape, in the order --d gives, and then
   keep an idle motion. Keyframe animations, not transitions: in testing, a
   transition on these shapes' stroke-dashoffset never started in Chromium.
   Each drawn shape carries pathLength="1". */
.cg-motion .cg-ln {
  stroke-dasharray: 1;
  stroke-dashoffset: 1;
}

.cg-motion .cg-steps.is-in .cg-ln {
  animation: cg-draw 0.9s var(--cg-ease) calc(var(--base) + var(--d, 0s)) both;
}

.cg-motion .cg-steps.is-in .cg-pop {
  animation: cg-pop 0.7s var(--cg-ease) calc(var(--base) + var(--d, 0s)) both;
}

.cg-motion .cg-steps.is-in .cg-grow-y {
  animation: cg-grow-y 0.9s var(--cg-ease) calc(var(--base) + var(--d, 0s)) both;
}

.cg-motion .cg-steps.is-in .cg-grow-0 {
  animation: cg-grow 0.8s var(--cg-ease) calc(var(--base) + var(--d, 0s)) both;
}

.cg-motion .cg-steps.is-in .cg-corner {
  animation:
    cg-corner-in 0.9s var(--cg-ease) calc(var(--base) + var(--d, 0s)) both,
    cg-breathe 3.2s ease-in-out var(--idle) infinite;
}

/* These draw like any line, then do one more thing. */
.cg-motion .cg-steps.is-in .cg-check {
  animation:
    cg-draw 0.7s var(--cg-ease) calc(var(--base) + var(--d)) both,
    cg-tick 0.6s var(--cg-ease) calc(var(--base) + var(--d) + 0.55s) both;
}

.cg-motion .cg-steps.is-in .cg-src {
  animation:
    cg-draw 0.9s var(--cg-ease) calc(var(--base) + var(--d)) both,
    cg-emit 3.2s ease-out var(--idle) infinite;
}

.cg-motion .cg-steps.is-in .cg-dst {
  animation:
    cg-draw 0.9s var(--cg-ease) calc(var(--base) + var(--d)) both,
    cg-land 3.2s ease-out var(--idle) infinite;
}

.cg-motion .cg-steps.is-in .cg-hop {
  animation: cg-hop 3.2s ease-in-out var(--idle) infinite;
}

.cg-motion .cg-steps.is-in .cg-scan {
  animation: cg-scan 3.2s ease-in-out var(--idle) infinite;
}

/* Four tokens a quarter-cycle apart, so one is always on its way down. */
.cg-motion .cg-steps.is-in .cg-tok--l {
  animation: cg-tok-l 4.8s ease-in-out var(--idle) infinite;
}
.cg-motion .cg-steps.is-in .cg-tok--ml {
  animation: cg-tok-ml 4.8s ease-in-out calc(var(--idle) + 1.2s) infinite;
}
.cg-motion .cg-steps.is-in .cg-tok--r {
  animation: cg-tok-r 4.8s ease-in-out calc(var(--idle) + 2.4s) infinite;
}
.cg-motion .cg-steps.is-in .cg-tok--mr {
  animation: cg-tok-mr 4.8s ease-in-out calc(var(--idle) + 3.6s) infinite;
}

@keyframes cg-draw {
  from {
    stroke-dashoffset: 1;
  }
  to {
    stroke-dashoffset: 0;
  }
}

@keyframes cg-pop {
  from {
    transform: scale(0);
    opacity: 0;
  }
  65% {
    transform: scale(1.18);
    opacity: 1;
  }
  to {
    transform: scale(1);
  }
}

@keyframes cg-grow-y {
  from {
    transform: scaleY(0);
  }
}

@keyframes cg-grow {
  from {
    transform: scale(0);
  }
}

@keyframes cg-corner-in {
  from {
    transform: translate(var(--cx), var(--cy));
    opacity: 0;
  }
}

@keyframes cg-breathe {
  50% {
    transform: translate(calc(var(--cx) * 0.3), calc(var(--cy) * 0.3));
  }
}

@keyframes cg-tick {
  50% {
    transform: scale(1.2);
  }
}

@keyframes cg-emit {
  0% {
    transform: scale(1.1);
  }
  18%,
  100% {
    transform: scale(1);
  }
}

@keyframes cg-land {
  0%,
  76% {
    transform: scale(1);
  }
  86% {
    transform: scale(1.14);
  }
  100% {
    transform: scale(1);
  }
}

/* From the arc's start to its end: (38, 52) to (82, 52) about (60, 61.6). */
@keyframes cg-hop {
  0% {
    transform: rotate(23.6deg);
    opacity: 0;
  }
  8% {
    opacity: 1;
  }
  78% {
    transform: rotate(156.4deg);
    opacity: 1;
  }
  90%,
  100% {
    transform: rotate(156.4deg);
    opacity: 0;
  }
}

@keyframes cg-scan {
  0% {
    transform: translateY(0);
    opacity: 0;
  }
  12%,
  88% {
    opacity: 0.8;
  }
  100% {
    transform: translateY(52px);
    opacity: 0;
  }
}

/* Root (60, 26) → trunk → branch → child box, which it lights as it lands. */
@keyframes cg-tok-l {
  0% {
    transform: translate(0, 0) scale(1);
    opacity: 0;
  }
  8% {
    opacity: 1;
  }
  30% {
    transform: translate(0, 16px);
  }
  58% {
    transform: translate(-36px, 16px);
  }
  78% {
    transform: translate(-36px, 30px) scale(1);
    opacity: 1;
  }
  92%,
  100% {
    transform: translate(-36px, 30px) scale(2.6);
    opacity: 0;
  }
}

@keyframes cg-tok-r {
  0% {
    transform: translate(0, 0) scale(1);
    opacity: 0;
  }
  8% {
    opacity: 1;
  }
  30% {
    transform: translate(0, 16px);
  }
  58% {
    transform: translate(36px, 16px);
  }
  78% {
    transform: translate(36px, 30px) scale(1);
    opacity: 1;
  }
  92%,
  100% {
    transform: translate(36px, 30px) scale(2.6);
    opacity: 0;
  }
}

/* Through the middle child to a leaf. */
@keyframes cg-tok-ml {
  0% {
    transform: translate(0, 0) scale(1);
    opacity: 0;
  }
  6% {
    opacity: 1;
  }
  45% {
    transform: translate(0, 56px);
  }
  62% {
    transform: translate(-16px, 56px);
  }
  78% {
    transform: translate(-16px, 68px) scale(1);
    opacity: 1;
  }
  92%,
  100% {
    transform: translate(-16px, 68px) scale(2.6);
    opacity: 0;
  }
}

@keyframes cg-tok-mr {
  0% {
    transform: translate(0, 0) scale(1);
    opacity: 0;
  }
  6% {
    opacity: 1;
  }
  45% {
    transform: translate(0, 56px);
  }
  62% {
    transform: translate(16px, 56px);
  }
  78% {
    transform: translate(16px, 68px) scale(1);
    opacity: 1;
  }
  92%,
  100% {
    transform: translate(16px, 68px) scale(2.6);
    opacity: 0;
  }
}

/* The statement: its box opens outward from the centre, the four edges draw
   clockwise, the numbers fade up, and the words rise (after --split-delay). */
.cg-motion .cg-creed {
  clip-path: inset(50% 50% 50% 50%);
}

.cg-motion .cg-creed.is-in {
  clip-path: inset(0 0 0 0);
  transition: clip-path 1s var(--cg-ease) 0.1s;
}

.cg-motion .cg-edge--t,
.cg-motion .cg-edge--b {
  transform: scaleX(0);
}

.cg-motion .cg-edge--r,
.cg-motion .cg-edge--l {
  transform: scaleY(0);
}

.cg-motion .cg-creed.is-in .cg-edge {
  transform: none;
  transition: transform 0.5s var(--cg-ease);
}

.cg-motion .cg-creed.is-in .cg-edge--t {
  transition-delay: 0.55s;
}
.cg-motion .cg-creed.is-in .cg-edge--r {
  transition-delay: 0.75s;
}
.cg-motion .cg-creed.is-in .cg-edge--b {
  transition-delay: 0.95s;
}
.cg-motion .cg-creed.is-in .cg-edge--l {
  transition-delay: 1.15s;
}

.cg-motion .cg-creed .cg-index {
  opacity: 0;
}

.cg-motion .cg-creed.is-in .cg-index {
  opacity: 1;
  transition: opacity 0.6s ease calc(0.9s + var(--l) * 0.15s);
}

/* The terminal types its output one line at a time. */
.cg-motion .cg-terminal .cg-tl {
  clip-path: inset(0 100% 0 0);
  transition: clip-path 0.42s steps(28, end) calc(0.35s + var(--l) * 60ms);
}

.cg-motion .cg-terminal.is-in .cg-tl {
  clip-path: inset(0 0 0 0);
}

.cg-motion .cg-terminal .cg-caret {
  opacity: 0;
}

/* Four blinks and done: nothing blinks for more than five seconds. */
.cg-motion .cg-terminal.is-in .cg-caret {
  animation: cg-blink 1s steps(1, end) calc(0.35s + var(--l) * 60ms) 4 forwards;
}

@keyframes cg-blink {
  0% {
    opacity: 1;
  }
  50% {
    opacity: 0;
  }
  100% {
    opacity: 1;
  }
}

/* The rule above each figure draws across. */
.cg-motion .cg-stats > div::before {
  transform: scaleX(0);
  transition: transform 1.2s var(--cg-ease) calc(0.2s + var(--i, 0) * 110ms);
}

.cg-motion .cg-stats > div.is-in::before {
  transform: none;
}

/* Principles slide in one after another. */
.cg-motion .cg-values .cg-value {
  opacity: 0;
  transform: translateX(32px);
  transition:
    opacity 0.8s var(--cg-ease) calc(var(--i) * 90ms),
    transform 1s var(--cg-ease) calc(var(--i) * 90ms);
}

.cg-motion .cg-values.is-in .cg-value,
.cg-motion .cg-values:focus-within .cg-value {
  opacity: 1;
  transform: none;
}

/* Menu links cascade in each time it opens. */
.cg-motion .cg-menu-list li {
  animation: cg-rise-in 0.9s var(--cg-ease) backwards;
  animation-delay: calc(0.15s + var(--i) * 55ms);
}

.cg-motion .cg-menu-aside > div {
  animation: cg-rise-in 0.9s 0.5s var(--cg-ease) backwards;
}

/* Paused: every looping animation holds where it is. */
.cg.is-still .cg-marquee ul,
.cg.is-still .cg-badge > svg:first-child,
.cg.is-still .cg-orbit-rings,
.cg.is-still .cg-orbit-sat {
  animation-play-state: paused;
}

/* A [data-idle] element off screen (idleOffscreen in motion.ts): its looping
   animations wait, rather than costing a frame's work no one can see. */
.is-offscreen,
.is-offscreen :deep(*) {
  animation-play-state: paused !important;
}

.cg-marquee:hover ul {
  animation-play-state: paused;
}

/* ---------- Narrow screens ---------- */

@media (max-width: 960px) {
  .cg-intro,
  .cg-principles,
  .cg-cards {
    grid-template-columns: 1fr;
  }

  .cg-badge {
    position: relative;
    inset: auto;
    width: 104px;
    height: 104px;
  }

  .cg-steps,
  .cg-stats {
    grid-template-columns: 1fr;
  }

  .cg-step {
    min-height: 0;
  }

  .cg-step + .cg-step {
    border-left: 0;
    border-top: 1px solid rgba(34, 34, 34, 0.18);
  }

  .cg-step-icon {
    width: 88px;
  }

  .cg-proof-stage {
    grid-template-rows: auto auto auto;
  }

  .cg-terminal {
    grid-row: 1;
    padding-bottom: clamp(1.25rem, 0.6rem + 2.4vw, 3rem);
  }

  .cg-feature,
  .cg-feature-row {
    grid-column: 1 / -1;
  }

  .cg-feature {
    grid-row: 2;
  }

  .cg-feature-row {
    grid-row: 3;
  }

  .cg-footer-grid {
    grid-template-columns: 1fr 1fr;
  }

  .cg-menu {
    grid-template-columns: 1fr;
    align-content: start;
  }

  /* Side by side, the open panel needs 560px beside four 64px strips; below
     this width it would be clipped, so the surfaces stack instead. */
  .cg-acc {
    flex-direction: column;
    height: auto;
  }

  .cg-acc-item,
  .cg-acc-item.is-open,
  .cg-acc-item:not(.is-open):hover {
    flex: none;
  }

  .cg-acc-strip {
    position: relative;
    flex-direction: row;
    width: 100%;
    padding: 1rem 1.1rem;
  }

  .cg-acc-strip i {
    order: 1;
  }

  .cg-acc-strip-label {
    writing-mode: horizontal-tb;
    transform: none;
  }

  .is-open > .cg-acc-strip {
    display: none;
  }

  .cg-acc-panel {
    position: relative;
    min-width: 0;
    min-height: 360px;
    gap: 3rem;
  }
}

@media (max-width: 760px) {
  .cg-head {
    flex-direction: column;
    align-items: flex-start;
  }

  .cg-hero-foot span:nth-child(2) {
    display: none;
  }

  /* The toggle keeps its name for assistive technology, and shows its icon. */
  .cg-motion-toggle span {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .cg-feature-row {
    grid-template-columns: 1fr;
  }

  .cg-footer-grid {
    grid-template-columns: 1fr;
  }

  .cg-footer-base {
    flex-direction: column;
  }
}

@media (prefers-reduced-motion: reduce) {
  .cg *,
  .cg *::before,
  .cg *::after {
    animation: none !important;
    transition: none !important;
  }
}
</style>
