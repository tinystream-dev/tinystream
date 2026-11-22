// SPDX-License-Identifier: AGPL-3.0-or-later

import { type CSSProperties, useLayoutEffect, useRef, useState } from 'react'

export function Overview({ text, style, lines = 4, long: after = 420, className = 'mt-8 max-w-[68ch]' }: {
  text: string
  style?: CSSProperties
  lines?: number
  /** How many characters it takes before it might need clamping. */
  long?: number
  className?: string
}) {
  const [open, setOpen] = useState(false)
  const [preview, setPreview] = useState<{ source: string; value: string } | null>(null)
  const paragraph = useRef<HTMLParagraphElement>(null)
  const long = text.length > after

  useLayoutEffect(() => {
    const el = paragraph.current
    if (!el || !long) return
    const boundaries = [0, ...Array.from(new Intl.Segmenter(undefined, { granularity: 'word' }).segment(text))
      .filter((s) => s.isWordLike).map((s) => s.index + s.segment.length)]
    const measure = () => {
      const css = getComputedStyle(el)
      if (parseFloat(css.width) <= 0) return
      // Measure whole words with their ellipsis; native line-clamp can cut the last word in half.
      const probe = el.cloneNode(false) as HTMLParagraphElement
      probe.setAttribute('aria-hidden', 'true')
      Object.assign(probe.style, {
        position: 'absolute', visibility: 'hidden', pointerEvents: 'none',
        width: css.width, height: 'auto', maxHeight: 'none', overflow: 'hidden',
      })
      el.parentElement!.appendChild(probe)
      try {
        const maxHeight = Math.ceil(parseFloat(css.lineHeight) * lines)
        const fits = (value: string) => {
          probe.textContent = value
          return probe.scrollHeight <= maxHeight && probe.scrollWidth <= el.clientWidth
        }
        let value = text
        if (!fits(text)) {
          let low = 0
          let high = boundaries.length - 1
          while (low < high) {
            const mid = Math.ceil((low + high) / 2)
            if (fits(`${text.slice(0, boundaries[mid]).trimEnd()}…`)) low = mid
            else high = mid - 1
          }
          value = `${text.slice(0, boundaries[low]).trimEnd()}…`
        }
        setPreview((old) => old?.source === text && old.value === value ? old : { source: text, value })
      } finally {
        probe.remove()
      }
    }
    let width = ''
    const resize = () => {
      const next = getComputedStyle(el).width
      if (next === width) return
      width = next
      measure()
    }
    resize()
    const observer = new ResizeObserver(resize)
    observer.observe(el)
    document.fonts.addEventListener('loadingdone', measure)
    return () => {
      observer.disconnect()
      document.fonts.removeEventListener('loadingdone', measure)
    }
  }, [text, long, lines])

  return (
    <div style={style} className={`refreshable ${className}`}>
      <p ref={paragraph} className="text-[15px] leading-relaxed whitespace-pre-line wrap-break-word hyphens-auto text-ink-2">
        {open || !long || preview?.source !== text ? text : preview.value}
      </p>
      {long && (open || (preview?.source === text && preview.value !== text)) && (
        <button className="mt-1.5 text-sm text-ink-3 hover:text-ink" onClick={() => setOpen((o) => !o)}>
          {open ? 'Less' : 'More'}
        </button>
      )}
    </div>
  )
}
