// Motion for the landing page, written against the platform rather than an
// animation library: one scroll loop, IntersectionObserver for entrances, and
// CSS for the animation itself. JavaScript only writes custom properties and
// classes. Everything here moves transform and opacity, never layout, so none
// of it can shift the page; and every helper stands down when `still()` says
// the viewer has paused motion or asked the system for less of it.

export type Still = () => boolean
type Stop = () => void

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v))

// A canvas whose draws average more than `slow` ms (a quarter of a 60 Hz
// frame) drops to 30 frames a second, and returns to 60 below `fast` ms.
const PACE = { slow: 4, fast: 2.5 }

/**
 * Paces a canvas animation to about 60 frames a second on any display. On a
 * 120 Hz screen it says to draw every other frame, on 240 Hz every fourth, so
 * a decorative canvas costs no more than it would at 60 Hz and its cadence
 * stays even. On a machine where drawing is slow (`took` reports each draw),
 * it settles at 30 a second: steady motion rather than frames dropped at
 * random. The animation's own clock comes from the frame time, so motion
 * keeps its speed at any rate.
 */
export function pacer() {
  let last = 0
  let interval = 16.7
  let count = 0
  let cost = 0
  let slow = false
  return {
    /** Whether this frame should draw. */
    due(time: number) {
      if (last) interval += (Math.min(100, time - last) - interval) * 0.1
      last = time
      return count++ % Math.max(1, Math.round((slow ? 33.3 : 16.7) / interval)) === 0
    },
    /** How long the last draw took, in ms. */
    took(ms: number) {
      cost += (ms - cost) * 0.1
      if (cost > PACE.slow) slow = true
      else if (cost < PACE.fast) slow = false
    },
  }
}

// Scroll-linked values ease toward where the scroll puts them over about this
// many ms, so a mouse wheel's 100 px notches glide rather than jump.
const SCROLL_EASE_MS = 90

/**
 * Scroll-linked progress. Each `[data-scroll]` element under `root` gets a
 * `--p` custom property:
 *
 * - `top` — 0 while the element's top is at the viewport's top, 1 once it
 *   has scrolled one viewport height above it: progress away from a pinned
 *   hero, independent of how tall the content after it is.
 * - `enter` — 0 as the element's top enters at the bottom of the viewport, 1
 *   once it has risen to 35% from the top.
 *
 * A `top` element's children read its `--p`, so it is an ordinary, inherited
 * custom property. An `enter` element is the only reader of its own value,
 * so that is written as `--rise`, registered in style.css as not inherited:
 * changing it restyles that one element, not the whole page inside it.
 *
 * Each frame reads every position first and only then writes: a write
 * invalidates style, so reading after one would force a layout per element.
 * A value is written only when it has changed, and it eases toward its
 * target rather than jumping, running frames only until it settles.
 */
export function scrollScene(root: HTMLElement, still: Still): Stop {
  const items = [...root.querySelectorAll<HTMLElement>('[data-scroll]')].map((el) => {
    const top = el.dataset.scroll === 'top'
    return { el, top, name: top ? '--p' : '--rise', target: 0, shown: -1 }
  })
  let frame = 0
  let last = 0

  const update = (now: number) => {
    frame = 0
    if (still()) {
      last = 0
      return
    }
    const vh = window.innerHeight
    for (const it of items) {
      const r = it.el.getBoundingClientRect()
      it.target = it.top ? clamp(-r.top / vh, 0, 1) : clamp((vh - r.top) / (vh * 0.65), 0, 1)
    }
    const k = last ? 1 - Math.exp(-Math.min(64, now - last) / SCROLL_EASE_MS) : 1
    last = now
    let moving = false
    for (const it of items) {
      let next = it.shown < 0 ? it.target : it.shown + (it.target - it.shown) * k
      if (Math.abs(it.target - next) < 0.0005) next = it.target
      else moving = true
      if (next !== it.shown) {
        it.el.style.setProperty(it.name, next.toFixed(4))
        it.shown = next
      }
    }
    if (moving) frame = requestAnimationFrame(update)
    else last = 0
  }
  const request = () => {
    if (!frame) frame = requestAnimationFrame(update)
  }

  window.addEventListener('scroll', request, { passive: true })
  window.addEventListener('resize', request)
  update(performance.now())
  return () => {
    cancelAnimationFrame(frame)
    window.removeEventListener('scroll', request)
    window.removeEventListener('resize', request)
  }
}

/**
 * Pauses the CSS animations inside each `[data-idle]` element while it is off
 * screen. The page's looping animations move parts of inline SVGs, which the
 * browser animates on the main thread, every frame, whether or not anyone
 * can see them.
 */
export function idleOffscreen(root: HTMLElement): Stop {
  const io = new IntersectionObserver((entries) => {
    for (const entry of entries) entry.target.classList.toggle('is-offscreen', !entry.isIntersecting)
  })
  root.querySelectorAll('[data-idle]').forEach((node) => io.observe(node))
  return () => io.disconnect()
}

/** Adds `is-in` to each `[data-reveal]` and `[data-split]` element once it enters. */
export function reveals(root: HTMLElement): Stop {
  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue
        entry.target.classList.add('is-in')
        io.unobserve(entry.target)
      }
    },
    { rootMargin: '0px 0px -10% 0px' },
  )
  root.querySelectorAll('[data-reveal], [data-split]').forEach((node) => io.observe(node))
  return () => io.disconnect()
}

/**
 * Counts each `[data-count]` element up to the figure it already shows when
 * it first enters. The figure is server-rendered and restored exactly at the
 * end, so the published number is never anything but itself; screen readers
 * get it from a visually hidden copy, not from the moving one.
 */
export function counters(root: HTMLElement, still: Still): Stop {
  const io = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue
      io.unobserve(entry.target)
      const el = entry.target as HTMLElement
      const target = el.dataset.count ?? el.textContent ?? ''
      if (still()) {
        el.textContent = target
        continue
      }
      const decimals = (target.split('.')[1] ?? '').length
      const grouped = target.includes(',')
      const value = parseFloat(target.replace(/,/g, ''))
      const format = (v: number) => (grouped ? Math.round(v).toLocaleString('en-GB') : v.toFixed(decimals))
      const start = performance.now()
      const duration = 1600
      const tick = (now: number) => {
        const t = clamp((now - start) / duration, 0, 1)
        const eased = 1 - Math.pow(2, -10 * t)
        el.textContent = t < 1 ? format(value * eased) : target
        if (t < 1) requestAnimationFrame(tick)
      }
      requestAnimationFrame(tick)
    }
  })
  root.querySelectorAll('[data-count]').forEach((node) => io.observe(node))
  return () => io.disconnect()
}

/**
 * A ring that trails the pointer and swells over anything clickable. It is
 * an addition, not a replacement: the system cursor stays. Fine pointers only.
 */
export function cursor(root: HTMLElement, ring: HTMLElement, still: Still): Stop {
  if (!window.matchMedia('(pointer: fine)').matches) return () => {}
  let x = -100
  let y = -100
  let rx = x
  let ry = y
  let frame = 0

  const loop = () => {
    rx += (x - rx) * 0.2
    ry += (y - ry) * 0.2
    ring.style.transform = `translate3d(${rx}px, ${ry}px, 0)`
    frame = Math.abs(x - rx) + Math.abs(y - ry) > 0.2 ? requestAnimationFrame(loop) : 0
  }
  const move = (e: PointerEvent) => {
    if (still()) return
    x = e.clientX
    y = e.clientY
    const target = e.target as Element | null
    ring.classList.toggle('is-over', !!target?.closest('a, button'))
    ring.classList.add('is-on')
    if (!frame) frame = requestAnimationFrame(loop)
  }
  const leave = () => ring.classList.remove('is-on')

  root.addEventListener('pointermove', move)
  document.addEventListener('pointerleave', leave)
  return () => {
    cancelAnimationFrame(frame)
    root.removeEventListener('pointermove', move)
    document.removeEventListener('pointerleave', leave)
  }
}

/** `.cg-button` elements lean a few pixels toward a fine pointer over them. */
export function magnetic(root: HTMLElement, still: Still): Stop {
  if (!window.matchMedia('(pointer: fine)').matches) return () => {}
  const reach = 6
  const move = (e: PointerEvent) => {
    const el = (e.target as Element | null)?.closest<HTMLElement>('.cg-button')
    if (!el || still()) return
    const b = el.getBoundingClientRect()
    el.style.setProperty('--mx', `${(((e.clientX - b.left) / b.width) * 2 - 1) * reach}px`)
    el.style.setProperty('--my', `${(((e.clientY - b.top) / b.height) * 2 - 1) * reach}px`)
  }
  const out = (e: PointerEvent) => {
    const el = (e.target as Element | null)?.closest<HTMLElement>('.cg-button')
    if (el && !el.contains(e.relatedTarget as Node | null)) {
      el.style.removeProperty('--mx')
      el.style.removeProperty('--my')
    }
  }
  root.addEventListener('pointermove', move)
  root.addEventListener('pointerout', out)
  return () => {
    root.removeEventListener('pointermove', move)
    root.removeEventListener('pointerout', out)
  }
}

/**
 * The marquee runs faster while the page is scrolled, then eases back. Its
 * animations are looked up once: getAnimations() brings every style up to
 * date first, which done every frame forced a style recalculation per frame.
 */
export function marqueeVelocity(lists: HTMLElement[], still: Still): Stop {
  let last = window.scrollY
  let boost = 0
  let rate = 1
  let frame = 0
  let animations: Animation[] = []

  const settle = () => {
    boost *= 0.92
    if (!animations.length) animations = lists.flatMap((l) => l.getAnimations())
    const next = 1 + (boost > 0.01 ? boost : 0)
    if (Math.abs(next - rate) > 0.005) {
      rate = next
      for (const a of animations) a.playbackRate = rate
    }
    frame = boost > 0.01 ? requestAnimationFrame(settle) : 0
  }
  const scroll = () => {
    const y = window.scrollY
    if (!still()) boost = Math.min(5, boost + Math.abs(y - last) / 60)
    last = y
    if (!frame) frame = requestAnimationFrame(settle)
  }
  window.addEventListener('scroll', scroll, { passive: true })
  return () => {
    cancelAnimationFrame(frame)
    window.removeEventListener('scroll', scroll)
  }
}
