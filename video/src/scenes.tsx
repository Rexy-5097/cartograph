// The walkthrough's scenes. Every sentence and figure here is taken from the
// landing page (docs/.vitepress/theme/Landing.vue), which traces each figure
// to a committed benchmark report. Add no number that is not there. The
// B-roll — the globe, the graph, the code panes — is illustration; the trace
// and the dot field are the real results, drawn.

import React from 'react'
import { interpolate, useCurrentFrame } from 'remotion'
import { CodePanes, markBox, PANES } from './broll/Code'
import { DotGrid, EDGES, FP_POINT, placed } from './broll/DotGrid'
import { Globe } from './broll/Globe'
import { Graph, GraphPalette } from './broll/Graph'
import { easeInOut, easeOut } from './broll/canvas'
import { Appear, CountUp, DotField, Highlight, Label, Mark, Rise, Scene, TONES, Words } from './parts'
import { C, FONT, GLIDE, MONO, prog } from './theme'
import { Sfx, Typing } from './audio'

const H1: React.CSSProperties = { fontSize: 132, fontWeight: 600, lineHeight: 1.0, letterSpacing: '-0.035em' }
const H2: React.CSSProperties = { fontSize: 92, fontWeight: 600, lineHeight: 1.04, letterSpacing: '-0.03em' }
const BODY: React.CSSProperties = { fontSize: 36, lineHeight: 1.4, fontWeight: 400 }
const abs = (x: number, y: number, extra?: React.CSSProperties): React.CSSProperties => ({ position: 'absolute', left: x, top: y, ...extra })

type P = { total: number }

// ─── 1. Cold open: the globe assembles ─────────────────────────────────────
export const COLD_GLOBE = { x: 1310, y: 540 }
const COLD_LINES = [
  { text: 'One TypeScript call.', at: 55, y: 300 },
  { text: 'One Python handler.', at: 82, y: 400 },
  { text: 'One database table.', at: 109, y: 500 },
]

export const ColdOpen: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  return (
    <Scene
      total={total}
      bg={C.ink}
      color={C.manilla}
      exit={false}
      push={0}
      vignette
      broll={
        <Globe
          cx={COLD_GLOBE.x}
          cy={COLD_GLOBE.y}
          r={300}
          count={820}
          assemble={[0, 80]}
          edges={COLD_LINES.map((l) => ({ x: 900, y: l.y + 44, at: l.at + 12 }))}
        />
      }
    >
      {COLD_LINES.map((l) => (
        <Words key={l.text} text={l.text} at={l.at} stagger={4} style={abs(140, l.y, { fontSize: 74, fontWeight: 600, letterSpacing: '-0.03em', whiteSpace: 'nowrap' })} />
      ))}
      <Words
        text="Three languages. No shared symbol table."
        at={140}
        stagger={3}
        style={abs(140, 650, { fontSize: 40, color: C.pink, fontWeight: 500 })}
      />
      <div style={abs(140, 160, { opacity: prog(f, 20, 20) })}>
        <Label at={20} color={C.khaki}>Apache Airflow · 9b43d6a</Label>
      </div>
      <Sfx at={0} name="shimmer" volume={0.5} />
      {COLD_LINES.map((l) => <Sfx key={l.text} at={l.at} name="thump" volume={0.35} />)}
      {COLD_LINES.map((l) => <Sfx key={`c${l.text}`} at={l.at + 12} name="connect" volume={0.45} />)}
      {COLD_LINES.map((l) => <Sfx key={`d${l.text}`} at={l.at + 36} name="data" volume={0.3} />)}
      <Sfx at={140} name="chime" volume={0.4} />
    </Scene>
  )
}

// ─── 2. Title ──────────────────────────────────────────────────────────────
export const Title: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const slide = prog(f, 44, 30)
  return (
    <Scene total={total} bg={C.manilla} color={C.ink} style={{ alignItems: 'center', justifyContent: 'center' }} broll={<DotField color={C.khaki} cx={960} cy={540} opacity={0.5} />}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 44 * slide, marginTop: -80 }}>
        <Mark at={6} sendAt={[60, 140]} size={190} color={C.ink} />
        <div style={{ width: 800 * slide, overflow: 'hidden' }}>
          <Rise at={54}>
            <div style={{ fontSize: 150, fontWeight: 600, letterSpacing: '-0.045em', whiteSpace: 'nowrap' }}>Cartograph</div>
          </Rise>
        </div>
      </div>
      <div style={{ height: 50 }} />
      <Words text="Compute the architecture." at={92} style={{ fontSize: 64, fontWeight: 600, letterSpacing: '-0.025em' }} />
      <Words text="Prove the relationships." at={108} style={{ fontSize: 64, fontWeight: 600, letterSpacing: '-0.025em', color: C.pinkDeep }} />
      <div style={{ height: 40 }} />
      <Appear at={140}>
        <div style={{ fontFamily: MONO, fontSize: 24, letterSpacing: '0.12em', textTransform: 'uppercase', color: C.grey }}>
          Local-first architectural intelligence · TypeScript · TSX · Python
        </div>
      </Appear>
      <Sfx at={22} name="pop" volume={0.5} />
      <Sfx at={28} name="pop" volume={0.5} />
      <Sfx at={34} name="connect" volume={0.5} />
      <Sfx at={54} name="thump" volume={0.7} />
      <Sfx at={60} name="data" volume={0.4} />
      <Sfx at={92} name="whoosh-short" volume={0.25} />
      <Sfx at={140} name="data" volume={0.35} />
    </Scene>
  )
}

// ─── 3. What it is ─────────────────────────────────────────────────────────
const STATEMENT =
  'A local-first engine that derives a cross-language architecture graph from source, and shows the evidence behind every edge it draws.'
const GREEN_GRAPH: GraphPalette = { node: C.khaki, edge: '#56634f', cross: C.pink, accent: C.lightPink, text: C.khaki, panel: C.green }

export const Statement: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const words = STATEMENT.split(' ')
  const from = words.indexOf('evidence')
  const sweep = 14 + words.length * 4.2 + 14
  return (
    <Scene
      total={total}
      bg={C.green}
      color={C.manilla}
      style={{ justifyContent: 'center' }}
      vignette
      broll={
        <Graph
          parse={prog(f, 0, 110)}
          links={prog(f, 40, 110)}
          resolve={prog(f, 100, 120)}
          prove={0}
          palette={GREEN_GRAPH}
          camera={{ scale: 1.25, x: -60 + f * 0.25, y: 0 }}
          opacity={0.3}
        />
      }
    >
      <Label at={0} color={C.khaki}>What it is</Label>
      <div style={{ height: 50 }} />
      <p style={{ ...H2, fontSize: 84, margin: 0, maxWidth: 1600 }}>
        {words.map((w, i) => {
          const t = prog(f, 14 + i * 4.2, 16)
          const last = i === words.length - 1
          return (
            <span key={i} style={{ display: 'inline', opacity: 0.12 + 0.88 * t, filter: t < 1 ? `blur(${(1 - t) * 6}px)` : undefined }}>
              {i >= from ? (
                <Highlight at={sweep + (i - from) * 5} dur={7} color={C.pinkDeep}>
                  {w}
                  {last ? '' : ' '}
                </Highlight>
              ) : (
                <>{w} </>
              )}
            </span>
          )
        })}
      </p>
      <Sfx at={sweep - 2} name="swish" volume={0.3} />
    </Scene>
  )
}

// ─── 4. What it reads: chips fly out of the code ───────────────────────────
const READS = ['symbols', 'imports', 'calls', 'HTTP requests', 'route declarations', 'ORM models']
const chipAt = (i: number) => 64 + i * 20
const chipSlot = (i: number) => ({ x: 140 + (i % 2) * 330, y: 470 + Math.floor(i / 2) * 92 })

export const Reads: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  return (
    <Scene total={total} bg={C.black} color={C.manilla} vignette push={0.03} broll={<CodePanes boxAt={chipAt} />}>
      <div style={abs(140, 120)}>
        <Label at={4} color={C.khaki}>It reads</Label>
      </div>
      <Words text="TypeScript, TSX and Python." at={12} stagger={4} style={abs(140, 200, { ...H2, fontSize: 82, width: 720 })} />
      {READS.map((r, i) => {
        // Each chip leaves the token it was read from and lands in the list.
        const pane = PANES.find((p) => p.marks.some((m) => m[3] === i))!
        const mark = pane.marks.find((m) => m[3] === i)!
        const start = chipAt(i) + 8
        const t = easeInOut((f - start) / 28)
        if (f < start) return null
        const b = markBox(pane, mark, start)
        const from = { x: b.x, y: b.y }
        const to = chipSlot(i)
        const x = from.x + (to.x - from.x) * t
        const y = from.y + (to.y - from.y) * t - Math.sin(Math.PI * t) * 120
        return (
          <div
            key={r}
            style={abs(x, y, {
              fontFamily: MONO,
              fontSize: 30,
              padding: '14px 26px',
              borderRadius: 999,
              border: `2px solid ${t > 0.95 ? C.khaki : C.pink}`,
              background: t > 0.95 ? 'transparent' : 'rgba(205,141,189,0.2)',
              whiteSpace: 'nowrap',
              transform: `scale(${0.8 + 0.2 * t})`,
              transformOrigin: 'left center',
              boxShadow: t < 1 ? `0 0 ${40 * (1 - t)}px rgba(205,141,189,0.6)` : undefined,
            })}
          >
            {r}
          </div>
        )
      })}
      <Appear at={210} style={abs(140, 790, { width: 720 })}>
        <p style={{ ...BODY, fontSize: 38, margin: 0 }}>
          …and joins them across languages that{' '}
          <Highlight at={226} dur={22} color={C.pinkDeep}>
            share no symbol table.
          </Highlight>
        </p>
      </Appear>
      <Appear at={246} style={abs(140, 935, { width: 720 })}>
        <p style={{ ...BODY, fontSize: 24, margin: 0, color: C.khaki }}>Only the source under the path it is given — not Git history, not a database.</p>
      </Appear>
      <Sfx at={0} name="whoosh" volume={0.35} />
      {READS.map((r, i) => (
        <React.Fragment key={`s${r}`}>
          <Sfx at={chipAt(i)} name="pop" volume={0.4} />
          <Sfx at={chipAt(i) + 8} name="whoosh-short" volume={0.28} />
          <Sfx at={chipAt(i) + 34} name="pop" volume={0.3} />
        </React.Fragment>
      ))}
      <Sfx at={226} name="swish" volume={0.3} />
    </Scene>
  )
}

// ─── 5. How it works: the graph builds itself ──────────────────────────────
const INK_GRAPH: GraphPalette = { node: C.ink, edge: '#b9b1a0', cross: C.pinkDeep, accent: C.pink, text: C.grey, panel: C.manilla }
const PHASES = [
  { n: '01', title: 'Parse', body: 'tree-sitter reads symbols, imports, calls, HTTP requests, route declarations, and ORM models with the tables they name.', from: 10, to: 128 },
  { n: '02', title: 'Resolve', body: 'Dynamic URLs are partially evaluated, route dialects normalised, and client calls matched to the handlers that serve them.', from: 128, to: 222 },
  { n: '03', title: 'Prove', body: 'Every edge carries its provenance, a confidence and its evidence. An ambiguous match produces no edge.', from: 222, to: 999 },
]

export const Pipeline: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const amb = interpolate(f, [150, 172, 250, 275], [0, 1, 1, 2], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })
  return (
    <Scene
      total={total}
      bg={C.manilla}
      color={C.ink}
      push={0}
      broll={
        <Graph
          parse={prog(f, 18, 60)}
          titles={prog(f, 30, 20)}
          links={prog(f, 58, 55)}
          resolve={prog(f, 132, 70)}
          prove={prog(f, 226, 80)}
          ambiguous={amb}
          palette={INK_GRAPH}
          camera={{ scale: interpolate(f, [0, total], [0.86, 0.98]), x: 0, y: 40 }}
        />
      }
    >
      {PHASES.map((p) => {
        const on = prog(f, p.from, 18) * (1 - prog(f, p.to - 10, 14))
        if (on <= 0) return null
        return (
          <React.Fragment key={p.n}>
            <div style={abs(140, 60, { opacity: on, transform: `translateY(${(1 - on) * 30}px)`, display: 'flex', alignItems: 'baseline', gap: 26 })}>
              <span style={{ fontFamily: MONO, fontSize: 30, color: C.pinkDeep }}>{p.n}</span>
              <span style={{ fontSize: 86, fontWeight: 600, letterSpacing: '-0.03em' }}>{p.title}</span>
            </div>
            <p style={abs(1040, 74, { width: 740, margin: 0, fontSize: 27, lineHeight: 1.4, color: C.grey, opacity: on, transform: `translateY(${(1 - on) * 20}px)` })}>{p.body}</p>
          </React.Fragment>
        )
      })}
      {/* The ambiguous call: two candidates, so no edge. */}
      <div style={abs(140, 975, { opacity: Math.min(1, amb) * (amb > 1.6 ? 1 : 1), fontFamily: MONO, fontSize: 24, color: C.pinkDeep, display: 'flex', gap: 18, alignItems: 'center' })}>
        <span style={{ width: 40, borderTop: `3px dashed ${C.pinkDeep}` }} />
        {amb < 1.3 ? 'Two candidates for one call…' : '…ambiguous, so no edge. Refuse rather than guess.'}
      </div>
      {[10, 128, 222].map((at) => <Sfx key={`w${at}`} at={at} name="whoosh-short" volume={0.3} />)}
      <Sfx at={18} name="shimmer" volume={0.35} />
      {[60, 76, 92].map((at) => <Sfx key={`l${at}`} at={at} name="connect" volume={0.25} />)}
      {[132, 146, 160, 174].map((at) => <Sfx key={`r${at}`} at={at} name="connect" volume={0.42} />)}
      {[150, 182].map((at) => <Sfx key={`d${at}`} at={at} name="data" volume={0.32} />)}
      <Sfx at={252} name="deny" volume={0.6} />
      {[232, 266, 286].map((at) => <Sfx key={`c${at}`} at={at} name="chime" volume={0.3} />)}
    </Scene>
  )
}

// ─── 6. Real output: the camera follows the trace ──────────────────────────
const CARD_W = 540
const GAP = 300
const STEP = CARD_W + GAP
const CARDS = [
  { lang: 'TypeScript', name: 'enqueueConnectionTest', kind: 'function', path: '…/requests/services.gen.ts:834' },
  { lang: 'Python · handler', name: 'enqueue_connection_test', kind: 'function', path: '…/routes/public/connections.py:383' },
  { lang: 'Python · SQLAlchemy model', name: 'ConnectionTestRequest', kind: 'class', path: '…/airflow/models/connection_test.py:82' },
  { lang: 'Database', name: 'connection_test_request', kind: 'table', path: '' },
]
const HOPS = [
  {
    edge: 'HTTP-CALL',
    meta: 'confidence 0.98 · route-matcher',
    evidence: 'POST /api/v2/connections/enqueue-test matched POST /api/v2/connections/enqueue-test — exact HTTP method; every path segment static and equal',
  },
  { edge: 'ORM-ACCESS', meta: 'confidence 0.80 · orm-resolution', evidence: 'ConnectionTestRequest(...) in enqueue_connection_test at …/connections.py:427' },
  { edge: 'QUERIES', meta: 'confidence 0.80 · orm-resolution', evidence: 'ConnectionTestRequest maps to table "connection_test_request"' },
]
// Per hop: when its edge draws, and the window its evidence shows.
const HOP_AT = [72, 162, 252]

export const Trace: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const centre = (i: number) => i * STEP + CARD_W / 2
  const worldMid = (3 * STEP + CARD_W) / 2
  const fx = interpolate(
    f,
    [0, 72, 118, 162, 208, 252, 298, 318, 356],
    [centre(0), centre(0), centre(1), centre(1), centre(2), centre(2), centre(3), centre(3), worldMid],
    { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: GLIDE },
  )
  const scale = interpolate(f, [0, 30, 318, 356], [1.12, 1, 1, 0.5], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: GLIDE })
  const cmd = '$ cartograph trace enqueueConnectionTest'
  const typed = cmd.slice(0, Math.floor(interpolate(f, [4, 34], [0, cmd.length], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })))
  const hop = HOP_AT.findIndex((at, i) => f >= at + 10 && f < (HOP_AT[i + 1] ?? 318) + 4)
  const final = prog(f, 350, 22)
  return (
    <Scene total={total} bg={C.ink} color={C.manilla} push={0} vignette style={{ padding: 0 }}>
      {/* The world: a grid that slides past, the cards and the edges. */}
      <div
        style={{
          position: 'absolute',
          left: 0,
          top: 0,
          width: 1920,
          height: 1080,
          transform: `translate(${960 - fx * scale}px, ${560}px) scale(${scale})`,
          transformOrigin: '0 0',
        }}
      >
        <div
          style={{
            position: 'absolute',
            left: -2000,
            top: -1500,
            width: 7500,
            height: 3000,
            backgroundImage: 'linear-gradient(rgba(237,232,218,0.05) 2px, transparent 2px), linear-gradient(90deg, rgba(237,232,218,0.05) 2px, transparent 2px)',
            backgroundSize: '120px 120px',
          }}
        />
        {HOPS.map((h, i) => {
          const t = easeOut((f - HOP_AT[i]) / 40)
          const x0 = i * STEP + CARD_W
          const len = GAP
          const pkt = ((f - HOP_AT[i]) % 60) / 40
          return (
            <div key={h.edge} style={{ position: 'absolute', left: x0, top: -2, width: len }}>
              <div style={{ height: 4, width: len * t, background: C.pink, boxShadow: `0 0 18px ${C.pink}` }} />
              {t >= 1 && pkt < 1 && <div style={{ position: 'absolute', left: len * pkt - 9, top: -7, width: 18, height: 18, borderRadius: 99, background: C.lightPink, boxShadow: `0 0 24px ${C.lightPink}` }} />}
              <div style={{ position: 'absolute', left: 0, width: len, top: -72, textAlign: 'center', fontFamily: MONO, fontSize: 26, fontWeight: 600, color: C.pink, opacity: t }}>↓ {h.edge}</div>
              <div style={{ position: 'absolute', left: 0, width: len, top: 22, textAlign: 'center', fontFamily: MONO, fontSize: 19, lineHeight: 1.5, color: C.khaki, opacity: t }}>{h.meta.split(' · ').map((m) => <div key={m}>{m}</div>)}</div>
            </div>
          )
        })}
        {CARDS.map((c, i) => {
          const at = i === 0 ? 22 : HOP_AT[i - 1] + 34
          const t = easeOut((f - at) / 22)
          const lit = i === 3 ? prog(f, at, 30) : 0
          return (
            <div
              key={c.name}
              style={{
                position: 'absolute',
                left: i * STEP,
                top: -125,
                width: CARD_W,
                height: 250,
                boxSizing: 'border-box',
                padding: '30px 34px',
                borderRadius: 22,
                background: i === 3 ? `rgba(205,141,189,${0.12 + 0.1 * lit})` : '#141414',
                border: `2px solid ${i === 3 ? C.pink : '#3a3a3a'}`,
                opacity: t,
                transform: `scale(${0.85 + 0.15 * t})`,
                boxShadow: '0 30px 60px rgba(0,0,0,0.5)',
              }}
            >
              <div style={{ fontFamily: MONO, fontSize: 18, letterSpacing: '0.1em', textTransform: 'uppercase', color: C.pink }}>{c.lang}</div>
              <div style={{ fontSize: 30, fontWeight: 600, letterSpacing: '-0.01em', marginTop: 22, fontFamily: MONO, whiteSpace: 'nowrap' }}>{c.name}</div>
              <div style={{ fontFamily: MONO, fontSize: 20, color: C.khaki, marginTop: 10 }}>({c.kind})</div>
              {c.path && <div style={{ fontFamily: MONO, fontSize: 18, color: '#8d8a80', marginTop: 18 }}>{c.path}</div>}
            </div>
          )
        })}
      </div>

      {/* Fixed on screen: the command, the repository, the evidence. */}
      <div style={abs(110, 80, { fontFamily: MONO, fontSize: 30, color: C.khaki })}>
        {typed}
        <span style={{ opacity: Math.floor(f / 14) % 2 ? 0 : 1, color: C.pink }}>▍</span>
      </div>
      <div style={abs(1430, 86, { fontFamily: MONO, fontSize: 22, color: '#8d8a80', opacity: prog(f, 10, 20) })}>apache/airflow · 9b43d6a</div>
      {HOPS.map((h, i) => {
        const on = hop === i ? 1 : 0
        const t = prog(f, HOP_AT[i] + 10, 16) * (1 - prog(f, (HOP_AT[i + 1] ?? 318) - 8, 12))
        if (t <= 0 || !on) return null
        return (
          <div key={h.edge} style={abs(110, 860, { width: 1700, opacity: t, transform: `translateY(${(1 - t) * 20}px)` })}>
            <div style={{ fontFamily: MONO, fontSize: 20, letterSpacing: '0.12em', textTransform: 'uppercase', color: C.pink, marginBottom: 12 }}>evidence</div>
            <div style={{ fontFamily: MONO, fontSize: 26, lineHeight: 1.5, color: C.manilla }}>{h.evidence}</div>
          </div>
        )
      })}
      <div style={abs(110, 850, { width: 1700, opacity: final, transform: `translateY(${(1 - final) * 24}px)` })}>
        <div style={{ fontSize: 52, fontWeight: 600, letterSpacing: '-0.02em', lineHeight: 1.15 }}>
          This path crosses the HTTP boundary and <Highlight at={364} dur={20} color={C.pinkDeep}>reaches a database table.</Highlight>
        </div>
      </div>
      <Typing from={4} to={34} every={2} volume={0.5} />
      <Sfx at={22} name="pop" volume={0.45} />
      {HOP_AT.map((at) => (
        <React.Fragment key={`h${at}`}>
          <Sfx at={at} name="connect" volume={0.5} />
          <Sfx at={at} name="whoosh-short" volume={0.25} />
          <Sfx at={at + 40} name="data" volume={0.4} />
          <Sfx at={at + 34} name="pop" volume={0.45} />
        </React.Fragment>
      ))}
      <Sfx at={318} name="whoosh" volume={0.45} />
      <Sfx at={364} name="chime" volume={0.4} />
    </Scene>
  )
}

// ─── 7. Measured: 14,932 dots ──────────────────────────────────────────────
const FILL: [number, number] = [18, 96]

export const Measured: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const n = placed(f, FILL)
  const fp = prog(f, 140, 18)
  return (
    <Scene
      total={total}
      bg={C.lightPink}
      color={C.ink}
      exit={false}
      push={0}
      broll={<DotGrid fill={FILL} mark={132} hollow={[205, 45]} ink={C.ink} accent={C.pinkDeep} dim="rgba(62,62,62,0.55)" />}
    >
      <div style={abs(140, 150)}>
        <Label at={0} color={C.pinkDeep}>Measured · failures included</Label>
      </div>
      <div style={abs(140, 220, { fontSize: 150, fontWeight: 600, letterSpacing: '-0.05em', lineHeight: 1, fontVariantNumeric: 'tabular-nums', opacity: prog(f, 8, 12) })}>
        {n.toLocaleString('en-US')}
      </div>
      <Words text="edges labelled," at={60} style={abs(140, 390, { fontSize: 60, fontWeight: 600, letterSpacing: '-0.025em' })} />
      <Appear at={136} style={abs(140, 470)}>
        <div style={{ fontSize: 60, fontWeight: 600, letterSpacing: '-0.025em' }}>
          <Highlight at={146} dur={20} color={C.pink}>one false positive.</Highlight>
        </div>
      </Appear>
      <Appear at={150} style={abs(140, 570, { width: 640 })}>
        <p style={{ ...BODY, fontSize: 27, margin: 0, color: C.grey }}>One false positive among 11,221 independently verified edges, across seven pinned repositories.</p>
      </Appear>
      <Appear at={205} style={abs(140, 700, { width: 660 })}>
        <p style={{ ...BODY, fontSize: 30, margin: 0 }}>
          <b>24.9% (3,711)</b> could not be verified from source. They concentrate where evidence is weakest —
        </p>
      </Appear>
      <Appear at={240} style={abs(140, 850, { width: 660 })}>
        <div style={{ fontSize: 38, fontWeight: 600, letterSpacing: '-0.02em', whiteSpace: 'nowrap' }}>
          so every figure is an <Highlight at={252} dur={18} color={C.pink}>upper bound.</Highlight>
        </div>
      </Appear>
      {/* The callout on the one pink dot. */}
      <div style={abs(FP_POINT.x + 30, FP_POINT.y - 92, { opacity: fp, transform: `translateY(${(1 - fp) * 12}px)` })}>
        <div style={{ fontFamily: MONO, fontSize: 22, padding: '10px 20px', borderRadius: 999, background: C.pinkDeep, color: C.white, whiteSpace: 'nowrap' }}>1 false positive</div>
      </div>
      <div style={abs(870, 945, { fontFamily: MONO, fontSize: 20, color: C.grey, opacity: prog(f, 110, 20) })}>
        {EDGES.toLocaleString('en-US')} dots · one per labelled edge · M08, seven pinned repositories
      </div>
      <Sfx at={18} name="shimmer" volume={0.75} />
      <Sfx at={132} name="chime" volume={0.85} />
      <Sfx at={205} name="fall" volume={0.7} />
      <Sfx at={252} name="swish" volume={0.25} />
    </Scene>
  )
}
export const MEASURED_FP = FP_POINT

// ─── 8. Recall in Airflow: gauges ──────────────────────────────────────────
const STATS = [
  { value: 0.907, decimals: 3, label: 'HttpCall recall', note: '117 of 129 operations' },
  { value: 0.744, decimals: 3, label: 'OrmAccess recall', note: '1,401 of 1,883 model uses' },
]

const Ring: React.FC<{ value: number; at: number; size: number }> = ({ value, at, size }) => {
  const f = useCurrentFrame()
  const r = size / 2 - 14
  const len = 2 * Math.PI * r
  const t = prog(f, at, 55)
  return (
    <svg width={size} height={size} style={{ transform: 'rotate(-90deg)' }}>
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="rgba(26,26,26,0.14)" strokeWidth={18} />
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={C.ink} strokeWidth={18} strokeLinecap="round" strokeDasharray={`${len * value * t} ${len}`} />
    </svg>
  )
}

export const Stats: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const SIZE = 330
  return (
    <Scene total={total} bg={C.khaki} color={C.ink} style={{ justifyContent: 'center' }}>
      <Label at={0} color={C.ink}>Measured in Apache Airflow, inside a stated scope</Label>
      <div style={{ height: 60 }} />
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 70 }}>
        {STATS.map((s, i) => {
          const at = 14 + i * 20
          return (
            <Appear key={s.label} at={at} lift={40}>
              <div style={{ position: 'relative', width: SIZE, height: SIZE }}>
                <Ring value={s.value} at={at + 4} size={SIZE} />
                <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 84, fontWeight: 600, letterSpacing: '-0.04em' }}>
                  <CountUp to={s.value} decimals={s.decimals} at={at + 4} dur={55} />
                </div>
              </div>
              <div style={{ fontSize: 38, fontWeight: 600, marginTop: 30 }}>{s.label}</div>
              <div style={{ fontFamily: MONO, fontSize: 23, marginTop: 10, color: C.grey }}>{s.note}</div>
            </Appear>
          )
        })}
        <Appear at={54} lift={40}>
          {/* Five chains; the three whose every hop is independently readable light up. */}
          <div style={{ width: SIZE, height: SIZE, display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 22 }}>
            {[0, 1, 2, 3, 4].map((c) => {
              const lit = c < 3 ? prog(f, 70 + c * 16, 14) : 0
              return (
                <div key={c} style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
                  {[0, 1, 2, 3].map((k) => (
                    <React.Fragment key={k}>
                      <span style={{ width: 26, height: 26, borderRadius: k === 3 ? 6 : 99, border: `3px solid ${C.ink}`, background: lit > 0.5 ? C.ink : 'transparent', boxSizing: 'border-box' }} />
                      {k < 3 && <span style={{ width: 60, height: 3, background: C.ink, opacity: 0.25 + 0.75 * lit }} />}
                    </React.Fragment>
                  ))}
                </div>
              )
            })}
          </div>
          <div style={{ fontSize: 38, fontWeight: 600, marginTop: 30 }}>
            <CountUp to={3} at={70} dur={48} /> of 5 end-to-end chains
          </div>
          <div style={{ fontFamily: MONO, fontSize: 23, marginTop: 10, color: C.grey }}>every hop independently readable</div>
        </Appear>
      </div>
      <div style={{ height: 70 }} />
      <Appear at={130}>
        <p style={{ ...BODY, fontSize: 28, margin: 0, maxWidth: 1500, color: C.grey }}>
          Recall was measured in one repository — not across the corpus. Confidence values are uncalibrated priors, not probabilities.
        </p>
      </Appear>
      <Sfx at={18} name="whoosh-short" volume={0.3} />
      <Sfx at={38} name="whoosh-short" volume={0.3} />
      {[70, 86, 102].map((at) => <Sfx key={at} at={at} name="pop" volume={0.5} />)}
    </Scene>
  )
}

// ─── 9. The governing principle ────────────────────────────────────────────
const CREED = ['Deterministic analysis first.', 'Evidence second.', 'Inference third.', 'The model last, and never in the graph.']
const PRINCIPLES = ['No model in the graph', 'Refuse rather than guess', 'Evidence is first-class', 'No unmeasured number', 'Local-first']
const DIM_GRAPH: GraphPalette = { node: '#4a4a4a', edge: '#333', cross: '#5a5a5a', accent: '#777', text: '#555', panel: C.black }

export const Creed: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  return (
    <Scene
      total={total}
      bg={C.black}
      color={C.manilla}
      style={{ justifyContent: 'center' }}
      vignette
      broll={<Graph parse={1} links={1} resolve={1} prove={0} palette={DIM_GRAPH} camera={{ scale: 1.5, x: 200 - f * 0.6, y: 0 }} opacity={0.45} />}
    >
      <Label at={0} color={C.khaki}>The governing principle</Label>
      <div style={{ height: 46 }} />
      {CREED.map((line, i) => {
        const at = 12 + i * 30
        const t = prog(f, at, 18)
        const last = i === CREED.length - 1
        const dim = last ? 0 : prog(f, at + 30, 26)
        return (
          <div
            key={line}
            style={{
              ...H2,
              fontSize: 86,
              color: last ? C.pink : C.manilla,
              opacity: t * (1 - 0.6 * dim),
              transform: `scale(${1.35 - 0.35 * t})`,
              transformOrigin: 'left center',
              filter: t < 1 ? `blur(${(1 - t) * 18}px)` : undefined,
            }}
          >
            {line}
          </div>
        )
      })}
      <div style={{ height: 70 }} />
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 16 }}>
        {PRINCIPLES.map((p, i) => (
          <Appear key={p} at={150 + i * 6} lift={14}>
            <span style={{ display: 'inline-block', fontFamily: MONO, fontSize: 24, padding: '12px 24px', borderRadius: 999, border: `1.5px solid #555`, color: C.khaki, background: 'rgba(34,34,34,0.8)' }}>{p}</span>
          </Appear>
        ))}
      </div>
      {CREED.map((line, i) => <Sfx key={line} at={12 + i * 30} name="thump" volume={i === CREED.length - 1 ? 0.95 : 0.6} />)}
      {PRINCIPLES.map((p, i) => <Sfx key={p} at={150 + i * 6} name="pop" volume={0.22} />)}
    </Scene>
  )
}

// ─── 10. Surfaces orbit the globe ──────────────────────────────────────────
const SURFACES = [
  { key: 'CLI', title: 'trace · blast · diff', tone: 'black' },
  { key: 'MAP', title: 'Desktop application', tone: 'green' },
  { key: 'MCP', title: 'MCP server', tone: 'pink' },
  { key: 'PR', title: 'Pull-request review', tone: 'light' },
  { key: 'ASK', title: 'Explanation over evidence', tone: 'khaki' },
]
export const SURF_GLOBE = { x: 960, y: 610 }

export const Surfaces: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const O = { rx: 720, ry: 200, tilt: -0.07 }
  const cards = SURFACES.map((s, i) => {
    const th = (i * Math.PI * 2) / SURFACES.length + f * 0.007 + 0.4
    const ex = Math.cos(th) * O.rx
    const ey = Math.sin(th) * O.ry
    const x = SURF_GLOBE.x + ex * Math.cos(O.tilt) - ey * Math.sin(O.tilt)
    const y = SURF_GLOBE.y + ex * Math.sin(O.tilt) + ey * Math.cos(O.tilt)
    const depth = (Math.sin(th) + 1) / 2
    return { s, i, x, y, depth, front: Math.sin(th) > -0.05 }
  })
  const card = (c: (typeof cards)[number]) => {
    const t = easeOut((f - 34 - c.i * 8) / 24)
    const tone = TONES[c.s.tone]
    const sc = (0.62 + 0.42 * c.depth) * t
    return (
      <div
        key={c.s.key}
        style={abs(c.x - 150, c.y - 80, {
          width: 300,
          height: 160,
          boxSizing: 'border-box',
          padding: '22px 26px',
          borderRadius: 22,
          fontFamily: FONT,
          background: tone.bg,
          color: tone.fg,
          border: c.s.tone === 'black' ? '1.5px solid #444' : undefined,
          transform: `scale(${sc})`,
          opacity: (0.45 + 0.55 * c.depth) * t,
          boxShadow: '0 30px 60px rgba(0,0,0,0.45)',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
        })}
      >
        <div style={{ fontSize: 58, fontWeight: 600, letterSpacing: '-0.03em', lineHeight: 1 }}>{c.s.key}</div>
        <div style={{ fontSize: 22, fontWeight: 600 }}>{c.s.title}</div>
      </div>
    )
  }
  const ring = easeOut((f - 20) / 30)
  return (
    <Scene
      total={total}
      bg={C.ink}
      color={C.manilla}
      push={0}
      exit={false}
      vignette
      broll={
        <>
          <svg width={1920} height={1080} style={{ position: 'absolute', inset: 0 }}>
            <ellipse cx={SURF_GLOBE.x} cy={SURF_GLOBE.y} rx={O.rx} ry={O.ry} transform={`rotate(${(O.tilt * 180) / Math.PI} ${SURF_GLOBE.x} ${SURF_GLOBE.y})`} fill="none" stroke={C.pink} strokeOpacity={0.35 * ring} strokeWidth={2} strokeDasharray="4 12" />
          </svg>
          {cards.filter((c) => !c.front).sort((a, b) => a.depth - b.depth).map(card)}
          <Globe cx={SURF_GLOBE.x} cy={SURF_GLOBE.y} r={200} count={620} assemble={[0, 50]} />
          {cards.filter((c) => c.front).sort((a, b) => a.depth - b.depth).map(card)}
        </>
      }
    >
      <div style={abs(140, 100)}>
        <Label at={6} color={C.khaki}>One analysis core</Label>
      </div>
      <Words text="Five ways in." at={14} style={abs(140, 160, { ...H2, fontSize: 84 })} />
      <Sfx at={0} name="shimmer" volume={0.4} />
      <Sfx at={20} name="whoosh" volume={0.3} />
      {SURFACES.map((s, i) => <Sfx key={s.key} at={34 + i * 8} name="pop" volume={0.4} />)}
    </Scene>
  )
}

// ─── 11. Try it ────────────────────────────────────────────────────────────
const INSTALL = 'cargo install --git https://github.com/Rexy-5097/cartograph cartograph-cli'
const RUN = 'cartograph .'

export const Outro: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const typed = (s: string, at: number, cps = 2.6) => s.slice(0, Math.max(0, Math.floor((f - at) * cps)))
  const runAt = 40 + INSTALL.length / 2.6 + 8
  const caret = Math.floor(f / 15) % 2 === 0
  return (
    <Scene
      total={total}
      bg={C.green}
      color={C.manilla}
      style={{ justifyContent: 'center' }}
      vignette
      broll={<Globe cx={1560} cy={560} r={330} count={760} rgb="183, 179, 156" light="237, 232, 218" opacity={0.45} wire={false} />}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 36 }}>
        <Mark at={6} sendAt={[66, 160]} size={110} color={C.manilla} />
        <Rise at={24}>
          <div style={{ fontSize: 96, fontWeight: 600, letterSpacing: '-0.04em' }}>Try it.</div>
        </Rise>
      </div>
      <div style={{ height: 56 }} />
      <Appear at={30}>
        <div style={{ background: 'rgba(16,24,14,0.92)', borderRadius: 16, padding: '30px 36px', fontFamily: MONO, fontSize: 28, lineHeight: 1.8, border: '1px solid #2f3f2c', maxWidth: 1450 }}>
          <div>
            <span style={{ color: C.khaki }}>$ </span>
            {typed(INSTALL, 40)}
          </div>
          <div>
            {f >= runAt && <span style={{ color: C.khaki }}>$ </span>}
            {typed(RUN, runAt, 1.2)}
            {f >= runAt && <span style={{ opacity: caret ? 1 : 0, color: C.pink }}>▍</span>}
          </div>
        </div>
      </Appear>
      <div style={{ height: 56 }} />
      <Words text="No source code leaves the machine." at={runAt + 18} style={{ fontSize: 46, fontWeight: 600, letterSpacing: '-0.02em' }} />
      <Words text="Fully useful with no API key and no network." at={runAt + 32} style={{ fontSize: 34, color: C.khaki, marginTop: 10 }} />
      <div style={{ height: 34 }} />
      <Appear at={runAt + 52}>
        <div style={{ fontFamily: MONO, fontSize: 34, color: C.pink, letterSpacing: '0.02em' }}>cartograph.co.in</div>
      </Appear>
      <Sfx at={22} name="pop" volume={0.45} />
      <Sfx at={28} name="pop" volume={0.45} />
      <Sfx at={34} name="connect" volume={0.45} />
      <Typing from={40} to={40 + INSTALL.length / 2.6} every={2} volume={0.5} />
      <Typing from={runAt} to={runAt + RUN.length / 1.2} every={3} volume={0.5} />
      <Sfx at={runAt + 18} name="thump" volume={0.4} />
      <Sfx at={runAt + 52} name="chime" volume={0.5} />
    </Scene>
  )
}
