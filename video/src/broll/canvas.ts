import { useLayoutEffect, useRef } from 'react'

/**
 * A full-frame canvas redrawn on every render — once per video frame. Drawing
 * in a layout effect finishes before Remotion captures the frame.
 */
export const useCanvas = (draw: (g: CanvasRenderingContext2D) => void, width = 1920, height = 1080) => {
  const ref = useRef<HTMLCanvasElement>(null)
  useLayoutEffect(() => {
    const c = ref.current
    const g = c?.getContext('2d')
    if (!c || !g) return
    g.setTransform(1, 0, 0, 1, 0, 0)
    g.clearRect(0, 0, width, height)
    draw(g)
  })
  return ref
}

export const clamp01 = (x: number) => Math.max(0, Math.min(1, x))
export const lerp = (a: number, b: number, t: number) => a + (b - a) * t
/** The site's --cg-ease, as a plain function for per-point use. */
export const easeOut = (t: number) => 1 - Math.pow(1 - clamp01(t), 3)
export const easeInOut = (t: number) => {
  const x = clamp01(t)
  return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2
}
