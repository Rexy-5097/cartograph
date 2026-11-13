// 14,932 dots: one for every edge labelled in the M08 run. They fill in a
// sweep that the counter beside them follows exactly; one is marked as the
// single false positive; then 3,711 — the 24.9% that could not be verified
// from source — go hollow, gathered where evidence is weakest.

import React, { useMemo } from 'react'
import { random, useCurrentFrame } from 'remotion'
import { clamp01, easeOut, useCanvas } from './canvas'

export const EDGES = 14932
export const UNVERIFIED = 3711
export const GRID = { x: 870, y: 150, w: 920, h: 780 }
const COLS = Math.ceil(Math.sqrt((EDGES * GRID.w) / GRID.h))
const STEP = GRID.w / COLS
const ROWS = Math.ceil(EDGES / COLS)
// The false positive's place in the grid.
const FP = { col: Math.floor(COLS * 0.38), row: Math.floor(ROWS * 0.31) }
export const FP_POINT = { x: GRID.x + (FP.col + 0.5) * STEP, y: GRID.y + (FP.row + 0.5) * STEP }

type Props = { fill: [number, number]; mark: number; hollow: [number, number]; ink: string; accent: string; dim: string }

/** How many dots the sweep has placed by frame f. */
export const placed = (f: number, fill: [number, number]) => {
  const t = clamp01((f - fill[0]) / fill[1])
  return Math.round(EDGES * easeOut(t))
}

export const DotGrid: React.FC<Props> = ({ fill, mark, hollow, ink, accent, dim }) => {
  const f = useCurrentFrame()
  // Dots in sweep order (a diagonal from the top left, with a little noise),
  // and a weakness score that decides which go hollow.
  const order = useMemo(() => {
    const ids = Array.from({ length: EDGES }, (_, i) => i)
    const key = ids.map((i) => {
      const c = i % COLS
      const r = Math.floor(i / COLS)
      return c / COLS + r / ROWS + random(`s${i}`) * 0.25
    })
    const sweep = [...ids].sort((a, b) => key[a] - key[b])
    const rank = new Int32Array(EDGES)
    sweep.forEach((id, n) => (rank[id] = n))
    const weak = ids.map((i) => {
      const c = i % COLS
      const r = Math.floor(i / COLS)
      return Math.hypot(c / COLS - 0.92, r / ROWS - 0.9) + random(`w${i}`) * 0.35
    })
    const hollowSet = new Uint8Array(EDGES)
    ;[...ids].sort((a, b) => weak[a] - weak[b]).slice(0, UNVERIFIED).forEach((id) => (hollowSet[id] = 1))
    const hollowRank = new Float32Array(EDGES)
    ids.forEach((i) => (hollowRank[i] = random(`h${i}`)))
    return { rank, hollowSet, hollowRank }
  }, [])

  const ref = useCanvas((g) => {
    const n = placed(f, fill)
    const hv = clamp01((f - hollow[0]) / hollow[1])
    const rad = STEP * 0.34
    for (let i = 0; i < EDGES; i++) {
      if (order.rank[i] >= n) continue
      const x = GRID.x + ((i % COLS) + 0.5) * STEP
      const y = GRID.y + (Math.floor(i / COLS) + 0.5) * STEP
      const isHollow = order.hollowSet[i] && order.hollowRank[i] < hv * 1.0001
      if (isHollow) {
        g.strokeStyle = dim
        g.lineWidth = 1.1
        g.beginPath()
        g.arc(x, y, rad * 0.85, 0, Math.PI * 2)
        g.stroke()
      } else {
        g.fillStyle = ink
        g.beginPath()
        g.arc(x, y, rad, 0, Math.PI * 2)
        g.fill()
      }
    }
    // The false positive: a pink dot with rings.
    const m = clamp01((f - mark) / 14)
    if (m > 0) {
      g.fillStyle = accent
      g.beginPath()
      g.arc(FP_POINT.x, FP_POINT.y, rad + 7 * easeOut(m), 0, Math.PI * 2)
      g.fill()
      for (let k = 0; k < 2; k++) {
        const u = ((f - mark + k * 22) % 44) / 44
        g.strokeStyle = accent
        g.globalAlpha = 1 - u
        g.lineWidth = 3
        g.beginPath()
        g.arc(FP_POINT.x, FP_POINT.y, 12 + u * 60, 0, Math.PI * 2)
        g.stroke()
        g.globalAlpha = 1
      }
    }
  })
  return <canvas ref={ref} width={1920} height={1080} style={{ position: 'absolute', inset: 0 }} />
}
