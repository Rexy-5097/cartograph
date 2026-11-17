// An illustrative architecture graph: TypeScript files, Python modules and
// database tables, built in Cartograph's three stages. It is a picture of
// the idea, not output — it carries no figures, and its only labels are the
// kinds of things Cartograph reads.

import React, { useMemo } from 'react'
import { random, useCurrentFrame } from 'remotion'
import { clamp01, easeOut } from './canvas'

type Lang = 'ts' | 'py' | 'sql'
type Node = { x: number; y: number; lang: Lang; delay: number }
type Edge = { a: number; b: number; delay: number; cross: boolean }

export type GraphPalette = { node: string; edge: string; cross: string; accent: string; text: string; panel: string }

const CLUSTERS: Array<{ lang: Lang; cx: number; cols: number; rows: number; w: number; h: number }> = [
  { lang: 'ts', cx: 430, cols: 3, rows: 5, w: 340, h: 620 },
  { lang: 'py', cx: 960, cols: 3, rows: 5, w: 340, h: 620 },
  { lang: 'sql', cx: 1490, cols: 2, rows: 3, w: 220, h: 440 },
]
const TITLES: Record<Lang, string> = { ts: 'TypeScript · TSX', py: 'Python', sql: 'Tables' }

function build() {
  const nodes: Node[] = []
  const start: Record<Lang, number> = { ts: 0, py: 0, sql: 0 }
  CLUSTERS.forEach((c) => {
    start[c.lang] = nodes.length
    for (let row = 0; row < c.rows; row++) {
      for (let col = 0; col < c.cols; col++) {
        const i = nodes.length
        const gx = c.cols === 1 ? 0 : col / (c.cols - 1) - 0.5
        const gy = row / (c.rows - 1) - 0.5
        nodes.push({
          x: c.cx + gx * c.w + (random(`nx${i}`) - 0.5) * 70,
          y: 560 + gy * c.h + (random(`ny${i}`) - 0.5) * 50,
          lang: c.lang,
          delay: random(`nd${i}`),
        })
      }
    }
  })
  const of = (lang: Lang) => nodes.map((n, i) => ({ n, i })).filter((o) => o.n.lang === lang).map((o) => o.i)
  const ts = of('ts')
  const py = of('py')
  const sql = of('sql')
  const edges: Edge[] = []
  // Imports and calls within a language: each node to one or two near ones.
  ;[ts, py].forEach((group) => {
    group.forEach((i, k) => {
      const near = group
        .filter((j) => j !== i)
        .sort((p, q) => Math.hypot(nodes[p].x - nodes[i].x, nodes[p].y - nodes[i].y) - Math.hypot(nodes[q].x - nodes[i].x, nodes[q].y - nodes[i].y))
      const n = 1 + Math.floor(random(`ne${i}`) * 2)
      for (let m = 0; m < n; m++) {
        const j = near[m + Math.floor(random(`nm${i}-${m}`) * 2)]
        if (j !== undefined && !edges.some((e) => (e.a === i && e.b === j) || (e.a === j && e.b === i))) {
          edges.push({ a: i, b: j, delay: random(`ed${i}-${m}`), cross: false })
        }
      }
      void k
    })
  })
  // Across languages: client calls to the handlers that serve them, and models
  // to their tables.
  const pairs: Array<[number, number]> = [
    [ts[1], py[0]],
    [ts[4], py[4]],
    [ts[6], py[7]],
    [ts[8], py[6]],
    [ts[10], py[12]],
    [ts[13], py[10]],
    [py[2], sql[0]],
    [py[5], sql[1]],
    [py[8], sql[3]],
    [py[11], sql[2]],
    [py[14], sql[5]],
    [py[9], sql[4]],
  ]
  pairs.forEach(([a, b], n) => edges.push({ a, b, delay: n / pairs.length, cross: true }))
  // One client call with two equally good candidates: no edge is drawn.
  const ambiguous = { from: ts[12], to: [py[9], py[13]] as [number, number] }
  return { nodes, edges, ambiguous, proven: [6, 0, 8] }
}

type Props = {
  parse: number
  links: number
  resolve: number
  prove: number
  /** 0 hidden · 0–1 the two dashed candidates appear · 1–2 they are refused. */
  ambiguous?: number
  palette: GraphPalette
  camera?: { scale: number; x: number; y: number }
  titles?: number
  opacity?: number
}

const curve = (ax: number, ay: number, bx: number, by: number, bend = 0.12) => {
  const mx = (ax + bx) / 2 - (by - ay) * bend
  const my = (ay + by) / 2 + (bx - ax) * bend
  return { d: `M${ax} ${ay} Q${mx} ${my} ${bx} ${by}`, mx, my }
}

export const Graph: React.FC<Props> = ({ parse, links, resolve, prove, ambiguous = 0, palette, camera = { scale: 1, x: 0, y: 0 }, titles = 0, opacity = 1 }) => {
  const f = useCurrentFrame()
  const { nodes, edges, ambiguous: amb, proven } = useMemo(build, [])
  const shape = (n: Node, i: number) => {
    const t = easeOut((parse - n.delay * 0.6) / 0.4)
    if (t <= 0) return null
    const s = t
    const common = { fill: palette.node, transform: `translate(${n.x} ${n.y}) scale(${s})` }
    if (n.lang === 'ts') return <rect key={i} x={-10} y={-10} width={20} height={20} {...common} />
    if (n.lang === 'py') return <circle key={i} r={11} {...common} />
    return <rect key={i} x={-22} y={-14} width={44} height={28} rx={6} {...common} fill="none" stroke={palette.node} strokeWidth={3.5} />
  }
  const crossEdges = edges.filter((e) => e.cross)
  return (
    <svg width={1920} height={1080} style={{ position: 'absolute', inset: 0, opacity }}>
      <g transform={`translate(960 540) scale(${camera.scale}) translate(${-960 + camera.x} ${-540 + camera.y})`}>
        {CLUSTERS.map((c) => (
          <text key={c.lang} x={c.cx} y={560 - c.h / 2 - 70} textAnchor="middle" fill={palette.text} opacity={clamp01(titles)} style={{ font: '500 24px "Geist Mono"', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
            {TITLES[c.lang]}
          </text>
        ))}
        {edges
          .filter((e) => !e.cross)
          .map((e, k) => {
            const t = clamp01((links - e.delay * 0.6) / 0.4)
            if (t <= 0) return null
            const a = nodes[e.a]
            const b = nodes[e.b]
            return <path key={`i${k}`} d={curve(a.x, a.y, b.x, b.y, 0.08).d} fill="none" stroke={palette.edge} strokeWidth={2} pathLength={1} strokeDasharray={1} strokeDashoffset={1 - t} />
          })}
        {ambiguous > 0 &&
          amb.to.map((to, k) => {
            const a = nodes[amb.from]
            const b = nodes[to]
            const shown = clamp01(ambiguous)
            const gone = clamp01(ambiguous - 1)
            return (
              <path key={`a${k}`} d={curve(a.x, a.y, b.x, b.y, k ? -0.1 : 0.1).d} fill="none" stroke={palette.accent} strokeWidth={3} strokeDasharray="10 9" opacity={shown * (1 - gone)} />
            )
          })}
        {crossEdges.map((e, k) => {
          const t = easeOut((resolve - e.delay * 0.6) / 0.4)
          if (t <= 0) return null
          const a = nodes[e.a]
          const b = nodes[e.b]
          const c = curve(a.x, a.y, b.x, b.y, 0.14)
          // A packet runs along each resolved edge.
          const u = ((f + k * 13) % 50) / 50
          const v = 1 - u
          const px = v * v * a.x + 2 * v * u * c.mx + u * u * b.x
          const py = v * v * a.y + 2 * v * u * c.my + u * u * b.y
          const proof = easeOut((prove - e.delay * 0.5) / 0.5)
          const midx = 0.25 * a.x + 0.5 * c.mx + 0.25 * b.x
          const midy = 0.25 * a.y + 0.5 * c.my + 0.25 * b.y
          return (
            <g key={`c${k}`}>
              <path d={c.d} fill="none" stroke={palette.cross} strokeWidth={3.5} pathLength={1} strokeDasharray={1} strokeDashoffset={1 - t} />
              {t >= 1 && <circle cx={px} cy={py} r={6} fill={palette.accent} />}
              {proof > 0 && (
                <g transform={`translate(${midx} ${midy}) scale(${proof})`}>
                  <circle r={15} fill={palette.panel} stroke={palette.cross} strokeWidth={2.5} />
                  <path d="M-6 0 L-1.5 4.5 L6.5 -4.5" fill="none" stroke={palette.cross} strokeWidth={3} strokeLinecap="round" strokeLinejoin="round" />
                </g>
              )}
            </g>
          )
        })}
        {nodes.map(shape)}
        {/* The evidence each proven edge carries, called out on a few. */}
        {proven.map((ci, k) => {
          const e = crossEdges[ci]
          const proof = easeOut((prove - 0.3 - k * 0.15) / 0.3)
          if (!e || proof <= 0) return null
          const a = nodes[e.a]
          const b = nodes[e.b]
          const c = curve(a.x, a.y, b.x, b.y, 0.14)
          const midx = 0.25 * a.x + 0.5 * c.mx + 0.25 * b.x
          const midy = 0.25 * a.y + 0.5 * c.my + 0.25 * b.y
          return (
            <g key={`p${k}`} opacity={proof} transform={`translate(${midx + 24} ${midy - 70 + (1 - proof) * 16})`}>
              <rect x={0} y={0} width={430} height={44} rx={22} fill={palette.panel} stroke={palette.cross} strokeWidth={2} />
              <text x={215} y={29} textAnchor="middle" fill={palette.text} style={{ font: '500 19px "Geist Mono"' }}>
                provenance · confidence · evidence
              </text>
            </g>
          )
        })}
      </g>
    </svg>
  )
}

