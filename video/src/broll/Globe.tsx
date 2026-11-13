// The site's hero globe (docs/.vitepress/theme/GlobeField.vue), driven by the
// video's frame instead of the clock: code glyphs on a Fibonacci lattice,
// leaning toward the viewer, flickering, with a bright band raining down its
// face. Optional: it assembles from scattered glyphs, runs edges out to
// points on screen with packets, and carries labels on a tilted orbit.

import React, { useMemo } from 'react'
import { random, useCurrentFrame } from 'remotion'
import { clamp01, easeInOut, easeOut, lerp, useCanvas } from './canvas'

const GLYPHS = '01{}<>[]()/=+*#:;$&|~0123456789ABCDEF'
const TILT = 0.32

export type Orbit = { labels: string[]; rx: number; ry: number; tilt: number; rate: number; at?: number }

type Props = {
  cx: number
  cy: number
  r: number
  count?: number
  spin?: number
  /** [start frame, duration]: glyphs fly in from scattered positions. */
  assemble?: [number, number]
  rgb?: string
  light?: string
  wire?: boolean
  opacity?: number
  /** Points the globe's edges run to, each from its own frame. */
  edges?: Array<{ x: number; y: number; at: number }>
  orbit?: Orbit
  labelBg?: string
}

export const Globe: React.FC<Props> = ({
  cx,
  cy,
  r,
  count = 720,
  spin = 0.009,
  assemble,
  rgb = '205, 141, 189',
  light = '237, 223, 238',
  wire = true,
  opacity = 1,
  edges = [],
  orbit,
  labelBg = 'rgb(26, 26, 26)',
}) => {
  const f = useCurrentFrame()
  const points = useMemo(() => {
    const golden = Math.PI * (3 - Math.sqrt(5))
    return Array.from({ length: count }, (_, i) => {
      const y = 1 - (i / (count - 1)) * 2
      const rr = Math.sqrt(1 - y * y)
      const t = i * golden
      return {
        x: Math.cos(t) * rr,
        y,
        z: Math.sin(t) * rr,
        period: 5 + Math.floor(random(`p${i}`) * 40),
        delay: random(`d${i}`),
        ox: (random(`ox${i}`) - 0.5) * 2600,
        oy: (random(`oy${i}`) - 0.5) * 1600,
      }
    })
  }, [count])

  const ref = useCanvas((g) => {
    const angle = 0.6 + f * spin
    const k = r / 240 // the site's globe is ~240 px across at desktop width
    const built = assemble ? clamp01((f - assemble[0]) / assemble[1]) : 1

    // Orbit geometry, used before and after the globe.
    const onOrbit = (th: number) => {
      if (!orbit) return { x: 0, y: 0, front: true }
      const ex = Math.cos(th) * r * orbit.rx
      const ey = Math.sin(th) * r * orbit.ry
      const ct = Math.cos(orbit.tilt)
      const st = Math.sin(orbit.tilt)
      return { x: cx + ex * ct - ey * st, y: cy + ex * st + ey * ct, front: Math.sin(th) > 0 }
    }
    const orbitIn = orbit ? easeOut((f - (orbit.at ?? 0)) / 30) : 0
    const orbitArc = (from: number, to: number, alpha: number) => {
      if (!orbit) return
      g.save()
      g.translate(cx, cy)
      g.rotate(orbit.tilt)
      g.beginPath()
      g.ellipse(0, 0, r * orbit.rx, r * orbit.ry, 0, from, to)
      g.setLineDash([3, 9])
      g.strokeStyle = `rgba(${rgb}, ${alpha * orbitIn})`
      g.lineWidth = 1.6
      g.stroke()
      g.restore()
      g.setLineDash([])
    }
    orbitArc(Math.PI, Math.PI * 2, 0.25)

    // Wireframe: the rim, the equator and two latitudes.
    if (wire) {
      const w = easeOut(built * 1.2 - 0.2)
      g.lineWidth = 1.2
      g.strokeStyle = `rgba(${rgb}, ${0.3 * w})`
      g.beginPath()
      g.arc(cx, cy, r, 0, Math.PI * 2)
      g.stroke()
      for (const lat of [-Math.PI / 6, 0, Math.PI / 6]) {
        g.strokeStyle = `rgba(${rgb}, ${(lat === 0 ? 0.22 : 0.14) * w})`
        g.beginPath()
        g.ellipse(cx, cy - r * Math.sin(lat) * Math.cos(TILT), r * Math.cos(lat), r * Math.cos(lat) * Math.sin(TILT), 0, 0, Math.PI * 2)
        g.stroke()
      }
    }

    const ca = Math.cos(angle)
    const sa = Math.sin(angle)
    const cb = Math.cos(TILT)
    const sb = Math.sin(TILT)
    const band = ((f * 0.028) % 2.8) - 1.4
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    let lastFont = ''
    points.forEach((p, i) => {
      const x1 = p.x * ca + p.z * sa
      const z1 = -p.x * sa + p.z * ca
      const y2 = p.y * cb - z1 * sb
      const z2 = p.y * sb + z1 * cb
      let sx = cx + x1 * r
      let sy = cy + y2 * r
      let a = 1
      if (assemble) {
        const t = easeInOut((f - assemble[0] - p.delay * assemble[1] * 0.55) / (assemble[1] * 0.45))
        sx = lerp(cx + p.ox, sx, t)
        sy = lerp(cy + p.oy, sy, t)
        a = t
      }
      if (sx < -60 || sx > 1980 || sy < -60 || sy > 1140) return
      const near = (z2 + 1) / 2
      const rain = z2 > 0 ? Math.exp(-((y2 - band) ** 2) / 0.012) : 0
      const period = rain > 0.3 ? 2 : p.period
      const ch = GLYPHS[Math.floor(random(`c${i}-${Math.floor((f + i * 7) / period)}`) * GLYPHS.length)]
      let alpha = 0.1 + 0.62 * near ** 1.6 + rain * 0.7
      if (z2 < 0) alpha *= 0.55
      alpha *= a
      if (alpha < 0.03) return
      const bright = rain > 0.45
      const size = Math.max(4, Math.round((8 + 6 * near) * k))
      const font = `${bright ? 600 : 500} ${size}px "Geist Mono"`
      if (font !== lastFont) {
        g.font = font
        lastFont = font
      }
      g.fillStyle = `rgba(${bright ? light : rgb}, ${Math.min(1, alpha)})`
      g.fillText(ch, sx, sy)
    })

    // Edges from the rim to points on screen, each drawn in, then a packet.
    edges.forEach((e, n) => {
      const t = easeOut((f - e.at) / 22)
      if (t <= 0) return
      const dx = e.x - cx
      const dy = e.y - cy
      const len = Math.hypot(dx, dy) || 1
      const sx = cx + (dx / len) * r * 1.05
      const sy = cy + (dy / len) * r * 1.05
      const bend = 0.16 * (n % 2 ? 1 : -1)
      const qx = (sx + e.x) / 2 - (dy / len) * len * bend
      const qy = (sy + e.y) / 2 + (dx / len) * len * bend
      const at = (u: number) => {
        const v = 1 - u
        return [v * v * sx + 2 * v * u * qx + u * u * e.x, v * v * sy + 2 * v * u * qy + u * u * e.y]
      }
      g.strokeStyle = `rgba(${rgb}, 0.6)`
      g.lineWidth = 2
      g.beginPath()
      g.moveTo(sx, sy)
      for (let s = 1; s <= 24; s++) {
        const [x, y] = at((s / 24) * t)
        g.lineTo(x, y)
      }
      g.stroke()
      g.fillStyle = `rgb(${rgb})`
      g.beginPath()
      g.arc(sx, sy, 3.5, 0, Math.PI * 2)
      g.fill()
      if (t >= 1) {
        g.beginPath()
        g.arc(e.x, e.y, 5, 0, Math.PI * 2)
        g.fill()
        const u = ((f - e.at - 22 + n * 11) % 45) / 30
        if (u >= 0 && u < 1) {
          const [px, py] = at(u)
          g.fillStyle = `rgb(${light})`
          g.beginPath()
          g.arc(px, py, 4, 0, Math.PI * 2)
          g.fill()
        }
      }
    })

    orbitArc(0, Math.PI, 0.55)
    if (orbit) {
      const labels = orbit.labels.map((text, n) => ({ text, ...onOrbit(f * spin * orbit.rate + (n * Math.PI * 2) / orbit.labels.length) }))
      const pill = (l: { text: string; x: number; y: number; front: boolean }) => {
        const strength = (l.front ? 1 : 0.4) * orbitIn
        g.font = `600 ${Math.round(15 * k)}px "Geist Mono"`
        const w = g.measureText(l.text).width + 30 * k
        const h = 32 * k
        g.globalAlpha = orbitIn
        g.fillStyle = labelBg
        g.strokeStyle = `rgba(${rgb}, ${0.75 * strength})`
        g.lineWidth = 1.4
        g.beginPath()
        g.roundRect(l.x - w / 2, l.y - h / 2, w, h, h / 2)
        g.fill()
        g.stroke()
        g.fillStyle = `rgba(${light}, ${strength})`
        g.fillText(l.text, l.x, l.y + 1)
        g.globalAlpha = 1
      }
      labels.filter((l) => !l.front).forEach(pill)
      labels.filter((l) => l.front).forEach(pill)
    }
  })

  return <canvas ref={ref} width={1920} height={1080} style={{ position: 'absolute', inset: 0, opacity }} />
}
