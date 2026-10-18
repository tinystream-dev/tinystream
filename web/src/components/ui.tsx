// SPDX-License-Identifier: AGPL-3.0-or-later

import { type CSSProperties, type ComponentPropsWithoutRef, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode, useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { Check, ChevronDown } from 'lucide-react'
import { createPortal } from 'react-dom'
import { useCorners } from '../lib/theme'
import { Squircle } from './Squircle'

type ButtonProps = ComponentPropsWithoutRef<'button'> & {
  variant?: 'primary' | 'quiet' | 'plain' | 'danger'
  size?: 'sm' | 'md' | 'lg'
  radius?: number
}

const variants = {
  primary: 'bg-accent text-on-accent hover:bg-accent-hover active:bg-accent/85',
  quiet: 'bg-panel text-ink hover:bg-float active:bg-panel',
  plain: 'bg-transparent text-ink-2 hover:bg-hover hover:text-ink active:bg-press',
  danger: 'bg-transparent text-danger hover:bg-danger/10 active:bg-danger/15',
}

const sizes = {
  sm: 'h-7 px-2.5 text-[13px] gap-1.5',
  md: 'h-8 px-3 text-sm gap-2',
  lg: 'h-11 px-5 text-[15px] gap-2.5 font-medium',
}

export function Button({ variant = 'quiet', size = 'md', radius, className = '', ...rest }: ButtonProps) {
  return (
    <Squircle
      as="button"
      radius={radius ?? (size === 'lg' ? 14 : 9)}
      edge={variant === 'quiet'}
      className={`inline-flex shrink-0 items-center justify-center whitespace-nowrap transition-colors duration-100 select-none disabled:pointer-events-none disabled:opacity-40 ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    />
  )
}

export function IconButton({
  label,
  className = '',
  children,
  ...rest
}: ComponentPropsWithoutRef<'button'> & { label: string }) {
  const { props, tip } = useTip(label)
  return (
    <Squircle
      as="button"
      radius={8}
      aria-label={label}
      className={`inline-flex size-8 shrink-0 items-center justify-center text-ink-2 transition-colors hover:bg-hover hover:text-ink active:bg-press ${className}`}
      {...props}
      {...rest}
    >
      {children}
      {tip}
    </Squircle>
  )
}

export function Spinner({ className = 'size-5' }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={`animate-spin ${className}`} aria-label="Loading">
      <circle cx="12" cy="12" r="9.5" fill="none" stroke="currentColor" strokeOpacity=".15" strokeWidth="2.5" />
      <path d="M21.5 12A9.5 9.5 0 0 0 12 2.5" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
    </svg>
  )
}

/** A floating layer: squircle panel, lit edge, deep shadow. */
export function Panel({ className = '', children, radius = 14 }: { className?: string; children: ReactNode; radius?: number }) {
  return (
    <div className="lift" style={{ '--lift-radius': `${radius}px` } as CSSProperties}>
      <Squircle radius={radius} edge className={`material bg-float ${className}`}>
        {children}
      </Squircle>
    </div>
  )
}

/**
 * Click-to-open menu anchored to a trigger. Closes on outside click or Escape.
 * `portal` renders it into the body, pinned to the page, for triggers inside a
 * masked box (like a Squircle) that would otherwise clip it.
 */
export function Popover({
  trigger,
  children,
  align = 'end',
  side = 'bottom',
  portal = false,
  open: controlled,
  onOpenChange,
}: {
  trigger: (props: { open: boolean; toggle: () => void }) => ReactNode
  children: (close: () => void) => ReactNode
  align?: 'start' | 'end'
  side?: 'top' | 'bottom'
  portal?: boolean
  /** Opens and closes it from outside, together with `onOpenChange`. */
  open?: boolean
  onOpenChange?: (open: boolean) => void
}) {
  const [inner, setInner] = useState(false)
  const open = controlled ?? inner
  const change = useRef(onOpenChange)
  change.current = onOpenChange
  const setOpen = useCallback((o: boolean) => {
    setInner(o)
    change.current?.(o)
  }, [])
  const [rect, setRect] = useState<DOMRect | null>(null)
  const ref = useRef<HTMLDivElement>(null)
  const menu = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node
      if (!ref.current?.contains(t) && !menu.current?.contains(t)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        setOpen(false)
      }
    }
    document.addEventListener('pointerdown', onDown)
    document.addEventListener('keydown', onKey, true)
    return () => {
      document.removeEventListener('pointerdown', onDown)
      document.removeEventListener('keydown', onKey, true)
    }
  }, [open, setOpen])
  useLayoutEffect(() => {
    if (!open || !portal) return
    const place = () => ref.current && setRect(ref.current.getBoundingClientRect())
    place()
    window.addEventListener('resize', place)
    window.addEventListener('scroll', place, true)
    return () => {
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', place, true)
    }
  }, [open, portal])
  const close = () => setOpen(false)
  const anim = 'animate-[pop_120ms_ease-out]'
  return (
    <div ref={ref} className="relative">
      {trigger({ open, toggle: () => setOpen(!open) })}
      {open && !portal && (
        <div
          className={`absolute z-50 ${align === 'end' ? 'right-0' : 'left-0'} ${side === 'top' ? 'bottom-full mb-2' : 'top-full mt-2'} ${anim}`}
        >
          {children(close)}
        </div>
      )}
      {open &&
        portal &&
        rect &&
        createPortal(
          <div
            ref={menu}
            className={`absolute z-50 ${align === 'end' ? '-translate-x-full' : ''} ${side === 'top' ? '-translate-y-full' : ''} ${anim}`}
            style={{
              left: (align === 'end' ? rect.right : rect.left) + window.scrollX,
              top: (side === 'top' ? rect.top - 8 : rect.bottom + 8) + window.scrollY,
            }}
          >
            {children(close)}
          </div>,
          document.body,
        )}
    </div>
  )
}

/**
 * Details that float beside something once the pointer rests on it, in place
 * of the browser's `title` tooltip. Mouse only: touch has the tap, and a card
 * appearing under a finger just gets in the way.
 */
export function HoverCard({ content, children, className = '' }: { content: ReactNode; children: ReactNode; className?: string }) {
  const ref = useRef<HTMLDivElement>(null)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const [rect, setRect] = useState<DOMRect | null>(null)
  const hide = useCallback(() => {
    clearTimeout(timer.current)
    setRect(null)
  }, [])
  useEffect(() => {
    if (!rect) return
    // It's placed once; anything that moves the page puts it away instead.
    window.addEventListener('scroll', hide, true)
    window.addEventListener('resize', hide)
    window.addEventListener('pointerdown', hide, true)
    return () => {
      window.removeEventListener('scroll', hide, true)
      window.removeEventListener('resize', hide)
      window.removeEventListener('pointerdown', hide, true)
    }
  }, [rect, hide])
  useEffect(() => () => clearTimeout(timer.current), [])
  return (
    <div
      ref={ref}
      className={className}
      onPointerEnter={(e) => {
        if (e.pointerType !== 'mouse') return
        clearTimeout(timer.current)
        timer.current = setTimeout(() => ref.current && setRect(ref.current.getBoundingClientRect()), 450)
      }}
      onPointerLeave={hide}
    >
      {children}
      {rect && createPortal(<Floating anchor={rect}>{content}</Floating>, document.body)}
    </div>
  )
}

/** Beside the anchor, on whichever side has room, kept inside the window. */
function Floating({ anchor, children }: { anchor: DOMRect; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<{ left: number; top: number; side: 'left' | 'right' } | null>(null)
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const { width, height } = el.getBoundingClientRect()
    const gap = 12
    const margin = 12
    const right = anchor.right + gap + width <= window.innerWidth - margin
    const left = right ? anchor.right + gap : Math.max(margin, anchor.left - gap - width)
    const top = Math.max(margin, Math.min(anchor.top, window.innerHeight - height - margin))
    setPos({ left, top, side: right ? 'right' : 'left' })
  }, [anchor])
  return (
    <div
      ref={ref}
      role="tooltip"
      className="pointer-events-none fixed z-50 w-80"
      style={pos ? { left: pos.left, top: pos.top } : { left: 0, top: 0, visibility: 'hidden' }}
    >
      {pos && (
        <div className={pos.side === 'right' ? 'animate-[nudge-right_160ms_ease-out]' : 'animate-[nudge-left_160ms_ease-out]'}>
          <Panel radius={16} className="p-4">
            {children}
          </Panel>
        </div>
      )}
    </div>
  )
}

/**
 * A short label under (or over) something once the pointer rests on it, in
 * place of the browser's `title` tooltip. Spread `props` onto the element and
 * render `tip` anywhere inside it. Mouse only, like `HoverCard`.
 */
export function useTip(label: ReactNode | undefined) {
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const [rect, setRect] = useState<DOMRect | null>(null)
  const hide = useCallback(() => {
    clearTimeout(timer.current)
    setRect(null)
  }, [])
  useEffect(() => {
    if (!rect) return
    window.addEventListener('scroll', hide, true)
    window.addEventListener('resize', hide)
    window.addEventListener('pointerdown', hide, true)
    return () => {
      window.removeEventListener('scroll', hide, true)
      window.removeEventListener('resize', hide)
      window.removeEventListener('pointerdown', hide, true)
    }
  }, [rect, hide])
  useEffect(() => () => clearTimeout(timer.current), [])
  const props = label
    ? {
        onPointerEnter: (e: ReactPointerEvent<HTMLElement>) => {
          if (e.pointerType !== 'mouse') return
          const el = e.currentTarget
          clearTimeout(timer.current)
          timer.current = setTimeout(() => el.isConnected && setRect(el.getBoundingClientRect()), 500)
        },
        onPointerLeave: hide,
      }
    : {}
  const tip = label && rect ? createPortal(<TipBubble anchor={rect}>{label}</TipBubble>, document.body) : null
  return { props, tip }
}

/** `useTip` for plain content: wraps it in a span that carries the tip. */
export function Tip({ label, className, children }: { label: ReactNode | undefined; className?: string; children?: ReactNode }) {
  const { props, tip } = useTip(label)
  return (
    <span className={className} {...props}>
      {children}
      {tip}
    </span>
  )
}

/** Centered under the anchor, or over it when there's no room below. */
function TipBubble({ anchor, children }: { anchor: DOMRect; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null)
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const { width, height } = el.getBoundingClientRect()
    const gap = 6
    const margin = 8
    const below = anchor.bottom + gap + height <= window.innerHeight - margin
    const top = below ? anchor.bottom + gap : anchor.top - gap - height
    const left = Math.max(margin, Math.min(anchor.left + anchor.width / 2 - width / 2, window.innerWidth - width - margin))
    setPos({ left, top })
  }, [anchor])
  return (
    <div
      ref={ref}
      role="tooltip"
      className="pointer-events-none fixed z-[60] max-w-80 material rounded-md break-words bg-float px-2 py-1 text-xs text-ink shadow-(--shadow-tip)"
      style={pos ? { left: pos.left, top: pos.top } : { left: 0, top: 0, visibility: 'hidden' }}
    >
      <div className={pos ? 'animate-[fade_100ms_ease-out]' : undefined}>{children}</div>
    </div>
  )
}

export function MenuItem({
  active,
  children,
  hint,
  onClick,
}: {
  active?: boolean
  children: ReactNode
  hint?: ReactNode
  onClick: () => void
}) {
  return (
    <Squircle
      as="button"
      radius={8}
      onClick={onClick}
      className={`flex w-full items-center gap-3 px-2.5 py-1.5 text-left text-[13px] transition-colors hover:bg-hover ${active ? 'text-ink' : 'text-ink-2'}`}
    >
      <span className={`size-1.5 shrink-0 rounded-full ${active ? 'bg-ink' : 'bg-transparent'}`} />
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {hint && <span className="shrink-0 text-2xs text-ink-3">{hint}</span>}
    </Squircle>
  )
}

export function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[13px] text-ink-2">{label}</span>
      {children}
      {hint && <span className="mt-1.5 block text-xs leading-relaxed text-ink-3">{hint}</span>}
    </label>
  )
}

export function Input({ className = '', ...rest }: ComponentPropsWithoutRef<'input'>) {
  return (
    <Squircle radius={9} edge className="bg-raised">
      <input
        className={`h-9 w-full bg-transparent px-3 text-sm text-ink outline-none placeholder:text-ink-3 ${className}`}
        {...rest}
      />
    </Squircle>
  )
}

/**
 * A dropdown in place of the browser's own. The list floats in the body
 * (so masked boxes don't clip it) under the field, or over it when there's
 * more room above, and follows the keyboard the way a native one does.
 */
export function Select<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T
  options: { value: T; label: string }[]
  onChange: (v: T) => void
}) {
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const [rect, setRect] = useState<DOMRect | null>(null)
  const field = useRef<HTMLDivElement>(null)
  const button = useRef<HTMLButtonElement>(null)
  const list = useRef<HTMLDivElement>(null)
  const typed = useRef({ text: '', at: 0 })
  const id = useId()
  const chosen = options.findIndex((o) => o.value === value)

  const show = () => {
    setActive(Math.max(0, chosen))
    setOpen(true)
  }
  const pick = (i: number) => {
    const o = options[i]
    setOpen(false)
    button.current?.focus()
    if (o && o.value !== value) onChange(o.value)
  }

  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node
      if (!field.current?.contains(t) && !list.current?.contains(t)) setOpen(false)
    }
    document.addEventListener('pointerdown', onDown)
    return () => document.removeEventListener('pointerdown', onDown)
  }, [open])
  useLayoutEffect(() => {
    if (!open) return
    const place = () => field.current && setRect(field.current.getBoundingClientRect())
    place()
    window.addEventListener('resize', place)
    window.addEventListener('scroll', place, true)
    return () => {
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', place, true)
    }
  }, [open])
  useLayoutEffect(() => {
    if (open) list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [open, active, rect])

  const onKeyDown = (e: ReactKeyboardEvent) => {
    const last = options.length - 1
    const move = (i: number) => (open ? setActive(Math.max(0, Math.min(last, i))) : pick(Math.max(0, Math.min(last, i))))
    let handled = true
    if (e.key === 'ArrowDown') e.altKey && !open ? show() : move((open ? active : chosen) + 1)
    else if (e.key === 'ArrowUp') move((open ? active : chosen) - 1)
    else if (e.key === 'Home') move(0)
    else if (e.key === 'End') move(last)
    else if (e.key === 'Enter' || e.key === ' ') open ? pick(active) : show()
    else if (e.key === 'Escape' && open) setOpen(false)
    else if (e.key === 'Tab' && open) (setOpen(false), (handled = false))
    else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      // Typing jumps to the first option starting with what's been typed.
      const now = Date.now()
      const text = (now - typed.current.at < 700 ? typed.current.text : '') + e.key.toLowerCase()
      typed.current = { text, at: now }
      const from = Math.max(0, open ? active : chosen)
      const order = options.map((_, i) => (from + (text.length === 1 ? 1 : 0) + i) % options.length)
      const hit = order.find((i) => options[i].label.toLowerCase().startsWith(text))
      if (hit !== undefined) move(hit)
    } else handled = false
    if (handled) {
      e.preventDefault()
      e.stopPropagation()
    }
  }

  // Under the field, or over it when that has more room; as wide as the field
  // at least, and never past the window.
  const margin = 8
  const below = rect ? window.innerHeight - rect.bottom - margin : 0
  const above = rect ? rect.top - margin : 0
  const up = !!rect && below < 240 && above > below
  const room = Math.max(120, (up ? above : below) - 6)
  return (
    <div ref={field} className="min-w-0">
      <Squircle radius={9} edge className="bg-raised">
        <button
          ref={button}
          type="button"
          role="combobox"
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-controls={open ? id : undefined}
          aria-activedescendant={open ? `${id}-${active}` : undefined}
          onClick={() => (open ? setOpen(false) : show())}
          onKeyDown={onKeyDown}
          className="flex h-9 w-full items-center gap-2 pr-2.5 pl-3 text-left text-sm text-ink outline-none transition-colors hover:bg-hover"
        >
          <span className="min-w-0 flex-1 truncate">{options[chosen]?.label ?? ''}</span>
          <ChevronDown className={`size-4 shrink-0 text-ink-3 transition-transform duration-150 ${open ? 'rotate-180' : ''}`} />
        </button>
      </Squircle>
      {open &&
        rect &&
        createPortal(
          <div
            className={`absolute z-[60] ${up ? 'origin-bottom -translate-y-full' : 'origin-top'} animate-[pop_120ms_ease-out]`}
            style={{
              left: Math.max(margin, Math.min(rect.left, window.innerWidth - margin - rect.width)) + window.scrollX,
              top: (up ? rect.top - 6 : rect.bottom + 6) + window.scrollY,
              minWidth: rect.width,
              maxWidth: `calc(100vw - ${margin * 2}px)`,
            }}
          >
            <Panel radius={12}>
              <div
                ref={list}
                id={id}
                role="listbox"
                className="overflow-y-auto overscroll-contain p-1"
                style={{ maxHeight: Math.min(room, 320) }}
              >
                {options.map((o, i) => (
                  <Squircle
                    key={o.value}
                    id={`${id}-${i}`}
                    data-index={i}
                    role="option"
                    aria-selected={i === chosen}
                    radius={8}
                    onPointerMove={() => i !== active && setActive(i)}
                    onPointerDown={(e: ReactPointerEvent) => e.preventDefault()}
                    onClick={() => pick(i)}
                    className={`flex cursor-default items-center gap-2.5 py-1.5 pr-3 pl-2 text-[13px] ${i === active ? 'bg-hover text-ink' : 'text-ink-2'}`}
                  >
                    <Check className={`size-3.5 shrink-0 ${i === chosen ? 'text-ink' : 'invisible'}`} />
                    <span className="min-w-0 flex-1 truncate">{o.label}</span>
                  </Squircle>
                ))}
              </div>
            </Panel>
          </div>,
          document.body,
        )}
    </div>
  )
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-8.5 shrink-0 rounded-full transition-colors ${checked ? 'bg-accent' : 'bg-press'}`}
    >
      <span
        className={`absolute top-0.5 size-4 rounded-full transition-all ${checked ? 'left-4 bg-on-accent' : 'left-0.5 bg-ink-2'}`}
      />
    </button>
  )
}

/** A checkbox. The real input stays (visually hidden) so a wrapping <label>, focus, and keyboard all work as usual. */
export function Checkbox({ className = '', ...rest }: Omit<ComponentPropsWithoutRef<'input'>, 'type'>) {
  return (
    <span className={`relative inline-flex size-4 shrink-0 ${className}`}>
      <input type="checkbox" className="peer absolute inset-0 cursor-pointer opacity-0" {...rest} />
      <span className="pointer-events-none flex size-4 items-center justify-center rounded-[5px] bg-press text-on-accent inset-ring inset-ring-glow/8 transition-colors peer-checked:bg-accent peer-hover:bg-float peer-checked:peer-hover:bg-accent-hover peer-focus-visible:ring-2 peer-focus-visible:ring-ink/40 peer-disabled:opacity-40 [&>svg]:scale-50 [&>svg]:opacity-0 [&>svg]:transition-all peer-checked:[&>svg]:scale-100 peer-checked:[&>svg]:opacity-100">
        <Check className="size-3" strokeWidth={3} />
      </span>
    </span>
  )
}

/** Open dialogs, newest last; only the one on top answers Escape. */
const dialogs: symbol[] = []

/** A centered floating panel over a dimmed page. Escape or a click outside closes it. */
export function Dialog({
  children,
  onClose,
  width = 'max-w-lg',
}: {
  children: ReactNode
  onClose: () => void
  width?: string
}) {
  const [id] = useState(() => Symbol())
  useEffect(() => {
    dialogs.push(id)
    return () => void dialogs.splice(dialogs.indexOf(id), 1)
  }, [id])
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && dialogs.at(-1) === id && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [id, onClose])
  // Portaled, so no masked or blurred ancestor can clip it.
  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-shade/50 px-4 py-[8vh] animate-[fade_120ms_ease-out]"
      onPointerDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className={`w-full animate-[pop_140ms_ease-out] ${width}`}>
        <Panel radius={20} className="p-5">
          {children}
        </Panel>
      </div>
    </div>,
    document.body,
  )
}

/** A filter or sort to pick, in a row of them. */
export function Chip({ active, onClick, children }: { active: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <Squircle
      as="button"
      radius={9}
      onClick={onClick}
      aria-pressed={active}
      className={`h-7.5 px-3 text-[13px] transition-colors ${active ? 'bg-press text-ink' : 'text-ink-2 hover:bg-hover hover:text-ink'}`}
    >
      {children}
    </Squircle>
  )
}

/**
 * A row of mutually exclusive choices, the chosen one lifted. The lift slides
 * over to a new choice, its leading edge first, so it stretches on the way.
 */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  size = 'md',
}: {
  value: T
  options: { value: T; label: ReactNode; title?: string }[]
  onChange: (v: T) => void
  size?: 'sm' | 'md'
}) {
  const ref = useRef<HTMLDivElement>(null)
  const index = options.findIndex((o) => o.value === value)
  const [thumb, setThumb] = useState<{ left: number; right: number; index: number; dir: number } | null>(null)
  const corners = useCorners()

  useLayoutEffect(() => {
    const row = ref.current
    if (!row) return
    const place = () => {
      const on = row.children[index + 1] as HTMLElement | undefined
      if (!on) return setThumb(null)
      const left = on.offsetLeft
      const right = row.clientWidth - left - on.offsetWidth
      // Only a new choice moves it; a label changing width just resizes it.
      setThumb((t) => (t && t.left === left && t.right === right && t.index === index ? t : { left, right, index, dir: t && t.index !== index ? Math.sign(index - t.index) : 0 }))
    }
    place()
    const ro = new ResizeObserver(place)
    ro.observe(row)
    return () => ro.disconnect()
  }, [index])

  const lead = '340ms cubic-bezier(0.3, 1.35, 0.5, 1)'
  const trail = '420ms cubic-bezier(0.65, 0, 0.25, 1) 50ms'
  const transition = !thumb?.dir ? 'none' : thumb.dir > 0 ? `left ${trail}, right ${lead}` : `left ${lead}, right ${trail}`
  return (
    <Squircle radius={11} edge className="inline-flex bg-raised p-0.5">
      <div ref={ref} className="relative flex">
        <div
          aria-hidden
          className="absolute inset-y-0 bg-float shadow-(--shadow-chip)"
          style={thumb ? { left: thumb.left, right: thumb.right, borderRadius: Math.round(9 * corners.scale), transition } : { display: 'none' }}
        />
        {options.map((o, i) => (
          <SegmentedOption key={o.value} on={i === index} placed={!!thumb} tip={o.title} size={size} onClick={() => onChange(o.value)}>
            {o.label}
          </SegmentedOption>
        ))}
      </div>
    </Squircle>
  )
}

function SegmentedOption({
  on,
  placed,
  tip: label,
  size,
  onClick,
  children,
}: {
  on: boolean
  placed: boolean
  tip?: string
  size: 'sm' | 'md'
  onClick: () => void
  children: ReactNode
}) {
  const { props, tip } = useTip(label)
  return (
    <Squircle
      as="button"
      radius={9}
      aria-pressed={on}
      onClick={onClick}
      className={`flex items-center gap-1.5 whitespace-nowrap transition-[color,scale] duration-200 active:scale-[0.96] ${size === 'sm' ? 'h-7 px-2.5 text-xs' : 'h-8 px-3 text-[13px]'} ${on ? 'text-ink' : 'text-ink-2 hover:text-ink'} ${on && !placed ? 'bg-float shadow-(--shadow-chip)' : ''}`}
      {...props}
    >
      {children}
      {tip}
    </Squircle>
  )
}

/**
 * What a `Segmented` switches between: a new choice slides in from the side
 * it was picked toward. While `fetching`, the old one stays, faded back.
 */
export function Swap<T extends string>({ value, order, fetching = false, children }: { value: T; order: readonly T[]; fetching?: boolean; children: ReactNode }) {
  const [seen, setSeen] = useState({ value, dir: 0 })
  if (value !== seen.value) setSeen({ value, dir: Math.sign(order.indexOf(value) - order.indexOf(seen.value)) })
  return (
    <div data-fetching={fetching || undefined} className="swap">
      <div key={value} className={seen.dir ? 'swap-in' : undefined} style={{ '--from': `${seen.dir * 32}px` } as CSSProperties}>
        {children}
      </div>
    </div>
  )
}

const tones = {
  quiet: 'bg-panel text-ink-2',
  ok: 'bg-ok/12 text-ok',
  danger: 'bg-danger/12 text-danger',
  live: 'bg-info-deep/12 text-info',
  warn: 'bg-warn-deep/12 text-warn',
  strong: 'bg-ink text-canvas',
}

export function Badge({ tone = 'quiet', children, title }: { tone?: keyof typeof tones; children: ReactNode; title?: string }) {
  return (
    <Tip label={title} className={`inline-flex h-5 shrink-0 items-center gap-1 rounded-md px-1.5 text-2xs font-medium tabular ${tones[tone]}`}>
      {children}
    </Tip>
  )
}

/** A thin bar; `indeterminate` animates when there's no number yet. */
export function Progress({ value, tone = 'ink', className = '' }: { value: number | null; tone?: 'ink' | 'ok' | 'live'; className?: string }) {
  const color = tone === 'ok' ? 'bg-ok' : tone === 'live' ? 'bg-info' : 'bg-ink'
  return (
    <div className={`relative h-1 overflow-hidden rounded-full bg-press ${className}`}>
      {value === null ? (
        <div className={`absolute inset-y-0 w-1/3 animate-[slide_1.2s_ease-in-out_infinite] rounded-full ${color}`} />
      ) : (
        <div className={`h-full rounded-full transition-[width] duration-700 ease-out ${color}`} style={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }} />
      )}
    </div>
  )
}
