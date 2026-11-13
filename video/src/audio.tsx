// Sound placement. The files are synthesised by audio/synth.py into
// public/audio/. Effects sit inside each scene's own frame count, so they move
// with the scene when its timing changes.

import React from 'react'
import { Html5Audio, Sequence, interpolate, staticFile } from 'remotion'

export type SfxName =
  | 'pop'
  | 'connect'
  | 'data'
  | 'chime'
  | 'whoosh-short'
  | 'whoosh'
  | 'whoosh-long'
  | 'swish'
  | 'thump'
  | 'riser'
  | 'shimmer'
  | 'fall'
  | 'deny'
  | 'static'
  | 'beep'
  | 'beep-low'
  | 'hdd'
  | 'crt-off'

/** Overall effects level against the score. */
const FX = 0.55

export const Sfx: React.FC<{ at: number; name: SfxName; volume?: number }> = ({ at, name, volume = 1 }) => (
  <Sequence from={Math.max(0, Math.round(at))} layout="none">
    <Html5Audio src={staticFile(`audio/${name}.wav`)} volume={volume * FX} />
  </Sequence>
)

/** Keystrokes across a span of typing, every `every` frames, varied. */
export const Typing: React.FC<{ from: number; to: number; every?: number; volume?: number }> = ({ from, to, every = 2, volume = 0.6 }) => {
  const keys: React.ReactNode[] = []
  let n = 0
  for (let f = from; f < to; f += every + ((n * 7) % 3 === 0 ? 1 : 0)) {
    keys.push(
      <Sequence key={f} from={Math.round(f)} layout="none">
        <Html5Audio src={staticFile(`audio/key${(n * 5 + 3) % 6}.wav`)} volume={volume * FX * (0.75 + ((n * 13) % 5) * 0.06)} />
      </Sequence>,
    )
    n++
  }
  return <>{keys}</>
}

/** The ambient score, faded in and out over the whole video. */
export const Score: React.FC<{ total: number }> = ({ total }) => (
  <Html5Audio
    src={staticFile('audio/score.wav')}
    volume={(f) => interpolate(f, [0, 75, total - 150, total - 10], [0, 0.42, 0.42, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })}
  />
)

/** A bed (hum, tune, score) from a frame, with a volume envelope over its own frames. */
export const Bed: React.FC<{ name: string; from: number; frames: number; volume: number; fadeIn?: number; fadeOut?: number; loop?: boolean }> = ({ name, from, frames, volume, fadeIn = 15, fadeOut = 15, loop }) => (
  <Sequence from={from} durationInFrames={frames} layout="none">
    <Html5Audio
      src={staticFile(`audio/${name}.wav`)}
      loop={loop}
      volume={(f) => interpolate(f, [0, fadeIn, frames - fadeOut, frames], [0, volume, volume, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' })}
    />
  </Sequence>
)
