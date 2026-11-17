// The commercial's opening: a tape of a 90s monitor. One program, one file,
// one language — then a decade at a time the same screen fills with more
// moving parts, until the set is switched off and today begins.
//
// Industry history in broad strokes only; nothing here is a claim about
// Cartograph.

import React from 'react'
import { interpolate, random, useCurrentFrame } from 'remotion'
import { Sfx, Typing } from '../audio'
import { clamp01, easeInOut, easeOut, useCanvas } from '../broll/canvas'
import { C, FONT, MONO } from '../theme'

export const ORIGINS = { frames: 580, era2: 300, era3: 420, off: 540 }

const SCREEN = { w: 1300, h: 975 }
const PHOSPHOR = '#86f7a0'

type Line = { text: string; at: number; cps?: number; color?: string; indent?: number }
// The 1994 session: a prompt, a file, a build, a greeting.
const SESSION: Line[] = [
  { text: 'READY.', at: 28, cps: 99 },
  { text: 'C:\\> edit hello.c', at: 40, cps: 0.9 },
  { text: '#include <stdio.h>', at: 66, cps: 2.2, color: '#b6ffc6' },
  { text: '', at: 76 },
  { text: 'int main(void) {', at: 78, cps: 2.2, color: '#b6ffc6' },
  { text: '    printf("hello, world\\n");', at: 88, cps: 2.4, color: '#b6ffc6' },
  { text: '    return 0;', at: 104, cps: 2.2, color: '#b6ffc6' },
  { text: '}', at: 112, cps: 2, color: '#b6ffc6' },
  { text: '', at: 116 },
  { text: 'C:\\> cc hello.c', at: 140, cps: 0.9 },
  { text: 'C:\\> hello', at: 172, cps: 0.9 },
  { text: 'hello, world', at: 190, cps: 99, color: '#ffffff' },
]

/** Animated snow for the tape's first frames. */
const Snow: React.FC<{ amount: number }> = ({ amount }) => {
  const f = useCurrentFrame()
  const ref = useCanvas(
    (g) => {
      if (amount <= 0) return
      const img = g.createImageData(325, 244)
      for (let i = 0; i < img.data.length; i += 4) {
        const v = random(`snow${f}-${i}`) * 255
        img.data[i] = img.data[i + 1] = img.data[i + 2] = v
        img.data[i + 3] = 255 * amount
      }
      g.putImageData(img, 0, 0)
    },
    325,
    244,
  )
  return <canvas ref={ref} width={325} height={244} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', imageRendering: 'pixelated' }} />
}

/** Boxes and wires drawn in phosphor: the shape of a system in each decade. */
type Box = { id: string; x: number; y: number; label: string }
const ERA2: Box[] = [
  { id: 'page', x: 330, y: 420, label: 'index.html' },
  { id: 'server', x: 830, y: 420, label: 'server' },
]
const ERA2_WIRES: Array<[string, string, string?]> = [['page', 'server', 'HTTP']]
const ERA3: Box[] = [
  { id: 'web', x: 200, y: 260, label: 'web' },
  { id: 'mobile', x: 200, y: 560, label: 'mobile' },
  { id: 'api', x: 520, y: 410, label: 'api' },
  { id: 'auth', x: 820, y: 210, label: 'auth' },
  { id: 'orders', x: 860, y: 410, label: 'orders' },
  { id: 'queue', x: 820, y: 620, label: 'queue' },
  { id: 'worker', x: 1080, y: 560, label: 'worker' },
  { id: 'db', x: 1100, y: 300, label: 'db' },
]
const ERA3_WIRES: Array<[string, string, string?]> = [
  ['web', 'api'],
  ['mobile', 'api'],
  ['api', 'auth'],
  ['api', 'orders'],
  ['api', 'queue'],
  ['orders', 'db'],
  ['queue', 'worker'],
  ['worker', 'db'],
  ['auth', 'db'],
]

const Diagram: React.FC<{ boxes: Box[]; wires: Array<[string, string, string?]>; at: number; color: string; stagger: number }> = ({ boxes, wires, at, color, stagger }) => {
  const f = useCurrentFrame()
  const by = Object.fromEntries(boxes.map((b) => [b.id, b]))
  return (
    <svg width={SCREEN.w} height={SCREEN.h} style={{ position: 'absolute', inset: 0 }}>
      {wires.map(([a, b, label], i) => {
        const t = easeOut((f - at - 10 - i * stagger) / 16)
        if (t <= 0) return null
        const A = by[a]
        const B = by[b]
        const x2 = A.x + (B.x - A.x) * t
        const y2 = A.y + (B.y - A.y) * t
        return (
          <g key={`${a}-${b}`}>
            <line x1={A.x} y1={A.y} x2={x2} y2={y2} stroke={color} strokeWidth={3} strokeDasharray="10 7" />
            {label && t >= 1 && (
              <text x={(A.x + B.x) / 2} y={A.y - 26} textAnchor="middle" fill={color} style={{ font: `500 28px ${MONO}` }}>
                {label}
              </text>
            )}
          </g>
        )
      })}
      {boxes.map((b, i) => {
        const t = easeOut((f - at - i * stagger) / 12)
        if (t <= 0) return null
        const w = Math.max(150, b.label.length * 19 + 50)
        return (
          <g key={b.id} opacity={t}>
            <rect x={b.x - w / 2} y={b.y - 38} width={w} height={76} fill="#0a0d10" stroke={color} strokeWidth={3} />
            <text x={b.x} y={b.y + 10} textAnchor="middle" fill={color} style={{ font: `500 30px ${MONO}` }}>
              {b.label}
            </text>
          </g>
        )
      })}
    </svg>
  )
}

export const Origins: React.FC = () => {
  const f = useCurrentFrame()
  const { era2, era3, off } = ORIGINS
  // How much "old television" there is: full in 1994, less each decade.
  const crt = f < era2 ? 1 : f < era3 ? 0.75 : 0.55
  const ink = f < era2 ? PHOSPHOR : f < era3 ? '#cfe6ff' : C.lightPink
  const flicker = 0.93 + 0.07 * random(`fl${f}`)
  const jitter = (random(`jx${f}`) - 0.5) * 2.2 * crt
  const snow = f < 26 ? 1 - f / 26 : 0
  // A roll between decades: the picture slips vertically for a few frames.
  const roll = (at: number) => (f >= at - 4 && f < at + 6 ? (1 - Math.abs(f - at) / 6) * 140 : 0)
  const slip = roll(era2) + roll(era3)
  // Switch-off: squash to a line, then to a dot, then dark.
  const sq = easeInOut((f - off) / 10)
  const dot = easeInOut((f - off - 10) / 10)
  const gone = clamp01((f - off - 20) / 14)

  const yearRoll = (at: number) => clamp01((f - at) / 10)

  const lines = SESSION.map((l) => {
    const shown = l.cps === undefined ? (f >= l.at ? l.text : '') : l.text.slice(0, Math.max(0, Math.floor((f - l.at) * l.cps)))
    return { ...l, shown, visible: f >= l.at }
  })
  const caption = (from: number, to: number) => clamp01((f - from) / 12) * (1 - clamp01((f - to) / 12))

  return (
    <div style={{ position: 'absolute', inset: 0, background: '#050505', overflow: 'hidden' }}>
      {/* The set. */}
      <div
        style={{
          position: 'absolute',
          left: (1920 - SCREEN.w) / 2,
          top: (1080 - SCREEN.h) / 2,
          width: SCREEN.w,
          height: SCREEN.h,
          borderRadius: 56,
          overflow: 'hidden',
          background: f < era2 ? 'radial-gradient(ellipse at 50% 45%, #0f1d12 0%, #050a06 75%)' : 'radial-gradient(ellipse at 50% 45%, #141a22 0%, #07090c 75%)',
          transform: `translateX(${jitter}px) scale(${1 - 0.99 * dot}, ${1 - 0.995 * sq})`,
          filter: `brightness(${(1 + 1.6 * sq) * flicker}) blur(${0.5 * crt}px)`,
          opacity: 1 - gone,
          boxShadow: 'inset 0 0 120px rgba(0,0,0,0.9), 0 0 0 2px rgba(255,255,255,0.05), 0 0 80px rgba(120,255,150,0.05)',
        }}
      >
        <div style={{ position: 'absolute', inset: 0, transform: `translateY(${-slip}px)` }}>
          {/* 1994: the terminal. */}
          {f < era2 && (
            <div style={{ position: 'absolute', left: 90, top: 80, fontFamily: MONO, fontSize: 38, lineHeight: 1.45, color: ink, textShadow: `0 0 8px ${ink}, 2px 0 0 rgba(255,60,60,0.35), -2px 0 0 rgba(60,140,255,0.35)` }}>
              {lines
                .filter((l) => l.visible)
                .map((l, i) => (
                  <div key={i} style={{ whiteSpace: 'pre', minHeight: '1.45em', color: l.color }}>
                    {l.shown}
                    {/* The cursor rides the newest line. */}
                    {i === lines.filter((x) => x.visible).length - 1 && Math.floor(f / 8) % 2 === 0 && <span>█</span>}
                  </div>
                ))}
            </div>
          )}
          {/* 2004 and 2014: the systems grow. */}
          {f >= era2 && f < era3 && <Diagram boxes={ERA2} wires={ERA2_WIRES} at={era2 + 4} color={ink} stagger={10} />}
          {f >= era3 && <Diagram boxes={ERA3} wires={ERA3_WIRES} at={era3 + 2} color={ink} stagger={4} />}

          {/* Captions, in the tape's own lower third. */}
          <div style={{ position: 'absolute', left: 0, right: 0, bottom: 90, textAlign: 'center', fontFamily: FONT, color: '#f4f1e8', textShadow: '0 2px 0 rgba(0,0,0,0.8), 0 0 14px rgba(0,0,0,0.9)' }}>
            {f < era2 && (
              <div style={{ opacity: caption(212, 292) }}>
                <div style={{ fontSize: 46, fontWeight: 500 }}>One file. One language.</div>
                <div style={{ fontSize: 30, fontWeight: 400, marginTop: 10, opacity: clamp01((f - 236) / 12) }}>You could read the whole program.</div>
              </div>
            )}
            {f >= era2 && f < era3 && (
              <div style={{ opacity: caption(era2 + 30, era3 - 6) }}>
                <div style={{ fontSize: 46, fontWeight: 500 }}>A page and a server.</div>
              </div>
            )}
            {f >= era3 && (
              <div style={{ opacity: caption(era3 + 34, off - 4) }}>
                <div style={{ fontSize: 46, fontWeight: 500 }}>Then services. APIs. Frameworks.</div>
                <div style={{ fontSize: 30, fontWeight: 400, marginTop: 10, opacity: clamp01((f - era3 - 56) / 12) }}>More than one language in one system.</div>
              </div>
            )}
          </div>
        </div>

        {/* The glass: scanlines, a rolling bar, snow, vignette. */}
        <div style={{ position: 'absolute', inset: 0, opacity: 0.55 * crt, backgroundImage: 'repeating-linear-gradient(0deg, rgba(0,0,0,0.55) 0px, rgba(0,0,0,0.55) 2px, transparent 2px, transparent 5px)' }} />
        <div style={{ position: 'absolute', left: 0, right: 0, height: 180, top: ((f * 6) % (SCREEN.h + 360)) - 180, background: 'linear-gradient(transparent, rgba(255,255,255,0.045), transparent)' }} />
        {snow > 0 && (
          <div style={{ position: 'absolute', inset: 0, opacity: snow }}>
            <Snow amount={1} />
          </div>
        )}
        <div style={{ position: 'absolute', inset: 0, background: 'radial-gradient(ellipse at 50% 50%, transparent 55%, rgba(0,0,0,0.75) 100%)' }} />
      </div>

      {/* The tape's on-screen display, over the set. */}
      <div style={{ position: 'absolute', inset: 0, opacity: 1 - clamp01((f - off) / 8), fontFamily: MONO, color: '#f2f2f2', textShadow: '2px 2px 0 rgba(0,0,0,0.85)' }}>
        <div style={{ position: 'absolute', left: 120, top: 70, fontSize: 40, letterSpacing: '0.06em', opacity: f < 70 || Math.floor(f / 20) % 2 === 0 ? 1 : 0.0 }}>PLAY ▶</div>
        <div style={{ position: 'absolute', right: 120, bottom: 70, fontSize: 40, letterSpacing: '0.08em', overflow: 'hidden', height: 52 }}>
          <div style={{ transform: `translateY(${-52 * (f >= era3 ? 1 + yearRoll(era3) : yearRoll(era2))}px)` }}>
            <div style={{ height: 52 }}>1994</div>
            <div style={{ height: 52 }}>2004</div>
            <div style={{ height: 52 }}>2014</div>
          </div>
        </div>
        <div style={{ position: 'absolute', left: 120, bottom: 70, fontSize: 26, opacity: 0.8 }}>SP&nbsp;&nbsp;0:{String(Math.floor(f / 30)).padStart(2, '0')}:{String(f % 30).padStart(2, '0')}</div>
      </div>
      {/* A white dot lingers as the set dies. */}
      {f >= off + 10 && (
        <div style={{ position: 'absolute', left: 960 - 6, top: 540 - 6, width: 12, height: 12, borderRadius: 12, background: '#fff', boxShadow: '0 0 30px 10px rgba(255,255,255,0.7)', opacity: interpolate(f, [off + 10, off + 22, off + 36], [0, 1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' }) }} />
      )}

      {/* Sound */}
      <Sfx at={0} name="static" volume={0.9} />
      <Sfx at={28} name="beep" volume={0.5} />
      <Typing from={40} to={60} every={2} volume={0.9} />
      <Typing from={66} to={116} every={2} volume={0.8} />
      <Typing from={140} to={156} every={2} volume={0.9} />
      <Sfx at={158} name="hdd" volume={0.8} />
      <Typing from={172} to={182} every={2} volume={0.9} />
      <Sfx at={190} name="beep-low" volume={0.45} />
      <Sfx at={era2 - 4} name="static" volume={0.35} />
      <Sfx at={era3 - 4} name="static" volume={0.35} />
      {ERA3.map((b, i) => <Sfx key={b.id} at={era3 + 2 + i * 4} name="beep-low" volume={0.12} />)}
      <Sfx at={off} name="crt-off" volume={1} />
    </div>
  )
}
