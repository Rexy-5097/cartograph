<script setup lang="ts">
// The real trace from the README — Apache Airflow at 9b43d6a — drawn as a
// chain: a TypeScript function, the Python handler it reaches over HTTP, the
// model that handler uses, and the table behind it. Each hop's edge kind,
// confidence, provenance and evidence are verbatim from that output, with
// directory paths shortened to "…" as the README shortens them.
//
// It builds once when it comes into view. With `loop` (the landing page,
// which has a pause control) packets keep travelling the edges and the
// evidence below steps from hop to hop until the reader picks one; elsewhere
// a packet crosses each edge once and the chain rests. Nothing moves under
// prefers-reduced-motion, with `paused`, or before JavaScript runs.

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { prefersReduced, useInView } from './shared'

const props = withDefaults(defineProps<{ loop?: boolean; paused?: boolean }>(), { loop: false, paused: false })

const NODES = [
  { lang: 'TypeScript', kind: 'function', name: 'enqueueConnectionTest', at: '…/requests/services.gen.ts:834' },
  { lang: 'Python', kind: 'function', name: 'enqueue_connection_test', at: '…/routes/public/connections.py:383' },
  { lang: 'Python', kind: 'class', name: 'ConnectionTestRequest', at: '…/airflow/models/connection_test.py:82' },
  { lang: 'Database', kind: 'table', name: 'connection_test_request', at: '' },
]
const EDGES = [
  {
    kind: 'HTTP-CALL',
    confidence: '0.98',
    provenance: 'route-matcher',
    evidence: 'POST /api/v2/connections/enqueue-test matched POST /api/v2/connections/enqueue-test at …/routes/public/connections.py:383 (enqueue_connection_test); exact HTTP method; every path segment static and equal',
  },
  { kind: 'ORM-ACCESS', confidence: '0.80', provenance: 'orm-resolution', evidence: 'ConnectionTestRequest(...) in enqueue_connection_test at …/connections.py:427' },
  { kind: 'QUERIES', confidence: '0.80', provenance: 'orm-resolution', evidence: 'ConnectionTestRequest maps to table "connection_test_request"' },
]

const root = ref<HTMLElement | null>(null)
const { seen, visible } = useInView(root)
const armed = ref(false)
const reduced = ref(false)
const picked = ref(0)
const touched = ref(false)
const looping = computed(() => props.loop && armed.value && seen.value && visible.value && !props.paused && !reduced.value)

onMounted(() => {
  reduced.value = prefersReduced()
  armed.value = !reduced.value
})

// On the landing page the evidence steps from hop to hop, until the reader chooses.
let timer = 0
watch(
  looping,
  (on) => {
    clearInterval(timer)
    if (on && !touched.value) timer = window.setInterval(() => (picked.value = (picked.value + 1) % EDGES.length), 4200)
  },
  { immediate: false },
)
onBeforeUnmount(() => clearInterval(timer))

function pick(i: number) {
  touched.value = true
  clearInterval(timer)
  picked.value = i
}
</script>

<template>
  <figure ref="root" class="tc" :class="{ 'is-armed': armed, 'is-in': !armed || seen, 'is-looping': looping }">
    <ol class="tc-row">
      <template v-for="(n, i) in NODES" :key="n.name">
        <li class="tc-node" :class="{ 'is-end': i === NODES.length - 1 }" :style="{ '--i': i * 2 }">
          <span class="tc-lang">{{ n.lang }}</span>
          <span class="tc-name">{{ n.name }}</span>
          <span class="tc-kind">{{ n.kind }}</span>
          <span v-if="n.at" class="tc-at">{{ n.at }}</span>
        </li>
        <li v-if="i < EDGES.length" class="tc-edge" :style="{ '--i': i * 2 + 1 }">
          <button type="button" class="tc-edge-label" :aria-pressed="picked === i" @click="pick(i)">
            <span class="tc-edge-kind">↓ {{ EDGES[i].kind }}</span>
            <span class="tc-edge-meta">{{ EDGES[i].confidence }}<br />{{ EDGES[i].provenance }}</span>
          </button>
          <span class="tc-line" aria-hidden="true"><i class="tc-packet" /></span>
        </li>
      </template>
    </ol>
    <div class="tc-evidence" aria-live="polite">
      <span class="tc-evidence-label">Evidence · {{ EDGES[picked].kind }}</span>
      <p :key="picked">{{ EDGES[picked].evidence }}</p>
    </div>
    <figcaption>
      Real output of <code>cartograph trace enqueueConnectionTest</code> on Apache Airflow at <code>9b43d6a</code>. Directory paths
      are shortened to “…”; everything else is verbatim. Select a hop to read its evidence.
    </figcaption>
  </figure>
</template>

<style scoped>
.tc {
  /* A host can fix the accent with --graphic-accent; otherwise it follows the theme. */
  --tc-accent: var(--graphic-accent, var(--cg-pink-deep, #8a4a79));
  --tc-rule: color-mix(in srgb, currentColor 18%, transparent);
  --tc-muted: color-mix(in srgb, currentColor 62%, transparent);
  container-type: inline-size;
  margin: 2rem 0;
}

:global(.dark) .tc {
  --tc-accent: var(--graphic-accent, var(--cg-pink, #cd8dbd));
}

.tc-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(6.5rem, 0.55fr) minmax(0, 1fr) minmax(6.5rem, 0.55fr) minmax(0, 1fr) minmax(6.5rem, 0.55fr) minmax(0, 1fr);
  align-items: stretch;
  margin: 0;
  padding: 0;
  list-style: none;
}

.tc-node {
  display: grid;
  align-content: start;
  gap: 0.4rem;
  min-width: 0;
  margin: 0;
  padding: 1rem;
  border: 1px solid var(--tc-rule);
  border-radius: 0.6rem;
}

.tc-node.is-end {
  border-color: var(--tc-accent);
}

.tc-lang {
  font-family: var(--cg-font-mono, monospace);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--tc-accent);
}

.tc-name {
  margin-top: 0.35rem;
  font-family: var(--cg-font-mono, monospace);
  font-size: 13px;
  font-weight: 600;
  overflow-wrap: anywhere;
}

.tc-kind,
.tc-at {
  font-family: var(--cg-font-mono, monospace);
  font-size: 11px;
  color: var(--tc-muted);
  overflow-wrap: anywhere;
}

.tc-edge {
  position: relative;
  display: grid;
  align-content: center;
  margin: 0;
  padding: 0 0.35rem;
}

.tc-edge-label {
  display: grid;
  justify-items: center;
  gap: 0.2rem;
  margin: 0 0 0.6rem;
  padding: 0.25rem;
  border: 0;
  border-radius: 0.3rem;
  font: inherit;
  color: inherit;
  background: none;
  cursor: pointer;
}

.tc-edge-kind {
  font-family: var(--cg-font-mono, monospace);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--tc-accent);
  white-space: nowrap;
}

.tc-edge-meta {
  font-family: var(--cg-font-mono, monospace);
  font-size: 10px;
  color: var(--tc-muted);
  text-align: center;
}

.tc-edge-label[aria-pressed='true'] .tc-edge-kind {
  text-decoration: underline;
  text-underline-offset: 3px;
}

.tc-line {
  position: relative;
  display: block;
  height: 2px;
  background: var(--tc-accent);
  transform-origin: 0 50%;
}

.tc-packet {
  position: absolute;
  top: -4px;
  left: 0;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--tc-accent);
  opacity: 0;
}

.tc-evidence {
  margin-top: 1.25rem;
  padding: 1rem 1.1rem;
  border-left: 2px solid var(--tc-accent);
  background: color-mix(in srgb, currentColor 4%, transparent);
}

.tc-evidence-label {
  font-family: var(--cg-font-mono, monospace);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--tc-accent);
}

.tc-evidence p {
  margin: 0.4rem 0 0;
  font-family: var(--cg-font-mono, monospace);
  font-size: 12.5px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.tc figcaption {
  margin-top: 0.75rem;
  font-size: 12.5px;
  line-height: 1.5;
  color: var(--tc-muted);
}

.tc figcaption code {
  font-size: 0.95em;
}

/* ---------- Building in: only once JavaScript has armed it ---------- */
.tc.is-armed .tc-node,
.tc.is-armed .tc-edge-label {
  opacity: 0;
  transform: translateY(12px);
  transition:
    opacity 0.6s var(--cg-ease, ease) calc(var(--i) * 0.16s),
    transform 0.6s var(--cg-ease, ease) calc(var(--i) * 0.16s);
}

.tc.is-armed .tc-line {
  transform: scaleX(0);
  transition: transform 0.7s var(--cg-ease, ease) calc(var(--i) * 0.16s + 0.1s);
}

.tc.is-armed.is-in .tc-node,
.tc.is-armed.is-in .tc-edge-label {
  opacity: 1;
  transform: none;
}

.tc.is-armed.is-in .tc-line {
  transform: none;
}

/* One crossing per edge after it is drawn; with `loop`, endlessly. */
.tc.is-armed.is-in .tc-packet {
  animation: tc-packet 1.6s var(--cg-ease, ease) calc(var(--i) * 0.16s + 0.9s) 1;
}

.tc.is-looping .tc-packet {
  animation: tc-packet 1.6s var(--cg-ease, ease) calc(var(--i) * 0.3s) infinite;
}

.tc-evidence p {
  animation: tc-fade 0.45s var(--cg-ease, ease);
}

@keyframes tc-packet {
  0% {
    left: 0;
    opacity: 0;
  }
  12%,
  88% {
    opacity: 1;
  }
  100% {
    left: calc(100% - 10px);
    opacity: 0;
  }
}

@keyframes tc-fade {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
}

/* ---------- A column on narrow widths ---------- */
@container (max-width: 760px) {
  .tc-row {
    grid-template-columns: minmax(0, 1fr);
  }

  .tc-node {
    grid-template-columns: auto 1fr;
    align-items: baseline;
    gap: 0.15rem 0.75rem;
    padding: 0.75rem 0.9rem;
  }

  .tc-lang {
    grid-row: span 3;
    min-width: 5.5rem;
  }

  .tc-name {
    margin-top: 0;
  }

  .tc-edge {
    grid-template-columns: 2px auto;
    justify-content: start;
    align-items: center;
    gap: 1rem;
    min-height: 3.5rem;
    padding: 0 0 0 1.25rem;
  }

  .tc-edge-label {
    order: 2;
    justify-items: start;
    margin: 0;
  }

  .tc-line {
    order: 1;
    width: 2px;
    height: 100%;
    transform-origin: 50% 0;
  }

  .tc.is-armed .tc-line {
    transform: scaleY(0);
  }

  .tc-edge-meta {
    text-align: left;
  }

  .tc-packet {
    top: 0;
    left: -4px;
  }

  .tc.is-armed.is-in .tc-packet,
  .tc.is-looping .tc-packet {
    animation-name: tc-packet-down;
  }
}

@keyframes tc-packet-down {
  0% {
    top: 0;
    opacity: 0;
  }
  12%,
  88% {
    opacity: 1;
  }
  100% {
    top: calc(100% - 10px);
    opacity: 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .tc-evidence p {
    animation: none;
  }
}
</style>
