import { continueRender, delayRender, staticFile } from 'remotion'

// Geist and Geist Mono, the site's typefaces, loaded before any frame renders.
const handle = delayRender('Loading Geist')

const faces = [
  new FontFace('Geist', `url(${staticFile('geist.woff2')}) format('woff2')`, { weight: '100 900' }),
  new FontFace('Geist Mono', `url(${staticFile('geist-mono.woff2')}) format('woff2')`, { weight: '100 900' }),
]

Promise.all(faces.map((f) => f.load()))
  .then((loaded) => {
    loaded.forEach((f) => document.fonts.add(f))
    continueRender(handle)
  })
  .catch((err) => {
    console.error('Font load failed', err)
    continueRender(handle)
  })
