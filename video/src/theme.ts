import { Easing, interpolate } from 'remotion'

// The documentation site's palette (docs/.vitepress/theme/style.css).
export const C = {
  black: '#222222',
  ink: '#1a1a1a',
  grey: '#3e3e3e',
  pink: '#cd8dbd',
  pinkDeep: '#8a4a79',
  lightPink: '#eddfee',
  manilla: '#ede8da',
  manillaDeep: '#e3dccb',
  green: '#1f2d1d',
  khaki: '#b7b39c',
  white: '#ffffff',
}

export const FONT = "'Geist', ui-sans-serif, system-ui, 'Segoe UI', sans-serif"
export const MONO = "'Geist Mono', ui-monospace, Consolas, monospace"

export const FPS = 30
export const WIDTH = 1920
export const HEIGHT = 1080

// The site's easing, --cg-ease.
export const EASE = Easing.bezier(0.22, 1, 0.36, 1)

/** 0 → 1 between `start` and `start + dur` frames, eased, clamped. */
export const prog = (frame: number, start: number, dur: number, easing = EASE) =>
  interpolate(frame, [start, start + dur], [0, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing,
  })

// The site's glide (INTRO_GLIDE in Landing.vue): an even ease-in-out for moves
// that leave, so nothing pops or bounces out.
export const GLIDE = Easing.bezier(0.65, 0, 0.35, 1)

/** Frames of quiet background before a scene's content arrives. */
export const LEAD = 12
/** Frames over which a scene's content glides out. */
export const EXIT = 18
