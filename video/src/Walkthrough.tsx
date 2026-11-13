import React from 'react'
import { AbsoluteFill } from 'remotion'
import { linearTiming, TransitionPresentation, TransitionSeries } from '@remotion/transitions'
import { Score, Sfx, SfxName } from './audio'
import { COLD_GLOBE, ColdOpen, Creed, MEASURED_FP, Measured, Outro, Pipeline, Reads, Statement, Stats, SURF_GLOBE, Surfaces, Title, Trace } from './scenes'
import { Roots, Tools } from './story'
import { C } from './theme'
import { globeZoom, halftone, iris } from './transitions'

// Scene lengths in frames at 30 fps, and how each hands over to the next.
const SCENES: Array<{ C: React.FC<{ total: number }>; frames: number }> = [
  { C: ColdOpen, frames: 210 },
  { C: Title, frames: 200 },
  { C: Roots, frames: 660 },
  { C: Tools, frames: 300 },
  { C: Statement, frames: 260 },
  { C: Reads, frames: 300 },
  { C: Pipeline, frames: 330 },
  { C: Trace, frames: 400 },
  { C: Measured, frames: 320 },
  { C: Stats, frames: 260 },
  { C: Creed, frames: 290 },
  { C: Surfaces, frames: 300 },
  { C: Outro, frames: 280 },
]

// The rail's M08 tick, where Roots hands over to Tools.
const M08 = { x: 180 + (1560 * 8) / 17, y: 975 }

type Handover = { p: TransitionPresentation<never>; frames: number; sound: SfxName; lead: number; volume: number }
const zoom = (p: { x: number; y: number }, frames = 36): Handover => ({ p: globeZoom(p) as never, frames, sound: 'whoosh-long', lead: 8, volume: 0.75 })
const dots = (from: 'left' | 'right' | 'top' | 'bottom'): Handover => ({ p: halftone({ from }) as never, frames: 32, sound: 'swish', lead: 0, volume: 0.6 })
const circle = (p: { x: number; y: number; ring?: string }, frames = 28): Handover => ({ p: iris(p) as never, frames, sound: 'whoosh-short', lead: 2, volume: 0.55 })

const HANDOVERS: Handover[] = [
  zoom(COLD_GLOBE), // fly into the globe
  dots('left'),
  circle({ ...M08, ring: C.pinkDeep }, 30), // out of the rail, into the tools
  circle({ x: 960, y: 540, ring: C.pink }),
  circle({ x: 1700, y: 900, ring: C.pink }),
  dots('bottom'),
  zoom({ x: 960, y: 560 }, 34), // into the graph, onto the trace
  dots('top'),
  circle({ ...MEASURED_FP, ring: C.pinkDeep }, 30), // out of the false positive
  circle({ x: 960, y: 540, ring: C.ink }),
  dots('right'),
  zoom(SURF_GLOBE), // into the core
]

export const WALKTHROUGH_FRAMES = SCENES.reduce((n, s) => n + s.frames, 0) - HANDOVERS.reduce((n, h) => n + h.frames, 0)

// Where each handover begins on the whole timeline.
const HANDOVER_AT = HANDOVERS.map((_, i) => SCENES.slice(0, i + 1).reduce((n, s) => n + s.frames, 0) - HANDOVERS.slice(0, i + 1).reduce((n, h) => n + h.frames, 0))

export const Walkthrough: React.FC = () => (
  <AbsoluteFill>
    <TransitionSeries>
      {SCENES.flatMap(({ C: Scene, frames }, i) => {
        const parts = [
          <TransitionSeries.Sequence key={`s${i}`} durationInFrames={frames}>
            <Scene total={frames} />
          </TransitionSeries.Sequence>,
        ]
        const h = HANDOVERS[i]
        if (h) parts.push(<TransitionSeries.Transition key={`t${i}`} presentation={h.p} timing={linearTiming({ durationInFrames: h.frames })} />)
        return parts
      })}
    </TransitionSeries>
    <Score total={WALKTHROUGH_FRAMES} />
    {HANDOVERS.map((h, i) => (
      <Sfx key={i} at={HANDOVER_AT[i] - h.lead} name={h.sound} volume={h.volume} />
    ))}
  </AbsoluteFill>
)
