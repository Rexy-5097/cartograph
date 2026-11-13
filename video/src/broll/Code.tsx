// Illustrative source code for the "It reads" scene: a TypeScript client, a
// Python route and an SQLAlchemy model, in layered panes that drift past at
// different depths. Generic examples — not taken from any benchmark corpus.
// Tokens Cartograph extracts are boxed, and their places on screen are
// exported so the scene can fly chips out of them.

import React from 'react'
import { useCurrentFrame } from 'remotion'
import { C, MONO } from '../theme'
import { easeOut } from './canvas'

type Tok = { text: string; c?: string }
export type Pane = {
  title: string
  x: number
  y: number
  w: number
  font: number
  depth: number
  drift: number
  lines: Tok[][]
  /** [line, start column, length, chip index] for the tokens that get boxed. */
  marks: Array<[number, number, number, number]>
}

const K = C.pink
const S = '#b7d3a8'
const F = '#e9c46a'
const D = '#8d8a80'

export const PANES: Pane[] = [
  {
    title: 'web/src/api/orders.ts',
    x: 930,
    y: 96,
    w: 860,
    font: 22,
    depth: 1,
    drift: 0.12,
    lines: [
      [{ text: 'import', c: K }, { text: ' { http } ' }, { text: 'from', c: K }, { text: " './client'", c: S }],
      [{ text: 'import', c: K }, { text: ' type { Order } ' }, { text: 'from', c: K }, { text: " './types'", c: S }],
      [],
      [{ text: 'export async function', c: K }, { text: ' fetchOrder', c: F }, { text: '(id: string) {' }],
      [{ text: '  const', c: K }, { text: ' res = ' }, { text: 'await', c: K }, { text: ' http.' }, { text: 'get', c: F }, { text: '(' }],
      [{ text: '    `/api/orders/${id}`', c: S }],
      [{ text: '  )' }],
      [{ text: '  return', c: K }, { text: ' res.data ' }, { text: 'as', c: K }, { text: ' Order' }],
      [{ text: '}' }],
    ],
    marks: [
      [0, 0, 6, 1],
      [3, 22, 10, 0],
      [4, 25, 3, 2],
      [5, 4, 19, 3],
    ],
  },
  {
    title: 'api/routes/orders.py',
    x: 1010,
    y: 520,
    w: 800,
    font: 20,
    depth: 0.8,
    drift: 0.18,
    lines: [
      [{ text: 'from', c: K }, { text: ' fastapi ' }, { text: 'import', c: K }, { text: ' APIRouter' }],
      [{ text: 'from', c: K }, { text: ' ..models ' }, { text: 'import', c: K }, { text: ' Order' }],
      [],
      [{ text: 'router = APIRouter(prefix=' }, { text: '"/api"', c: S }, { text: ')' }],
      [],
      [{ text: '@router.get(' }, { text: '"/orders/{order_id}"', c: S }, { text: ')' }],
      [{ text: 'def', c: K }, { text: ' get_order', c: F }, { text: '(order_id: str, db):' }],
      [{ text: '    return', c: K }, { text: ' db.get(' }, { text: 'Order', c: F }, { text: ', order_id)' }],
    ],
    marks: [[5, 0, 33, 4]],
  },
  {
    title: 'api/models.py',
    x: 1150,
    y: 880,
    w: 660,
    font: 19,
    depth: 0.6,
    drift: 0.24,
    lines: [
      [{ text: 'class', c: K }, { text: ' Order', c: F }, { text: '(Base):' }],
      [{ text: '    __tablename__ = ' }, { text: '"orders"', c: S }],
      [{ text: '    id = Column(String, primary_key=' }, { text: 'True', c: K }, { text: ')' }],
    ],
    marks: [[0, 0, 18, 5]],
  },
]

const LH = 1.6
const CHAR = 0.6 // Geist Mono advance, in ems

/** Where a pane's line sits on screen at frame f. */
export const paneY = (p: Pane, f: number) => p.y - f * p.drift
export const markBox = (p: Pane, m: [number, number, number, number], f: number) => {
  const [line, col, len] = m
  const top = paneY(p, f) + 64 + line * p.font * LH
  return { x: p.x + 32 + col * p.font * CHAR - 6, y: top - 2, w: len * p.font * CHAR + 12, h: p.font * LH + 4 }
}

export const CodePanes: React.FC<{ boxAt: (chip: number) => number }> = ({ boxAt }) => {
  const f = useCurrentFrame()
  return (
    <>
      {[...PANES].sort((a, b) => a.depth - b.depth).map((p) => {
        const n = PANES.indexOf(p)
        const t = easeOut((f - n * 8) / 30)
        return (
          <div
            key={p.title}
            style={{
              position: 'absolute',
              left: p.x + (1 - t) * 160,
              top: paneY(p, f),
              width: p.w,
              opacity: t * (0.5 + 0.5 * p.depth),
              filter: p.depth < 0.7 ? 'blur(0.6px)' : undefined,
              background: '#151515',
              border: '1px solid #333',
              borderRadius: 16,
              boxShadow: '0 40px 80px rgba(0,0,0,0.45)',
              fontFamily: MONO,
              fontSize: p.font,
              lineHeight: LH,
              color: C.manilla,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 22px', height: 44, borderBottom: '1px solid #2a2a2a', fontSize: 17, color: D }}>
              <i style={{ width: 11, height: 11, borderRadius: 9, background: C.pink }} />
              <i style={{ width: 11, height: 11, borderRadius: 9, background: C.khaki }} />
              <span style={{ marginLeft: 12 }}>{p.title}</span>
            </div>
            <pre style={{ margin: 0, padding: '18px 32px 22px', fontFamily: MONO, position: 'relative' }}>
              {p.lines.map((l, i) => (
                <div key={i} style={{ whiteSpace: 'pre', minHeight: `${LH}em` }}>
                  {l.map((tok, j) => (
                    <span key={j} style={{ color: tok.c }}>
                      {tok.text}
                    </span>
                  ))}
                </div>
              ))}
            </pre>
            {/* Boxes round what Cartograph extracts, drawn on the pane. */}
            {p.marks.map((m) => {
              const b = markBox(p, m, f)
              const on = easeOut((f - boxAt(m[3])) / 10)
              if (on <= 0) return null
              return (
                <div
                  key={m.join('-')}
                  style={{
                    position: 'absolute',
                    left: b.x - p.x,
                    top: b.y - paneY(p, f),
                    width: b.w,
                    height: b.h,
                    border: `2px solid ${C.pink}`,
                    borderRadius: 8,
                    background: 'rgba(205,141,189,0.14)',
                    opacity: on,
                    transform: `scale(${1.15 - 0.15 * on})`,
                  }}
                />
              )
            })}
          </div>
        )
      })}
    </>
  )
}
