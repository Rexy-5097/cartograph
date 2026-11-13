import React from 'react'
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion'
import { C, EXIT, FONT, GLIDE, MONO, prog } from './theme'

/**
 * A full-frame scene with the site's padding and a background colour. Its
 * content glides out together over the scene's last frames — one ease-in-out
 * move, never a cut — while the background stays for the transition.
 */
export const Scene: React.FC<{
  total: number
  bg: string
  color: string
  children?: React.ReactNode
  style?: React.CSSProperties
  /** Full-frame moving pictures behind the words. */
  broll?: React.ReactNode
  /** How far the camera pushes in on the B-roll over the scene (0 holds it still). */
  push?: number
  /** The words glide out at the end; off when the transition takes them. */
  exit?: boolean
  vignette?: boolean
}> = ({ total, bg, color, children, style, broll, push = 0.05, exit = true, vignette = false }) => {
  const f = useCurrentFrame()
  const e = exit ? prog(f, total - EXIT, EXIT, GLIDE) : 0
  const life = Math.min(1, f / total)
  return (
    <AbsoluteFill style={{ background: bg }}>
      {broll && <AbsoluteFill style={{ transform: `scale(${1 + push * life})` }}>{broll}</AbsoluteFill>}
      <AbsoluteFill
        style={{
          color,
          fontFamily: FONT,
          padding: '110px 140px',
          textWrap: 'pretty',
          opacity: 1 - e,
          transform: `translateY(${-36 * e}px) scale(${1 + 0.015 * life})`,
          ...style,
        } as React.CSSProperties}
      >
        {children}
      </AbsoluteFill>
      {vignette && <AbsoluteFill style={{ background: 'radial-gradient(ellipse at 50% 50%, transparent 55%, rgba(0,0,0,0.5) 100%)', pointerEvents: 'none' }} />}
    </AbsoluteFill>
  )
}

/** Words that arrive one after another, rising out of a blur. */
export const Words: React.FC<{ text: string; at: number; stagger?: number; dur?: number; style?: React.CSSProperties; wordStyle?: (word: string, i: number) => React.CSSProperties | undefined }> = ({
  text,
  at,
  stagger = 3,
  dur = 22,
  style,
  wordStyle,
}) => {
  const f = useCurrentFrame()
  const words = text.split(' ')
  return (
    <div style={style}>
      {words.map((w, i) => {
        const t = prog(f, at + i * stagger, dur)
        return (
          <React.Fragment key={i}>
            <span
              style={{
                display: 'inline-block',
                opacity: t,
                transform: `translateY(${(1 - t) * 0.45}em) scale(${1 + (1 - t) * 0.08})`,
                filter: t < 1 ? `blur(${(1 - t) * 14}px)` : undefined,
                ...wordStyle?.(w, i),
              }}
            >
              {w}
            </span>
            {i < words.length - 1 ? ' ' : ''}
          </React.Fragment>
        )
      })}
    </div>
  )
}

/**
 * A highlighter stroke swept behind a phrase, left to right. Keep trailing
 * punctuation inside it so a lone "." can never wrap onto a line of its own.
 */
export const Highlight: React.FC<{ at: number; dur?: number; color: string; children: React.ReactNode }> = ({ at, dur = 18, color, children }) => {
  const f = useCurrentFrame()
  const p = prog(f, at, dur, GLIDE)
  return (
    <span
      style={{
        backgroundImage: `linear-gradient(${color}, ${color})`,
        backgroundRepeat: 'no-repeat',
        backgroundSize: `${p * 100}% 0.34em`,
        backgroundPosition: '0 86%',
        boxDecorationBreak: 'clone',
        WebkitBoxDecorationBreak: 'clone',
      }}
    >
      {children}
    </span>
  )
}

/** A figure counting up to its value, in tabular numerals so it doesn't jitter. */
export const CountUp: React.FC<{ to: number; at: number; dur?: number; decimals?: number }> = ({ to, at, dur = 45, decimals = 0 }) => {
  const f = useCurrentFrame()
  const v = to * prog(f, at, dur)
  const text = v.toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals })
  return <span style={{ fontVariantNumeric: 'tabular-nums' }}>{text}</span>
}

/** A line that rises into place from behind a mask, as the site's headings do. */
export const Rise: React.FC<{ at: number; dur?: number; children: React.ReactNode; style?: React.CSSProperties }> = ({
  at,
  dur = 26,
  children,
  style,
}) => {
  const f = useCurrentFrame()
  const t = prog(f, at, dur)
  return (
    <div style={{ overflow: 'hidden', paddingBottom: '0.08em', ...style }}>
      <div style={{ transform: `translateY(${(1 - t) * 110}%)` }}>{children}</div>
    </div>
  )
}

/** Fades and lifts in. */
export const Appear: React.FC<{ at: number; dur?: number; lift?: number; children: React.ReactNode; style?: React.CSSProperties }> = ({
  at,
  dur = 22,
  lift = 24,
  children,
  style,
}) => {
  const f = useCurrentFrame()
  const t = prog(f, at, dur)
  return <div style={{ opacity: t, transform: `translateY(${(1 - t) * lift}px)`, ...style }}>{children}</div>
}

/** The site's small mono label, with a rule that draws in before it. */
export const Label: React.FC<{ at: number; children: React.ReactNode; color?: string }> = ({ at, children, color }) => {
  const f = useCurrentFrame()
  const rule = prog(f, at, 20)
  const text = prog(f, at + 6, 20)
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 18, fontFamily: MONO, fontSize: 24, letterSpacing: '0.08em', textTransform: 'uppercase', color }}>
      <span style={{ width: 56 * rule, height: 2, background: 'currentColor', display: 'block' }} />
      <span style={{ opacity: text }}>{children}</span>
    </div>
  )
}

/**
 * Cartograph's mark (docs/.vitepress/theme/CartographMark.vue): a square node
 * and a round node, joined by an edge that bends into a check, inside a
 * surveyor's brackets. Built from `at`; a packet crosses the edge at `sendAt`.
 */
export const Mark: React.FC<{ at?: number; sendAt?: number[]; size: number; color: string; style?: React.CSSProperties }> = ({
  at = 0,
  sendAt = [],
  size,
  color,
  style,
}) => {
  const f = useCurrentFrame()
  const { fps } = useVideoConfig()
  const EDGE = 'M18.5 28.5 23.5 33.5 32.7 18.3'
  const close = prog(f, at + 3, 27)
  const corners = [
    { d: 'M4 14V4h10', x: -5, y: -5 },
    { d: 'M34 4h10v10', x: 5, y: -5 },
    { d: 'M44 34v10H34', x: 5, y: 5 },
    { d: 'M14 44H4V34', x: -5, y: 5 },
  ]
  const pop = (start: number) => spring({ frame: f - start, fps, config: { damping: 11, stiffness: 160 } })
  const square = pop(at + 16)
  const draw = prog(f, at + 28, 22)

  // The packet in flight, if any, and the round node's ping when it lands.
  const CROSS = 30
  const flying = sendAt.find((s) => f >= s && f < s + CROSS + 12)
  const packetT = flying === undefined ? 0 : prog(f, flying, CROSS)
  const packetOn = flying !== undefined && f < flying + CROSS
  const ping = flying === undefined ? 0 : interpolate(f, [flying + CROSS - 2, flying + CROSS + 5, flying + CROSS + 12], [0, 1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })
  const round = pop(at + 22) * (1 + 0.28 * ping)

  return (
    <svg viewBox="0 0 48 48" width={size} height={size} style={{ overflow: 'visible', ...style }} fill="none" stroke={color} strokeWidth={2.6} strokeLinecap="round" strokeLinejoin="round">
      {corners.map((c) => (
        <path key={c.d} d={c.d} opacity={close} transform={`translate(${c.x * (1 - close)} ${c.y * (1 - close)})`} />
      ))}
      <rect x="10.5" y="24.5" width="8" height="8" transform={`translate(14.5 28.5) scale(${square}) translate(-14.5 -28.5)`} />
      <circle cx="35" cy="14.5" r="4.4" transform={`translate(35 14.5) scale(${round}) translate(-35 -14.5)`} />
      <path d={EDGE} pathLength={100} strokeDasharray={100} strokeDashoffset={100 * (1 - draw)} />
      <path d={EDGE} pathLength={100} strokeWidth={5.5} strokeDasharray="0.1 200" strokeDashoffset={-100 * packetT} opacity={packetOn ? 1 : 0} />
    </svg>
  )
}

/** A quiet field of dots whose size ripples outward from a point. */
export const DotField: React.FC<{ color: string; cx?: number; cy?: number; gap?: number; opacity?: number }> = ({
  color,
  cx = 1500,
  cy = 820,
  gap = 36,
  opacity = 0.35,
}) => {
  const f = useCurrentFrame()
  const dots: React.ReactNode[] = []
  for (let y = gap / 2; y < 1080; y += gap) {
    for (let x = gap / 2; x < 1920; x += gap) {
      const d = Math.hypot(x - cx, y - cy)
      const wave = (Math.sin(d / 70 - f / 9) + 1) / 2
      const fall = Math.max(0, 1 - d / 1400)
      const r = 0.6 + 3.2 * wave * fall
      dots.push(<circle key={`${x}-${y}`} cx={x} cy={y} r={r} />)
    }
  }
  return (
    <svg width={1920} height={1080} style={{ position: 'absolute', inset: 0, opacity }} fill={color}>
      {dots}
    </svg>
  )
}

/** The site's tone colours for cards. */
export const TONES: Record<string, { bg: string; fg: string }> = {
  black: { bg: C.black, fg: C.manilla },
  green: { bg: C.green, fg: C.manilla },
  pink: { bg: C.pink, fg: C.ink },
  light: { bg: C.lightPink, fg: C.ink },
  khaki: { bg: C.khaki, fg: C.ink },
}
