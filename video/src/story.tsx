// How Cartograph grew, told from its milestones (agentos/milestones/M00–M17).
// Each chapter states what that milestone's scope delivered, in its terms;
// nothing here is a figure, a date or a motive the record doesn't hold.

import React from 'react'
import { useCurrentFrame } from 'remotion'
import { Sfx, Typing } from './audio'
import { easeInOut, easeOut } from './broll/canvas'
import { Appear, Highlight, Label, Scene } from './parts'
import { C, FONT, GLIDE, MONO, prog } from './theme'

type P = { total: number }
const abs = (x: number, y: number, extra?: React.CSSProperties): React.CSSProperties => ({ position: 'absolute', left: x, top: y, ...extra })

// ─── The milestone rail ────────────────────────────────────────────────────
const RAIL = { x0: 180, x1: 1740, y: 975 }
const tickX = (m: number) => RAIL.x0 + ((RAIL.x1 - RAIL.x0) * m) / 17

const Rail: React.FC<{ pos: number; dark?: boolean }> = ({ pos, dark }) => {
  const f = useCurrentFrame()
  const ink = dark ? C.manilla : C.ink
  const quiet = dark ? '#5c5c5c' : '#bdb5a3'
  const accent = dark ? C.pink : C.pinkDeep
  const t = prog(f, 0, 24)
  return (
    <div style={{ position: 'absolute', inset: 0, opacity: t }}>
      <div style={abs(RAIL.x0, RAIL.y, { width: RAIL.x1 - RAIL.x0, height: 2, background: quiet })} />
      <div style={abs(RAIL.x0, RAIL.y, { width: tickX(pos) - RAIL.x0, height: 2, background: accent })} />
      {Array.from({ length: 18 }, (_, m) => {
        const done = m <= pos + 0.01
        const here = Math.abs(m - pos) < 0.5
        return (
          <div key={m} style={abs(tickX(m) - 30, RAIL.y + 18, { width: 60, textAlign: 'center', fontFamily: MONO, fontSize: here ? 20 : 15, color: here ? accent : done ? ink : quiet, fontWeight: here ? 600 : 400 })}>
            M{String(m).padStart(2, '0')}
          </div>
        )
      })}
      {Array.from({ length: 18 }, (_, m) => (
        <div key={`d${m}`} style={abs(tickX(m) - 4, RAIL.y - 3, { width: 8, height: 8, borderRadius: 9, background: m <= pos + 0.01 ? accent : quiet })} />
      ))}
      <div style={abs(tickX(pos) - 11, RAIL.y - 10, { width: 22, height: 22, borderRadius: 22, background: accent, boxShadow: `0 0 0 6px ${dark ? 'rgba(205,141,189,0.25)' : 'rgba(138,74,121,0.18)'}` })} />
    </div>
  )
}

/** A chapter heading pair that crossfades in and out over [from, to). */
const Chapter: React.FC<{ from: number; to: number; title: string; sub: React.ReactNode; color?: string }> = ({ from, to, title, sub, color }) => {
  const f = useCurrentFrame()
  const on = prog(f, from, 18) * (1 - prog(f, to - 14, 14))
  if (on <= 0) return null
  const words = title.split(' ')
  return (
    <>
      <div style={abs(140, 120, { width: 1640, fontSize: 80, fontWeight: 600, letterSpacing: '-0.03em', lineHeight: 1.05, opacity: Math.min(1, on * 1.4) })}>
        {words.map((w, i) => {
          const t = prog(f, from + i * 3, 18)
          return (
            <React.Fragment key={i}>
              <span style={{ display: 'inline-block', opacity: t, transform: `translateY(${(1 - t) * 30}px)`, filter: t < 1 ? `blur(${(1 - t) * 10}px)` : undefined }}>{w}</span>{' '}
            </React.Fragment>
          )
        })}
      </div>
      <div style={abs(140, 230, { width: 1560, fontSize: 30, lineHeight: 1.4, color, opacity: on * prog(f, from + 12, 18), transform: `translateY(${(1 - on) * 12}px)` })}>{sub}</div>
    </>
  )
}

/** Fades a stage in and out over [from, to). */
const stage = (f: number, from: number, to: number) => prog(f, from, 18) * (1 - prog(f, to - 14, 14))

// ─── Roots: M01 → M08 ──────────────────────────────────────────────────────
const CH = { one: [0, 120], two: [120, 235], three: [235, 375], four: [375, 520], five: [520, 640] } as const

const TS_LINES: Array<Array<[string, string?]>> = [
  [['import', C.pink], [" { http } from './client'"]],
  [],
  [['export function', C.pink], [' '], ['fetchOrder', '#e9c46a'], ['(id: string) {']],
  [['  return '], ['axios.get', '#e9c46a'], ['('], ['`/orders/${id}`', '#b7d3a8'], [')']],
  [['}']],
]
const TS_BOXES = [
  { line: 0, col: 0, len: 6, at: 40, tag: 'import' },
  { line: 2, col: 16, len: 10, at: 54, tag: 'symbol' },
  { line: 3, col: 9, len: 9, at: 68, tag: 'call site' },
  { line: 3, col: 19, len: 15, at: 82, tag: 'URL expression' },
]

const FRAMEWORKS = [
  { name: 'FastAPI', code: '@app.get("/orders/{id}")', at: 150 },
  { name: 'Flask', code: '@app.route("/orders/<int:id>")', at: 166 },
  { name: 'Django', code: 'path("orders/<int:id>/", order)', at: 182 },
]

const DIALECTS = ['/orders/:id', '/orders/{id}', '/orders/<int:id>', '/orders/${id}']

export const Roots: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  // Where the rail's marker sits, chapter by chapter.
  const pos =
    f < CH.two[0] ? 1 : f < CH.three[0] ? 1 + easeInOut((f - CH.two[0]) / 20) : f < CH.four[0] ? 2 + easeInOut((f - CH.three[0]) / 20) : f < CH.five[0] ? 3 + 3 * easeInOut((f - CH.four[0]) / 90) : 6 + 2 * easeInOut((f - CH.five[0]) / 60)
  const CW = 24 * 0.6
  const LH = 24 * 1.7

  // Chapter four and five share the chain.
  const chainOn = stage(f, CH.four[0], CH.five[1] + 20)
  const nodes = [
    { x: 520, label: 'TypeScript call', shape: 'square' },
    { x: 960, label: 'Python handler', shape: 'circle' },
    { x: 1400, label: 'Table', shape: 'table' },
  ]
  const edges = [
    { at: CH.four[0] + 40, label: 'HttpCall' },
    { at: CH.four[0] + 78, label: 'OrmAccess · Queries' },
  ]

  return (
    <Scene total={total} bg={C.manilla} color={C.ink} push={0} style={{ padding: 0 }}>
      <div style={abs(140, 60)}>
        <Label at={0} color={C.pinkDeep}>How it grew</Label>
      </div>

      <Chapter from={CH.one[0] + 6} to={CH.one[1]} title="It started by reading one language." sub="TypeScript and TSX — symbols, imports, call sites, and every fetch or axios call with its URL." color={C.grey} />
      <Chapter from={CH.two[0]} to={CH.two[1]} title="Then a second." sub="Python route declarations: FastAPI decorators, Flask's @app.route, Django's urlpatterns." color={C.grey} />
      <Chapter from={CH.three[0]} to={CH.three[1]} title="Four ways to write one route." sub={<>Normalised to one canonical form. A segment it cannot determine stays <span style={{ fontFamily: MONO }}>{'{unknown}'}</span>.</>} color={C.grey} />
      <Chapter from={CH.four[0]} to={CH.four[1]} title="Then it crossed the boundary." sub="A TypeScript call joined to the Python handler that serves it — then to the model, and the table behind it." color={C.grey} />
      <Chapter
        from={CH.five[0]}
        to={total + 40}
        title="Then it proved it on real code."
        sub={<>A full chain on a repository not written for the test — then measured, <Highlight at={CH.five[0] + 40} dur={18} color={C.pink}>failures published alongside.</Highlight></>}
        color={C.grey}
      />

      {/* One: a TypeScript file, its parts boxed as they are read. */}
      {(() => {
        const on = stage(f, CH.one[0], CH.one[1])
        if (on <= 0) return null
        return (
          <div style={abs(420, 380, { width: 1080, opacity: on, transform: `translateY(${(1 - on) * 30}px)` })}>
            <div style={{ background: C.ink, color: C.manilla, borderRadius: 18, padding: '30px 36px', fontFamily: MONO, fontSize: 24, lineHeight: 1.7, position: 'relative', boxShadow: '0 40px 80px rgba(0,0,0,0.25)' }}>
              {TS_LINES.map((l, i) => (
                <div key={i} style={{ whiteSpace: 'pre', minHeight: '1.7em' }}>
                  {l.map(([text, c], j) => (
                    <span key={j} style={{ color: c }}>{text}</span>
                  ))}
                </div>
              ))}
              {TS_BOXES.map((b) => {
                const t = easeOut((f - b.at) / 10)
                if (t <= 0) return null
                return (
                  <React.Fragment key={b.tag}>
                    <div style={abs(36 + b.col * CW - 6, 30 + b.line * LH - 2, { width: b.len * CW + 12, height: LH + 4, border: `2px solid ${C.pink}`, borderRadius: 8, background: 'rgba(205,141,189,0.14)', opacity: t, transform: `scale(${1.15 - 0.15 * t})` })} />
                    <div style={abs(36 + b.col * CW, b.line === 3 ? 30 + (b.line + 1) * LH + 8 : 30 + b.line * LH - 30, { fontSize: 17, color: C.pink, opacity: t, whiteSpace: 'nowrap' })}>{b.tag}</div>
                  </React.Fragment>
                )
              })}
            </div>
          </div>
        )
      })()}

      {/* Two: three Python frameworks' route declarations. */}
      {(() => {
        const on = stage(f, CH.two[0], CH.two[1])
        if (on <= 0) return null
        return (
          <div style={abs(140, 420, { width: 1640, display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 30, opacity: on })}>
            {FRAMEWORKS.map((fw) => {
              const t = easeOut((f - fw.at) / 20)
              return (
                <div key={fw.name} style={{ background: C.green, color: C.manilla, borderRadius: 20, padding: '30px 30px 36px', opacity: t, transform: `translateY(${(1 - t) * 50}px) rotate(${(1 - t) * -2}deg)` }}>
                  <div style={{ fontSize: 40, fontWeight: 600, letterSpacing: '-0.02em' }}>{fw.name}</div>
                  <div style={{ fontFamily: MONO, fontSize: 22, marginTop: 26, color: C.khaki, whiteSpace: 'nowrap' }}>{fw.code}</div>
                </div>
              )
            })}
          </div>
        )
      })()}

      {/* Three: four dialects fly together into one canonical route. */}
      {(() => {
        const on = stage(f, CH.three[0], CH.three[1])
        if (on <= 0) return null
        const merge = easeInOut((f - (CH.three[0] + 64)) / 30)
        const canon = easeOut((f - (CH.three[0] + 88)) / 18)
        return (
          <div style={{ position: 'absolute', inset: 0, opacity: on }}>
            {DIALECTS.map((d, i) => {
              const t = easeOut((f - (CH.three[0] + 14 + i * 9)) / 18)
              const x = 300 + (960 - 300) * merge
              const y = 400 + i * 110 + (565 - (400 + i * 110)) * merge
              return (
                <div key={d} style={abs(x, y, { transform: `translate(${-50 * merge}%, -50%) scale(${1 - 0.25 * merge})`, opacity: t * (1 - canon), fontFamily: MONO, fontSize: 36, padding: '14px 30px', borderRadius: 999, border: `2px solid ${C.ink}`, whiteSpace: 'nowrap' })}>
                  {d}
                </div>
              )
            })}
            {canon > 0 && (
              <div style={abs(960, 565, { transform: `translate(-50%, -50%) scale(${0.7 + 0.3 * canon})`, opacity: canon, fontFamily: MONO, fontSize: 48, fontWeight: 600, padding: '22px 44px', borderRadius: 999, background: C.ink, color: C.manilla, whiteSpace: 'nowrap', boxShadow: `0 0 0 ${10 * (1 - canon)}px rgba(138,74,121,0.3)` })}>
                (METHOD, /orders/{'{*}'})
              </div>
            )}
          </div>
        )
      })()}

      {/* Four and five: the chain across the boundary, then proven. */}
      {chainOn > 0 && (
        <div style={{ position: 'absolute', inset: 0, opacity: chainOn }}>
          <svg width={1920} height={1080} style={{ position: 'absolute', inset: 0 }}>
            {edges.map((e, i) => {
              const t = easeOut((f - e.at) / 26)
              const a = nodes[i].x + 50
              const b = nodes[i + 1].x - 50
              const u = ((f - e.at - 26) % 40) / 40
              return (
                <g key={e.label}>
                  <line x1={a} y1={560} x2={a + (b - a) * t} y2={560} stroke={C.pinkDeep} strokeWidth={5} strokeLinecap="round" />
                  {t >= 1 && u >= 0 && <circle cx={a + (b - a) * u} cy={560} r={9} fill={C.pink} />}
                  <text x={(a + b) / 2} y={530} textAnchor="middle" style={{ font: `600 24px ${MONO.split(',')[0]}` }} fill={C.pinkDeep} opacity={t}>
                    {e.label}
                  </text>
                  {(() => {
                    const ok = easeOut((f - (CH.five[0] + 16 + i * 14)) / 14)
                    if (ok <= 0) return null
                    return (
                      <g transform={`translate(${(a + b) / 2} 610) scale(${ok})`}>
                        <circle r={24} fill={C.pinkDeep} />
                        <path d="M-10 0 L-3 7 L10 -7" fill="none" stroke={C.white} strokeWidth={4.5} strokeLinecap="round" strokeLinejoin="round" />
                      </g>
                    )
                  })()}
                </g>
              )
            })}
          </svg>
          {nodes.map((n, i) => {
            const t = easeOut((f - (CH.four[0] + 10 + i * 34)) / 18)
            return (
              <div key={n.label} style={abs(n.x, 560, { transform: `translate(-50%, -50%) scale(${t})`, display: 'flex', flexDirection: 'column', alignItems: 'center' })}>
                <div
                  style={{
                    width: n.shape === 'table' ? 110 : 80,
                    height: n.shape === 'table' ? 72 : 80,
                    borderRadius: n.shape === 'circle' ? 99 : n.shape === 'table' ? 14 : 6,
                    background: n.shape === 'table' ? 'transparent' : C.ink,
                    border: n.shape === 'table' ? `6px solid ${C.ink}` : undefined,
                    boxSizing: 'border-box',
                  }}
                />
                <div style={{ position: 'absolute', top: '100%', marginTop: 18, fontFamily: MONO, fontSize: 22, whiteSpace: 'nowrap', color: C.grey }}>{n.label}</div>
              </div>
            )
          })}
          <div style={abs(0, 720, { width: 1920, textAlign: 'center', fontFamily: MONO, fontSize: 26, color: C.grey, opacity: stage(f, CH.four[0] + 100, CH.four[1] + 6) })}>
            <span style={{ color: C.ink }}>`${'{BASE}'}/orders/${'{id}'}`</span> → <span style={{ color: C.pinkDeep }}>/api/v1/orders/{'{*}'}</span> · evaluated, never guessed
          </div>
          <div style={abs(0, 720, { width: 1920, textAlign: 'center', fontFamily: MONO, fontSize: 24, color: C.grey, opacity: stage(f, CH.five[0] + 30, total + 40) })}>
            M07 · a full chain on a real repository&nbsp;&nbsp;·&nbsp;&nbsp;M08 · precision and recall, failures included
          </div>
        </div>
      )}

      <Rail pos={pos} />

      {/* Sound */}
      <Typing from={CH.one[0] + 18} to={CH.one[0] + 40} every={2} volume={0.45} />
      {TS_BOXES.map((b) => <Sfx key={b.tag} at={b.at} name="pop" volume={0.6} />)}
      {FRAMEWORKS.map((fw) => <Sfx key={fw.name} at={fw.at} name="whoosh-short" volume={0.35} />)}
      {DIALECTS.map((d, i) => <Sfx key={d} at={CH.three[0] + 14 + i * 9} name="pop" volume={0.5} />)}
      <Sfx at={CH.three[0] + 62} name="whoosh" volume={0.5} />
      <Sfx at={CH.three[0] + 88} name="chime" volume={0.7} />
      {nodes.map((n, i) => <Sfx key={n.label} at={CH.four[0] + 10 + i * 34} name="pop" volume={0.6} />)}
      {edges.map((e) => <Sfx key={e.label} at={e.at} name="connect" volume={0.6} />)}
      {edges.map((e) => <Sfx key={`d${e.label}`} at={e.at + 28} name="data" volume={0.45} />)}
      <Sfx at={CH.five[0] + 16} name="chime" volume={0.45} />
      <Sfx at={CH.five[0] + 30} name="chime" volume={0.35} />
    </Scene>
  )
}

// ─── Tools: M09 → M16, then now ────────────────────────────────────────────
const TOOLS = [
  { m: 'M09', name: 'Command line', line: 'analyze, trace, JSON output' },
  { m: 'M10', name: 'Incremental engine', line: 're-analyses only what a change touches' },
  { m: 'M11', name: 'MAP', line: 'a desktop map; click an edge for its evidence' },
  { m: 'M12', name: 'Blast radius', line: 'what depends on this, across languages' },
  { m: 'M13', name: 'Structural diff', line: 'what a change did to the architecture' },
  { m: 'M14', name: 'PR review', line: 'that diff, as a pull-request comment' },
  { m: 'M15', name: 'MCP server', line: 'the graph, for coding agents' },
  { m: 'M16', name: 'ASK', line: 'explanations that must cite evidence' },
]
const TOOL_AT = (i: number) => 40 + i * 18
const NOW_AT = 236

export const Tools: React.FC<P> = ({ total }) => {
  const f = useCurrentFrame()
  const reached = TOOLS.reduce((n, _, i) => (f >= TOOL_AT(i) ? i : n), -1)
  // The marker steps one milestone per card, then on to M17.
  const pos = reached < 0 ? 8 : f < NOW_AT ? 8 + reached + easeOut((f - TOOL_AT(reached)) / 12) : 16 + easeInOut((f - NOW_AT) / 24)
  const now = prog(f, NOW_AT, 22)
  const cardsOut = prog(f, NOW_AT - 6, 22, GLIDE)
  return (
    <Scene total={total} bg={C.ink} color={C.manilla} push={0} vignette style={{ padding: 0 }}>
      <div style={abs(140, 60)}>
        <Label at={0} color={C.khaki}>How it grew</Label>
      </div>
      <div style={abs(140, 120, { opacity: 1 - cardsOut, fontSize: 80, fontWeight: 600, letterSpacing: '-0.03em' })}>
        {'Then it became tools.'.split(' ').map((w, i) => {
          const t = prog(f, 4 + i * 3, 18)
          return (
            <React.Fragment key={i}>
              <span style={{ display: 'inline-block', fontSize: 80, fontWeight: 600, letterSpacing: '-0.03em', opacity: t, transform: `translateY(${(1 - t) * 30}px)`, filter: t < 1 ? `blur(${(1 - t) * 10}px)` : undefined }}>{w}</span>{' '}
            </React.Fragment>
          )
        })}
      </div>
      <div style={abs(140, 300, { width: 1640, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 24, opacity: 1 - cardsOut, transform: `scale(${1 - 0.06 * cardsOut})`, transformOrigin: '50% 40%', filter: cardsOut > 0 ? `blur(${cardsOut * 8}px)` : undefined })}>
        {TOOLS.map((t, i) => {
          const p = easeOut((f - TOOL_AT(i)) / 18)
          return (
            <div
              key={t.m}
              style={{
                height: 270,
                boxSizing: 'border-box',
                borderRadius: 20,
                padding: '24px 26px',
                background: i % 3 === 0 ? '#1c241a' : '#1b1b1b',
                border: `1.5px solid ${p > 0.9 && i === reached ? C.pink : '#383838'}`,
                opacity: p,
                transform: `translateY(${(1 - p) * 60}px) scale(${0.92 + 0.08 * p})`,
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'space-between',
                fontFamily: FONT,
              }}
            >
              <div style={{ fontFamily: MONO, fontSize: 20, color: C.pink }}>{t.m}</div>
              <div>
                <div style={{ fontSize: 40, fontWeight: 600, letterSpacing: '-0.02em', lineHeight: 1.05 }}>{t.name}</div>
                <div style={{ fontSize: 22, color: C.khaki, marginTop: 12, lineHeight: 1.3 }}>{t.line}</div>
              </div>
            </div>
          )
        })}
      </div>
      {/* Now. */}
      {now > 0 && (
        <div style={abs(140, 380, { opacity: now })}>
          <div style={{ fontSize: 190, fontWeight: 600, letterSpacing: '-0.05em', lineHeight: 1, transform: `scale(${1.2 - 0.2 * now})`, transformOrigin: 'left center', filter: now < 1 ? `blur(${(1 - now) * 16}px)` : undefined }}>
            Now<span style={{ color: C.pink }}>.</span>
          </div>
          <Appear at={NOW_AT + 22}>
            <div style={{ fontFamily: MONO, fontSize: 28, color: C.khaki, marginTop: 24 }}>M17 · the public launch, in progress</div>
          </Appear>
        </div>
      )}
      <Rail pos={Math.min(17, pos)} dark />

      {TOOLS.map((t, i) => <Sfx key={t.m} at={TOOL_AT(i)} name="pop" volume={0.55} />)}
      <Sfx at={NOW_AT - 34} name="riser" volume={0.6} />
      <Sfx at={NOW_AT} name="thump" volume={0.9} />
    </Scene>
  )
}
