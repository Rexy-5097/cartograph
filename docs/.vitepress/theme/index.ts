// The site theme: VitePress's default theme for the documentation pages,
// re-skinned by style.css, plus the landing page the home route renders.
//
// Geist and Geist Mono are bundled from npm rather than fetched from a font
// CDN, so a visit to the site makes no third-party request (ADR-0005). The
// default theme is imported without its own font, Inter, which nothing uses.
//
// The documentation pages' navigation bar carries Cartograph's mark before
// the site title, inside the title's link, so it answers that link's hover
// and focus. It does not idle there: those pages have no pause control.

import { h } from 'vue'
import DefaultTheme from 'vitepress/theme-without-fonts'
import type { Theme } from 'vitepress'
import '@fontsource-variable/geist'
import '@fontsource-variable/geist-mono'
import './style.css'
import CartographMark from './CartographMark.vue'
import Landing from './Landing.vue'
import EdgeField from './graphics/EdgeField.vue'
import TraceChain from './graphics/TraceChain.vue'

export default {
  extends: DefaultTheme,
  Layout: () => h(DefaultTheme.Layout, null, { 'nav-bar-title-before': () => h(CartographMark) }),
  enhanceApp({ app }) {
    app.component('Landing', Landing)
    // Moving graphics the documentation pages can place in their Markdown.
    // On those pages they play once and rest: there is no pause control.
    app.component('TraceChain', TraceChain)
    app.component('EdgeField', EdgeField)
  },
} satisfies Theme
