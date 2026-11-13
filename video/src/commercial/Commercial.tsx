// The commercial: a 90s tape, the decades, then today — the problem, what
// Cartograph does, how, what it holds to, the vision and the tagline.

import React from 'react'
import { AbsoluteFill, useCurrentFrame } from 'remotion'
import { fade } from '@remotion/transitions/fade'
import { linearTiming, TransitionSeries } from '@remotion/transitions'
import { Bed } from '../audio'
import { useCanvas } from '../broll/canvas'
import { How, Problem, Pros, Solution, Vision } from './Modern'
import { ORIGINS, Origins } from './Origins'

const FADE = 20
const SCENES: Array<{ C: React.FC<{ total: number }>; frames: number }> = [
  { C: Problem, frames: 390 },
  { C: Solution, frames: 330 },
  { C: How, frames: 480 },
  { C: Pros, frames: 360 },
  { C: Vision, frames: 450 },
]
const MODERN_FRAMES = SCENES.reduce((n, s) => n + s.frames, 0) - FADE * (SCENES.length - 1)
export const COMMERCIAL_FRAMES = ORIGINS.frames + MODERN_FRAMES

/** Cinema bars, 2.39:1, from the moment today begins. */
const Letterbox: React.FC = () => {
  const f = useCurrentFrame()
  if (f < ORIGINS.frames) return null
  const BAR = 138
  return (
    <>
      <div style={{ position: 'absolute', left: 0, right: 0, top: 0, height: BAR, background: '#000' }} />
      <div style={{ position: 'absolute', left: 0, right: 0, bottom: 0, height: BAR, background: '#000' }} />
    </>
  )
}

/** Fine film grain over everything, new every frame. */
const Grain: React.FC<{ amount: number }> = ({ amount }) => {
  const f = useCurrentFrame()
  const ref = useCanvas(
    (g) => {
      const img = g.createImageData(480, 270)
      let s = (f * 2654435761) >>> 0
      for (let i = 0; i < img.data.length; i += 4) {
        s = (s * 1664525 + 1013904223) >>> 0
        const v = s >>> 24
        img.data[i] = img.data[i + 1] = img.data[i + 2] = v
        img.data[i + 3] = 255
      }
      g.putImageData(img, 0, 0)
    },
    480,
    270,
  )
  return <canvas ref={ref} width={480} height={270} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', opacity: amount, mixBlendMode: 'soft-light', pointerEvents: 'none' }} />
}

export const Commercial: React.FC = () => {
  const f = useCurrentFrame()
  return (
    <AbsoluteFill style={{ background: '#000' }}>
      <TransitionSeries>
        <TransitionSeries.Sequence durationInFrames={ORIGINS.frames}>
          <Origins />
        </TransitionSeries.Sequence>
        {SCENES.flatMap(({ C, frames }, i) => {
          const parts = [
            <TransitionSeries.Sequence key={`s${i}`} durationInFrames={frames}>
              <C total={frames} />
            </TransitionSeries.Sequence>,
          ]
          if (i < SCENES.length - 1) parts.push(<TransitionSeries.Transition key={`t${i}`} presentation={fade()} timing={linearTiming({ durationInFrames: FADE })} />)
          return parts
        })}
      </TransitionSeries>
      {/* Grain belongs to the tape; today is clean (and compresses). */}
      {f < ORIGINS.frames && <Grain amount={0.5} />}
      <Letterbox />

      {/* Beds: the set's hum and a tune for the decades; then the score. */}
      <Bed name="hum" from={0} frames={ORIGINS.off} volume={0.18} fadeIn={10} fadeOut={1} loop />
      <Bed name="retro-tune" from={190} frames={ORIGINS.off - 190} volume={0.42} fadeIn={20} fadeOut={1} />
      <Bed name="cinema" from={ORIGINS.frames} frames={MODERN_FRAMES} volume={1} fadeIn={20} fadeOut={40} />
    </AbsoluteFill>
  )
}
