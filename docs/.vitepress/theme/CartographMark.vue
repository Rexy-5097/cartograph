<script setup lang="ts">
// Cartograph's mark: a square node and a round node, two languages, joined by
// an edge that bends into a check, inside a surveyor's brackets. An edge
// across languages, with the proof behind it.
//
// It builds itself once when it first appears: the brackets close in, the
// nodes appear, the edge is drawn. Hovering or focusing the link that holds
// it closes the brackets tighter, redraws the edge and sends a packet along
// it to the round node. With `idle`, a packet also travels the edge every few
// seconds; that is only for pages with a pause control (WCAG 2.2.2), which the
// landing page's header has and the documentation pages do not. With
// `loading`, once built it sends packet after packet: the landing page's
// loading screen, which lasts a few seconds and any input skips.
//
// SVG and CSS, coloured by `currentColor`. The idle packet is a short
// animation a timer starts, not an endless one: an endless CSS animation of
// an SVG's parts runs on the main thread every frame, even while it holds
// still between crossings. All of it stops under prefers-reduced-motion and
// the landing page's "Pause motion" (`.is-still`), leaving the finished mark.

import { onBeforeUnmount, onMounted, ref } from 'vue'

const props = withDefaults(defineProps<{ idle?: boolean; loading?: boolean }>(), { idle: false, loading: false })

// The idle packet: the first after `first` ms, then one every `every` ms,
// each crossing taking `crossing` ms.
const IDLE = { first: 2500, every: 8000, crossing: 1300 }
const sending = ref(false)
let timers: number[] = []

onMounted(() => {
  if (!props.idle || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
  const send = () => {
    sending.value = true
    timers.push(window.setTimeout(() => (sending.value = false), IDLE.crossing))
  }
  timers.push(
    window.setTimeout(() => {
      send()
      timers.push(window.setInterval(send, IDLE.every))
    }, IDLE.first),
  )
})

onBeforeUnmount(() => {
  timers.forEach((t) => {
    clearTimeout(t)
    clearInterval(t)
  })
  timers = []
})

// The edge, from the square's right side, down and up into the round node
// (aimed at its centre, ending on its outline). pathLength makes dash lengths
// percentages of it.
const EDGE = 'M18.5 28.5 23.5 33.5 32.7 18.3'
</script>

<template>
  <svg class="cg-mark" :class="{ 'is-sending': sending, 'is-loading': loading }" viewBox="0 0 48 48" aria-hidden="true" focusable="false">
    <g class="cg-mark-build is-tl"><path class="cg-mark-corner is-tl" d="M4 14V4h10" /></g>
    <g class="cg-mark-build is-tr"><path class="cg-mark-corner is-tr" d="M34 4h10v10" /></g>
    <g class="cg-mark-build is-br"><path class="cg-mark-corner is-br" d="M44 34v10H34" /></g>
    <g class="cg-mark-build is-bl"><path class="cg-mark-corner is-bl" d="M14 44H4V34" /></g>
    <g class="cg-mark-pop is-square"><rect x="10.5" y="24.5" width="8" height="8" /></g>
    <g class="cg-mark-pop is-round"><circle class="cg-mark-target" cx="35" cy="14.5" r="4.4" /></g>
    <g class="cg-mark-draw"><path class="cg-mark-edge" :d="EDGE" pathLength="100" /></g>
    <path class="cg-mark-packet" :d="EDGE" pathLength="100" />
  </svg>
</template>

<style scoped>
.cg-mark {
  --cg-mark-ease: cubic-bezier(0.22, 1, 0.36, 1);
  display: block;
  flex: none;
  width: 1em;
  height: 1em;
  overflow: visible;
  fill: none;
  stroke: currentColor;
  stroke-width: 2.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

/* Each bracket closes toward the centre: from (--out) when built, to (--in)
   when its link is hovered or focused. */
.is-tl { --out-x: -5px; --out-y: -5px; --in-x: 2.5px; --in-y: 2.5px; }
.is-tr { --out-x: 5px; --out-y: -5px; --in-x: -2.5px; --in-y: 2.5px; }
.is-br { --out-x: 5px; --out-y: 5px; --in-x: -2.5px; --in-y: -2.5px; }
.is-bl { --out-x: -5px; --out-y: 5px; --in-x: 2.5px; --in-y: -2.5px; }

.cg-mark-corner {
  transition: transform 0.5s var(--cg-mark-ease);
}

.cg-mark-pop,
.cg-mark-target {
  transform-box: fill-box;
  transform-origin: center;
}

.cg-mark-edge {
  stroke-dasharray: 100;
}

/* The packet: a dot riding the edge, hidden until it is sent. */
.cg-mark-packet {
  stroke-width: 5.5;
  stroke-dasharray: 0.1 200;
  opacity: 0;
}

@media (prefers-reduced-motion: no-preference) {
  /* Built once. Each animation fills backwards only, so once it ends the
     hover styles below take over. */
  .cg-mark-build {
    animation: cg-mark-close 0.9s var(--cg-mark-ease) 0.1s backwards;
  }

  .cg-mark-pop.is-square {
    animation: cg-mark-appear 0.5s var(--cg-mark-ease) 0.45s backwards;
  }

  .cg-mark-pop.is-round {
    animation: cg-mark-appear 0.5s var(--cg-mark-ease) 0.55s backwards;
  }

  /* The edge's offset is inherited from this group while it is built, and
     overridden by the path itself when it is redrawn on hover. */
  .cg-mark-draw {
    animation: cg-mark-stroke 0.7s var(--cg-mark-ease) 0.7s backwards;
  }

  /* An idle crossing, started by the timer above: the same packet and answer
     as on hover. */
  .is-sending .cg-mark-packet {
    animation: cg-mark-send 0.8s ease-in-out 0.35s backwards;
  }

  .is-sending .cg-mark-target {
    animation: cg-mark-ping 1.2s ease-out;
  }

  .is-loading .cg-mark-packet {
    animation: cg-mark-send 1.1s ease-in-out 1.3s infinite;
  }

  .is-loading .cg-mark-target {
    animation: cg-mark-ping 1.1s ease-out 1.3s infinite;
  }

  :is(a:hover, a:focus-visible) .cg-mark-corner {
    transform: translate(var(--in-x), var(--in-y));
  }

  :is(a:hover, a:focus-visible) .cg-mark-edge {
    animation: cg-mark-stroke 0.6s var(--cg-mark-ease);
  }

  :is(a:hover, a:focus-visible) .cg-mark-packet {
    animation: cg-mark-send 0.8s ease-in-out 0.35s backwards;
  }

  :is(a:hover, a:focus-visible) .cg-mark-target {
    animation: cg-mark-ping 1.2s ease-out;
  }
}

/* "Pause motion" on the landing page: the finished mark, and nothing moves. */
.is-still .cg-mark-build,
.is-still .cg-mark-pop,
.is-still .cg-mark-draw,
.is-still .cg-mark-edge,
.is-still .cg-mark-target,
.is-still .cg-mark-packet {
  animation: none !important;
}

.is-still .cg-mark-corner {
  transform: none !important;
  transition: none;
}

@keyframes cg-mark-close {
  from {
    transform: translate(var(--out-x), var(--out-y));
    opacity: 0;
  }
}

@keyframes cg-mark-appear {
  from {
    transform: scale(0);
  }
  70% {
    transform: scale(1.18);
  }
}

@keyframes cg-mark-stroke {
  from {
    stroke-dashoffset: 100;
  }
}

@keyframes cg-mark-send {
  from {
    stroke-dashoffset: 0;
    opacity: 1;
  }
  90% {
    stroke-dashoffset: -100;
    opacity: 1;
  }
  to {
    stroke-dashoffset: -100;
    opacity: 0;
  }
}

@keyframes cg-mark-ping {
  0%,
  80% {
    transform: scale(1);
  }
  90% {
    transform: scale(1.28);
  }
  100% {
    transform: scale(1);
  }
}
</style>
