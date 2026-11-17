// Shared plumbing for the site's moving graphics (TraceChain, EdgeField,
// OriginStory). Each one renders its finished state on the server and without
// JavaScript; motion is added on the client, only while the graphic is on
// screen, and never under prefers-reduced-motion.

import { onBeforeUnmount, onMounted, ref, type Ref } from 'vue'

export const prefersReduced = () => typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches

/**
 * `seen` turns true the first time the element's top has risen a quarter of
 * the way up the viewport, and stays true — a measure that works for a tall
 * graphic on a short screen, which may never be a fixed fraction on screen.
 * `visible` follows whether any of it is on screen now, for pausing loops.
 */
export function useInView(el: Ref<HTMLElement | null>) {
  const seen = ref(false)
  const visible = ref(false)
  const observers: IntersectionObserver[] = []
  onMounted(() => {
    const node = el.value
    if (!node) return
    const enter = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          seen.value = true
          enter.disconnect()
        }
      },
      { rootMargin: '0px 0px -25% 0px' },
    )
    const onScreen = new IntersectionObserver((entries) => {
      for (const e of entries) visible.value = e.isIntersecting
    })
    enter.observe(node)
    onScreen.observe(node)
    observers.push(enter, onScreen)
  })
  onBeforeUnmount(() => observers.forEach((o) => o.disconnect()))
  return { seen, visible }
}

export const clamp01 = (x: number) => Math.min(1, Math.max(0, x))
/** Where `x` sits between `a` and `b`, as 0–1. */
export const span = (x: number, a: number, b: number) => clamp01((x - a) / (b - a))
export const easeOut = (t: number) => 1 - Math.pow(1 - clamp01(t), 3)
export const easeInOut = (t: number) => {
  const x = clamp01(t)
  return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2
}
