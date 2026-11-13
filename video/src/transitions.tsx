// Custom scene transitions.
//   iris      — the next scene opens as a circle growing from a point.
//   globeZoom — the camera flies into a point (the globe's centre) while the
//               next scene opens from it, as if on the far side.
//   halftone  — the next scene arrives as dots that swell across the frame
//               in a sweep, the site's halftone fields made into a wipe.

import React, { useMemo } from 'react'
import { AbsoluteFill } from 'remotion'
import type { TransitionPresentation, TransitionPresentationComponentProps } from '@remotion/transitions'

type Point = { x: number; y: number }
const W = 1920
const H = 1080
const FAR = Math.hypot(W, H)

const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2)

// ─── iris ──────────────────────────────────────────────────────────────────
type IrisProps = Point & { ring?: string }
const Iris: React.FC<TransitionPresentationComponentProps<IrisProps>> = ({ children, presentationDirection, presentationProgress, passedProps }) => {
  const p = ease(presentationProgress)
  if (presentationDirection === 'exiting') {
    return <AbsoluteFill style={{ transform: `scale(${1 + 0.06 * p})`, transformOrigin: `${passedProps.x}px ${passedProps.y}px` }}>{children}</AbsoluteFill>
  }
  const r = p * FAR
  return (
    <AbsoluteFill>
      <AbsoluteFill style={{ clipPath: `circle(${r}px at ${passedProps.x}px ${passedProps.y}px)` }}>{children}</AbsoluteFill>
      {passedProps.ring && p < 1 && (
        <svg width={W} height={H} style={{ position: 'absolute', inset: 0 }}>
          <circle cx={passedProps.x} cy={passedProps.y} r={r} fill="none" stroke={passedProps.ring} strokeWidth={6} opacity={1 - p} />
        </svg>
      )}
    </AbsoluteFill>
  )
}
export const iris = (props: IrisProps): TransitionPresentation<IrisProps> => ({ component: Iris, props })

// ─── globeZoom ─────────────────────────────────────────────────────────────
const GlobeZoom: React.FC<TransitionPresentationComponentProps<Point>> = ({ children, presentationDirection, presentationProgress, passedProps }) => {
  const { x, y } = passedProps
  if (presentationDirection === 'exiting') {
    const p = Math.pow(presentationProgress, 2.2)
    return (
      <AbsoluteFill style={{ transform: `scale(${1 + 7 * p})`, transformOrigin: `${x}px ${y}px`, opacity: 1 - Math.max(0, (presentationProgress - 0.55) / 0.45), filter: `blur(${p * 8}px)` }}>
        {children}
      </AbsoluteFill>
    )
  }
  const p = ease(Math.max(0, (presentationProgress - 0.25) / 0.75))
  return (
    <AbsoluteFill style={{ clipPath: `circle(${p * FAR}px at ${x}px ${y}px)`, transform: `scale(${1.35 - 0.35 * p})`, transformOrigin: `${x}px ${y}px` }}>
      {children}
    </AbsoluteFill>
  )
}
export const globeZoom = (props: Point): TransitionPresentation<Point> => ({ component: GlobeZoom, props })

// ─── halftone ──────────────────────────────────────────────────────────────
type HalftoneProps = { from: 'left' | 'right' | 'top' | 'bottom'; cell?: number }
const Halftone: React.FC<TransitionPresentationComponentProps<HalftoneProps>> = ({ children, presentationDirection, presentationProgress, passedProps }) => {
  const cell = passedProps.cell ?? 54
  const cols = Math.ceil(W / cell)
  const rows = Math.ceil(H / cell)
  const p = presentationProgress
  const mask = useMemo(() => {
    if (presentationDirection === 'exiting') return ''
    let dots = ''
    const full = (cell * Math.SQRT2) / 2 + 1
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        // Position along the sweep, 0 where it starts.
        const u =
          passedProps.from === 'left' ? c / cols : passedProps.from === 'right' ? 1 - c / cols : passedProps.from === 'top' ? r / rows : 1 - r / rows
        const local = Math.max(0, Math.min(1, (p * 1.6 - u * 0.6) / 1))
        const rad = full * ease(local)
        if (rad > 0.2) dots += `<circle cx='${c * cell + cell / 2}' cy='${r * cell + cell / 2}' r='${rad.toFixed(1)}'/>`
      }
    }
    const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${W}' height='${H}'><g fill='black'>${dots}</g></svg>`
    return `url("data:image/svg+xml;utf8,${encodeURIComponent(svg)}")`
  }, [p, presentationDirection, cols, rows, cell, passedProps.from])
  if (presentationDirection === 'exiting') return <AbsoluteFill>{children}</AbsoluteFill>
  return (
    <AbsoluteFill style={{ WebkitMaskImage: mask, maskImage: mask, WebkitMaskSize: `${W}px ${H}px`, maskSize: `${W}px ${H}px` }}>
      {children}
    </AbsoluteFill>
  )
}
export const halftone = (props: HalftoneProps): TransitionPresentation<HalftoneProps> => ({ component: Halftone, props })
