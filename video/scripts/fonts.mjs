// Copies the Geist faces the site uses from @fontsource into public/, where
// Remotion's staticFile() serves them. Run on install; the copies are ignored.
import { copyFileSync, mkdirSync } from 'node:fs'

mkdirSync('public', { recursive: true })
copyFileSync('node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2', 'public/geist.woff2')
copyFileSync('node_modules/@fontsource-variable/geist-mono/files/geist-mono-latin-wght-normal.woff2', 'public/geist-mono.woff2')
