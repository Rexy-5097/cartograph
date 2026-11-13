// The commercial's modern half: the problem, what Cartograph does, how, what
// it holds itself to, and the vision. Quiet, letterboxed, one accent colour.
//
// Every product statement is taken from the README or the site, and nothing
// is claimed beyond them: no figure appears that the benchmarks don't hold,
// and the principles are the project's own.

import React from 'react'
import { useCurrentFrame } from 'remotion'
import { Sfx } from '../audio'
import { clamp01, easeInOut, easeOut } from '../broll/canvas'
import { Globe } from '../broll/Globe'
import { Graph, GraphPalette } from '../broll/Graph'
import { Mark } from '../parts'
import { C, FONT, MONO } from '../theme'

type P = { total: number }
export const INK = '#0b0b0b'
const TEXT = '#ede8da'
const MUTED = '#8f8b80'
const LINE = '#2a2a2a'

const abs = (x: number, y: number, extra?: React.CSSProperties): React.CSSProperties => ({ position: 'absolute', left: x, top: y, ...extra })

/** In over `dur`, out over 14 frames before `to`. */
const life = (f: number, from: number, to: number, dur = 24) => easeOut((f - from) / dur) * (1 - easeInOut((f - (to - 14)) / 14))

/** A small spaced uppercase label. */
const Kicker: React.FC<{ at: number; children: React.ReactNode; x?: number; y?: number }> = ({ at, children, x = 200, y = 200 }) => {
  const f = useCurrentFrame()
  const t = easeOut((f - at) / 30)
  return (
    <div style={abs(x, y, { fontFamily: MONO, fontSize: 20, letterSpacing: '0.32em', textTransform: 'uppercase', color: MUTED, opacity: t, display: 'flex', alignItems: 'center', gap: 20 })}>
      <span style={{ width: 40 * t, height: 1, background: MUTED }} />
      {children}
    </div>
  )
}

/** A line that arrives word by word out of a soft blur — slowly. */
const Line: React.FC<{ text: string; at: number; size?: number; weight?: number; color?: string; accent?: string[]; style?: React.CSSProperties }> = ({
  text,
  at,
  size = 64,
  weight = 300,
  color = TEXT,
  accent = [],
  style,
}) => {
  const f = useCurrentFrame()
  const words = text.split(' ')
  return (
    <div style={{ fontFamily: FONT, fontSize: size, fontWeight: weight, letterSpacing: '-0.02em', lineHeight: 1.18, color, ...style }}>
      {words.map((w, i) => {
        const t = easeOut((f - at - i * 4) / 30)
        const hot = accent.includes(w.replace(/[.,]/g, ''))
        return (
          <React.Fragment key={i}>
            <span style={{ display: 'inline-block', opacity: t, transform: `translateY(${(1 - t) * 18}px)`, filter: t < 1 ? `blur(${(1 - t) * 8}px)` : undefined, color: hot ? C.pink : undefined }}>{w}</span>
            {i < words.length - 1 ? ' ' : ''}
          </React.Fragment>
        )
      })}
    </div>
  )
}

const QUIET_GRAPH: GraphPalette = { node: '#3d3d3a', edge: '#262624', cross: C.pinkDeep, accent: C.pink, text: '#55534d', panel: INK }

// ─── The problem ───────────────────────────────────────────────────────────
const CHAIN = ['TypeScript', 'HTTP', 'Python', 'SQL']

export const Problem: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  // The picture opens from the dot the old set left behind: a line, then the frame.
  const lineW = easeInOut(f / 14)
  const open = easeInOut((f - 12) / 24)
  const a = life(f, 30, 168)
  const b = life(f, 172, total + 30)
  return (
    <div style={{ position: 'absolute', inset: 0, background: '#000' }}>
      <div style={{ position: 'absolute', inset: 0, background: INK, clipPath: `inset(${(1 - open) * 50}% 0 ${(1 - open) * 50}% 0)` }}>
        <Graph parse={clamp01(f / 120)} links={clamp01((f - 30) / 140)} resolve={clamp01((f - 90) / 160)} prove={0} palette={QUIET_GRAPH} camera={{ scale: 1.15, x: -40 + f * 0.15, y: 10 }} opacity={0.55} />
        <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(90deg, rgba(11,11,11,0.96) 0%, rgba(11,11,11,0.75) 45%, rgba(11,11,11,0.2) 100%)' }} />
        <Kicker at={20}>Today</Kicker>
        <div style={abs(200, 430, { width: 1300, opacity: a })}>
          <Line text="One request now crosses three languages." at={34} />
          <div style={{ display: 'flex', gap: 28, alignItems: 'center', marginTop: 46, fontFamily: MONO, fontSize: 26, color: MUTED }}>
            {CHAIN.map((c, i) => {
              const t = easeOut((f - 80 - i * 12) / 18)
              return (
                <React.Fragment key={c}>
                  <span style={{ opacity: t, color: t > 0.9 ? TEXT : MUTED }}>{c}</span>
                  {i < CHAIN.length - 1 && <span style={{ width: 70 * easeOut((f - 86 - i * 12) / 14), height: 1, background: C.pink, display: 'inline-block' }} />}
                </React.Fragment>
              )
            })}
          </div>
        </div>
        <div style={abs(200, 420, { width: 1400, opacity: b })}>
          <Line text="An import graph tells you which files import which." at={176} size={58} color={MUTED} />
          <div style={{ height: 22 }} />
          <Line text="It doesn't tell you what a request reaches." at={236} size={58} accent={['reaches']} />
        </div>
      </div>
      {/* The line the picture opens from. */}
      {open < 1 && <div style={abs(960 - 960 * lineW, 539, { width: 1920 * lineW, height: 2, background: '#fff', opacity: 1 - open, boxShadow: '0 0 20px rgba(255,255,255,0.8)' })} />}

      <Sfx at={4} name="whoosh" volume={0.35} />
      {CHAIN.map((c, i) => <Sfx key={c} at={80 + i * 12} name="pop" volume={0.18} />)}
    </div>
  )
}

// ─── The solution ──────────────────────────────────────────────────────────
export const Solution: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const sub = (from: number, to: number) => life(f, from, to, 26)
  return (
    <div style={{ position: 'absolute', inset: 0, background: INK }}>
      <Globe cx={960} cy={430} r={190} count={680} assemble={[0, 70]} rgb="183, 179, 156" light="237, 232, 218" spin={0.006} labelBg={INK} />
      <div style={{ position: 'absolute', left: 0, right: 0, top: 700, textAlign: 'center' }}>
        <Line text="Cartograph computes the architecture from source." at={60} size={54} />
      </div>
      <div style={{ position: 'absolute', left: 0, right: 0, top: 790, textAlign: 'center', fontFamily: FONT, fontSize: 34, fontWeight: 300, color: MUTED }}>
        <div style={{ position: 'absolute', left: 0, right: 0, opacity: sub(130, 205) }}>Across languages. Down to the table.</div>
        <div style={{ position: 'absolute', left: 0, right: 0, opacity: sub(205, total + 30) }}>
          With the <span style={{ color: TEXT }}>evidence</span> behind every edge.
        </div>
      </div>
      <Sfx at={0} name="shimmer" volume={0.25} />
    </div>
  )
}

// ─── How it works ──────────────────────────────────────────────────────────
const CARDS = [
  { lang: 'TypeScript', name: 'enqueueConnectionTest', kind: 'function' },
  { lang: 'Python', name: 'enqueue_connection_test', kind: 'handler' },
  { lang: 'SQLAlchemy', name: 'ConnectionTestRequest', kind: 'model' },
  { lang: 'Database', name: 'connection_test_request', kind: 'table' },
]
const EDGES = ['HTTP-CALL', 'ORM-ACCESS', 'QUERIES']
const CW = 330
const GAP = 110
const X0 = (1920 - (4 * CW + 3 * GAP)) / 2
const CARD_Y = 380
const STEPS = [
  { n: '01', title: 'Parse', body: 'It reads the source — TypeScript, TSX and Python. Nothing leaves the machine.', from: 16, to: 160 },
  { n: '02', title: 'Resolve', body: 'It joins a call to the handler that serves it, and a model to its table.', from: 160, to: 310 },
  { n: '03', title: 'Prove', body: 'Every edge carries its evidence. Where the evidence is thin, it draws nothing.', from: 310, to: 999 },
]
const CARD_AT = (i: number) => 34 + i * 22
const EDGE_AT = (i: number) => 176 + i * 40
const PROOF_AT = (i: number) => 326 + i * 26

export const How: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  return (
    <div style={{ position: 'absolute', inset: 0, background: INK }}>
      <Kicker at={6} x={X0}>How it works</Kicker>
      <div style={abs(1920 - X0 - 800, 202, { width: 800, textAlign: 'right', fontFamily: MONO, fontSize: 17, letterSpacing: '0.12em', color: '#5e5b54', opacity: easeOut((f - 40) / 30) })}>REAL OUTPUT · APACHE/AIRFLOW · 9B43D6A</div>

      {/* The chain. */}
      {EDGES.map((e, i) => {
        const t = easeOut((f - EDGE_AT(i)) / 28)
        const x = X0 + (i + 1) * CW + i * GAP
        const u = ((f - EDGE_AT(i) - 28) % 46) / 46
        const proof = easeOut((f - PROOF_AT(i)) / 20)
        return (
          <div key={e} style={abs(x, CARD_Y + 95, { width: GAP })}>
            <div style={{ height: 2, width: GAP * t, background: C.pink }} />
            {t >= 1 && u >= 0 && <div style={abs(GAP * u - 4, -3, { width: 8, height: 8, borderRadius: 8, background: '#fff' })} />}
            <div style={abs(-60, -46, { width: GAP + 120, textAlign: 'center', fontFamily: MONO, fontSize: 14, letterSpacing: '0.08em', color: C.pink, opacity: t })}>{e}</div>
            <div style={abs(GAP / 2 - 15, 24, { width: 30, height: 30, borderRadius: 30, border: `1.5px solid ${C.pink}`, display: 'flex', alignItems: 'center', justifyContent: 'center', opacity: proof, transform: `scale(${0.6 + 0.4 * proof})` })}>
              <svg width={14} height={14} viewBox="-7 -7 14 14">
                <path d="M-4.5 0 L-1.2 3.3 L4.5 -3" fill="none" stroke={C.pink} strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </div>
          </div>
        )
      })}
      {CARDS.map((c, i) => {
        const t = easeOut((f - CARD_AT(i)) / 26)
        const last = i === CARDS.length - 1
        return (
          <div
            key={c.name}
            style={abs(X0 + i * (CW + GAP), CARD_Y, {
              width: CW,
              height: 190,
              boxSizing: 'border-box',
              padding: '26px 24px',
              border: `1px solid ${last && f > EDGE_AT(2) + 30 ? C.pinkDeep : LINE}`,
              borderRadius: 14,
              background: '#101010',
              opacity: t,
              transform: `translateY(${(1 - t) * 20}px)`,
            })}
          >
            <div style={{ fontFamily: MONO, fontSize: 15, letterSpacing: '0.2em', textTransform: 'uppercase', color: MUTED }}>{c.lang}</div>
            <div style={{ fontFamily: MONO, fontSize: 20, color: TEXT, marginTop: 36, whiteSpace: 'nowrap' }}>{c.name}</div>
            <div style={{ fontFamily: MONO, fontSize: 16, color: '#5e5b54', marginTop: 10 }}>{c.kind}</div>
          </div>
        )
      })}

      {/* The three steps, one at a time. */}
      {STEPS.map((s) => {
        const on = life(f, s.from, s.to, 26)
        if (on <= 0) return null
        return (
          <div key={s.n} style={abs(X0, 690, { width: 1400, opacity: on, transform: `translateY(${(1 - on) * 14}px)`, display: 'flex', gap: 44, alignItems: 'baseline' })}>
            <span style={{ fontFamily: MONO, fontSize: 22, color: C.pink, letterSpacing: '0.1em' }}>{s.n}</span>
            <div>
              <div style={{ fontFamily: FONT, fontSize: 54, fontWeight: 400, letterSpacing: '-0.02em', color: TEXT }}>{s.title}</div>
              <div style={{ fontFamily: FONT, fontSize: 30, fontWeight: 300, color: MUTED, marginTop: 10 }}>{s.body}</div>
            </div>
          </div>
        )
      })}

      {CARDS.map((c, i) => <Sfx key={c.name} at={CARD_AT(i)} name="pop" volume={0.22} />)}
      {EDGES.map((e, i) => <Sfx key={e} at={EDGE_AT(i)} name="connect" volume={0.38} />)}
      {EDGES.map((e, i) => <Sfx key={`d${e}`} at={EDGE_AT(i) + 30} name="data" volume={0.22} />)}
      {EDGES.map((e, i) => <Sfx key={`p${e}`} at={PROOF_AT(i)} name="chime" volume={0.16} />)}
    </div>
  )
}

// ─── What it holds itself to ───────────────────────────────────────────────
const PROS = [
  { k: 'Local-first', v: 'Your source never leaves the machine. No API key or network required.' },
  { k: 'No model in the graph', v: 'Deterministic analysis draws every edge. A language model never does.' },
  { k: 'Refuses to guess', v: 'An ambiguous match produces no edge at all.' },
  { k: 'Measured in the open', v: 'Benchmarked on seven open-source repositories, failures published.' },
  { k: 'One core', v: 'Command line, desktop map, MCP server, pull-request review.' },
]

export const Pros: React.FC<P> = () => {
  const f = useCurrentFrame()
  return (
    <div style={{ position: 'absolute', inset: 0, background: INK }}>
      <Kicker at={6}>What it holds to</Kicker>
      <div style={abs(200, 270, { width: 1520 })}>
        {PROS.map((p, i) => {
          const at = 24 + i * 34
          const rule = easeInOut((f - at) / 30)
          const t = easeOut((f - at - 6) / 28)
          return (
            <div key={p.k} style={{ position: 'relative', height: 118 }}>
              <div style={{ position: 'absolute', top: 0, left: 0, height: 1, width: `${rule * 100}%`, background: LINE }} />
              <div style={{ position: 'absolute', top: 34, left: 0, width: 520, fontFamily: FONT, fontSize: 38, fontWeight: 400, letterSpacing: '-0.015em', color: TEXT, opacity: t, transform: `translateX(${(1 - t) * -16}px)` }}>{p.k}</div>
              <div style={{ position: 'absolute', top: 42, left: 560, width: 960, fontFamily: FONT, fontSize: 28, fontWeight: 300, color: MUTED, opacity: easeOut((f - at - 14) / 28) }}>{p.v}</div>
            </div>
          )
        })}
      </div>
      {PROS.map((p, i) => <Sfx key={p.k} at={24 + i * 34} name="pop" volume={0.12} />)}
    </div>
  )
}

// ─── The vision, then the card ─────────────────────────────────────────────
export const CARD_AT_FRAME = 200

export const Vision: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const vision = 1 - easeInOut((f - (CARD_AT_FRAME - 26)) / 22)
  const card = CARD_AT_FRAME
  const name = easeOut((f - card - 40) / 30)
  const fadeOut = 1 - easeInOut((f - (total - 34)) / 30)
  return (
    <div style={{ position: 'absolute', inset: 0, background: INK }}>
      {vision > 0 && (
        <div style={{ position: 'absolute', inset: 0, opacity: vision }}>
          <Kicker at={8}>The vision</Kicker>
          <div style={abs(200, 400, { width: 1500 })}>
            <Line text="Architecture that is computed," at={24} size={78} />
            <Line text="not remembered." at={52} size={78} color={MUTED} />
            <div style={{ height: 30 }} />
            <Line text="Proven, not asserted." at={104} size={78} accent={['Proven']} />
          </div>
        </div>
      )}
      {f >= card && (
        <div style={{ position: 'absolute', inset: 0, opacity: fadeOut, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 34, marginTop: -40 }}>
            <Mark at={card + 4} sendAt={[card + 70, card + 170]} size={96} color={TEXT} />
            <div style={{ overflow: 'hidden', width: 520 * name }}>
              <div style={{ fontFamily: FONT, fontSize: 96, fontWeight: 500, letterSpacing: '-0.045em', color: TEXT, whiteSpace: 'nowrap', transform: `translateY(${(1 - name) * 30}px)` }}>Cartograph</div>
            </div>
          </div>
          <div style={{ height: 46 }} />
          <Line text="Compute the architecture. Prove the relationships." at={card + 86} size={38} weight={300} />
          <div style={{ height: 70 }} />
          <div style={{ fontFamily: MONO, fontSize: 18, letterSpacing: '0.3em', textTransform: 'uppercase', color: MUTED, opacity: easeOut((f - card - 130) / 30) }}>Open source · cartograph.co.in</div>
        </div>
      )}
      <Sfx at={card + 20} name="pop" volume={0.25} />
      <Sfx at={card + 26} name="pop" volume={0.25} />
      <Sfx at={card + 32} name="connect" volume={0.35} />
      <Sfx at={card + 86} name="chime" volume={0.3} />
    </div>
  )
}
