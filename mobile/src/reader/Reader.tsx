// SPDX-License-Identifier: AGPL-3.0-or-later
// A book as one long scroll (web/src/books/Reader.tsx). Only a stretch of
// its blocks is there at a time, starting at the one it was left on, and
// grows at either end as it's scrolled to (dropping what's far behind).
// Every block in the stretch stays mounted, so when blocks are added above,
// Android's maintainVisibleContentPosition keeps the text still: with a
// virtualized list it lost its anchor and the text jumped. Where the top of
// the screen is in it is
// kept in characters (packages/shared/src/books.ts), so the web, or another
// phone, opens on the same sentence. Scrolling on hides everything but the
// text, the system's bars too; scrolling back or a tap brings it back.
// Pressing and holding picks a word, and dragging (or the handles) picks
// more, by whole words, to quote.

import { useQueryClient } from '@tanstack/react-query'
import {
  type Block,
  type BookDocument,
  QUOTE_LENGTH,
  type Span,
  chapterAt,
  createProgressSaver,
  fileUrl,
  lengthOf,
  locate,
  percent,
  pieces,
  positionOf,
  styleOf,
  textOf,
  wordAt,
} from '@tinystream/shared/books'
import { useRouter } from 'expo-router'
import { ArrowLeft, Highlighter, List, Minus, Plus } from 'lucide-react-native'
import { type ReactNode, memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { AppState, type LayoutChangeEvent, Linking, Pressable, ScrollView, Text, type TextStyle, View, type ViewStyle } from 'react-native'
import { Gesture, GestureDetector, ScrollView as GestureScrollView } from 'react-native-gesture-handler'
import Animated, { Easing, useAnimatedScrollHandler, useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { setImmersive } from '../../modules/player'
import { type BookDetail, SaveReading, patchReading } from '../books'
import { toast, toastError } from '../components/Feedback'
import { Img } from '../components/Img'
import { Sheet } from '../components/Sheet'
import { IconButton } from '../components/ui'
import { useClipsOn, useMe } from '../queries'
import { CreateQuote, type Mark, useQuotes } from '../quotes'
import { useApi } from '../session'
import { stored } from '../storage'
import { useTheme } from '../theme/ThemeProvider'
import { withAlpha } from '../theme/materials'
import { type Line, offsetAt, pointAt } from './hit'
import { QuoteList, QuoteSheet, SelectionBar, SendSheet, copy, sentTo } from './Quotes'

const SIZES = [15, 16, 17, 18, 20, 22, 24]
const textSize = stored<number>('reader.size', 18)

const SETTLE = Easing.bezier(0.16, 1, 0.3, 1)

const AnimatedScroll = Animated.createAnimatedComponent(GestureScrollView)

/** How many blocks are added at an end at a time, and the most kept at once. */
const STEP = 30
const KEEP = 150

type Tokens = ReturnType<typeof useTheme>['tokens']

/** Something over a run of text: a quote of yours, one sent to you, or what's being selected. */
type Over = { start: number; end: number; kind: 'mine' | 'theirs' | 'selection'; id?: number }

/** Something that can say where it is on screen (a laid out Text). */
type Measurable = {
  getBoundingClientRect?: () => { x: number; y: number; width: number; height: number }
  measureInWindow?: (done: (x: number, y: number, width: number, height: number) => void) => void
}

function rectOf(node: Measurable) {
  if (node.getBoundingClientRect) return node.getBoundingClientRect()
  let rect: { x: number; y: number; width: number; height: number } | null = null
  node.measureInWindow?.((x, y, width, height) => (rect = { x, y, width, height }))
  return rect
}

/** What the paragraphs tell the reader about themselves, and what's pressed in them. */
type Hooks = {
  register: (index: number, node: Measurable | null) => void
  laidOut: (index: number, lines: Line[]) => void
  linesOf: (index: number) => Line[] | undefined
  onQuote: (ids: number[]) => void
  onHandle: (which: 'start' | 'end', phase: 'begin' | 'move' | 'end', x: number, y: number) => void
}

function overStyle(over: Over[], tokens: Tokens, flash: number | null): TextStyle {
  const flashing = flash !== null && over.some((o) => o.id === flash)
  if (over.some((o) => o.kind === 'selection')) return { backgroundColor: withAlpha(tokens.ink, 0.22) }
  if (over.some((o) => o.kind === 'mine')) return { backgroundColor: withAlpha(tokens.highlight, flashing ? 0.55 : 0.28) }
  // Android draws underlines in the text's colour, so the tint is what tells them apart there.
  return {
    textDecorationLine: 'underline',
    textDecorationColor: withAlpha(tokens.highlight, 0.8),
    backgroundColor: withAlpha(tokens.highlight, flashing ? 0.4 : 0.1),
  }
}

function SpanView({ span, tokens, onJump, over, onPress: onOver }: { span: Span; tokens: Tokens; onJump: (block: number) => void; over?: TextStyle; onPress?: () => void }) {
  const s = styleOf(span.f)
  const style: TextStyle = {}
  if (s.bold) style.fontWeight = '600'
  if (s.italic) style.fontStyle = 'italic'
  if (s.underline || span.href || span.to != null) style.textDecorationLine = s.strike ? 'underline line-through' : 'underline'
  else if (s.strike) style.textDecorationLine = 'line-through'
  if (s.code) style.fontFamily = 'monospace'
  if (s.sup || s.sub) style.fontSize = 12
  const onPress = span.href ? () => void Linking.openURL(span.href!) : span.to != null ? () => onJump(span.to!) : onOver
  return (
    <Text style={[style, (span.href || span.to != null) && { textDecorationColor: tokens['ink-3'] }, over]} onPress={onPress}>
      {span.t}
    </Text>
  )
}

/** A selection's ends: a line as tall as the text, and a knob under it to drag. */
function Handle({ which, at, tokens, hooks }: { which: 'start' | 'end'; at: { x: number; y: number; height: number }; tokens: Tokens; hooks: Hooks }) {
  const drag = Gesture.Pan()
    .minDistance(0)
    .hitSlop(18)
    .runOnJS(true)
    .onBegin((e) => hooks.onHandle(which, 'begin', e.absoluteX, e.absoluteY))
    .onUpdate((e) => hooks.onHandle(which, 'move', e.absoluteX, e.absoluteY))
    .onFinalize((e) => hooks.onHandle(which, 'end', e.absoluteX, e.absoluteY))
  const color = tokens.highlight
  return (
    <GestureDetector gesture={drag}>
      <View style={{ position: 'absolute', left: at.x - 9, top: at.y, width: 18, height: at.height + 16, alignItems: 'center' }}>
        <View style={{ width: 2, height: at.height, backgroundColor: color }} />
        <View style={{ width: 14, height: 14, borderRadius: 7, backgroundColor: color }} />
      </View>
    </GestureDetector>
  )
}

/** A paragraph that says where its lines are, with a selection's handles over it. */
function Para({
  index,
  style,
  box,
  handles,
  tokens,
  hooks,
  children,
}: {
  index: number
  style: TextStyle[]
  box?: ViewStyle
  handles?: { start?: number; end?: number }
  tokens: Tokens
  hooks: Hooks
  children: ReactNode
}) {
  const lines = handles && hooks.linesOf(index)
  const start = lines && handles.start !== undefined ? pointAt(lines, handles.start) : null
  const end = lines && handles.end !== undefined ? pointAt(lines, handles.end, true) : null
  return (
    <View style={box}>
      <Text ref={(node) => hooks.register(index, node as Measurable | null)} onTextLayout={(e) => hooks.laidOut(index, e.nativeEvent.lines)} style={style}>
        {children}
      </Text>
      {start && <Handle which="start" at={start} tokens={tokens} hooks={hooks} />}
      {end && <Handle which="end" at={end} tokens={tokens} hooks={hooks} />}
    </View>
  )
}

const BlockView = memo(function BlockView({
  b,
  index,
  size,
  files,
  tokens,
  over,
  handles,
  flash,
  hooks,
  onJump,
}: {
  b: Block
  index: number
  size: number
  files: string
  tokens: Tokens
  over?: Over[]
  handles?: { start?: number; end?: number }
  flash: number | null
  hooks: Hooks
  onJump: (block: number) => void
}) {
  const base: TextStyle = { fontFamily: 'Geist', fontSize: size, lineHeight: Math.round(size * 1.65), color: tokens.ink, textAlign: b.c ? 'center' : 'left' }
  const spans = (over ? pieces(b, over) : (b.s ?? []).map((span) => ({ span, marks: [] as Over[] }))).map((p, i) => {
    if (p.marks.length === 0) return <SpanView key={i} span={p.span} tokens={tokens} onJump={onJump} />
    const ids = p.marks.flatMap((m) => (m.id !== undefined && m.kind !== 'selection' ? [m.id] : []))
    return <SpanView key={i} span={p.span} tokens={tokens} onJump={onJump} over={overStyle(p.marks, tokens, flash)} onPress={ids.length ? () => hooks.onQuote(ids) : undefined} />
  })
  const para = (style: TextStyle[], box?: ViewStyle) => (
    <Para index={index} style={style} box={box} handles={handles} tokens={tokens} hooks={hooks}>
      {spans}
    </Para>
  )
  switch (b.k) {
    case 'h': {
      const scale = [1, 1.5, 1.3, 1.15][b.l ?? 1] ?? 1.05
      return para([base, { fontSize: size * scale, lineHeight: Math.round(size * scale * 1.3), fontWeight: '600' }], { marginTop: size * 1.6, marginBottom: size * 0.8 })
    }
    case 'quote':
      return (
        <View style={{ borderLeftWidth: 2, borderColor: tokens['line-strong'], paddingLeft: size, marginLeft: ((b.l ?? 1) - 1) * size, marginBottom: size * 0.9 }}>
          {para([base, { color: tokens['ink-2'] }])}
        </View>
      )
    case 'pre':
      return para([base, { fontFamily: 'monospace', fontSize: size * 0.82, lineHeight: Math.round(size * 1.35) }], { marginBottom: size * 0.9 })
    case 'li':
      return (
        <View style={{ flexDirection: 'row', paddingLeft: ((b.l ?? 1) - 1) * size * 1.25, marginBottom: size * 0.4 }}>
          <Text style={[base, { color: tokens['ink-3'], width: size * 1.25 }]}>•</Text>
          {para([base], { flex: 1 })}
        </View>
      )
    case 'img':
      return <Picture src={fileUrl(files, b.src ?? '')} size={size} />
    case 'hr':
      return <View style={{ alignSelf: 'center', width: 64, height: 1, backgroundColor: tokens['line-strong'], marginVertical: size * 2 }} />
    default:
      return para([base], { marginBottom: size * 0.9 })
  }
})

function Picture({ src, size }: { src: string; size: number }) {
  const [ratio, setRatio] = useState(2 / 3)
  return (
    <View style={{ marginVertical: size * 1.4, alignItems: 'center' }}>
      <Img
        src={src}
        contentFit="contain"
        onLoad={(e) => e.source.width && e.source.height && setRatio(e.source.width / e.source.height)}
        style={{ width: '100%', aspectRatio: ratio, maxHeight: 640, borderRadius: 6 }}
      />
    </View>
  )
}

/** Each block's place in the scroll, in content coordinates, as it's laid out. */
type Layouts = Map<number, { y: number; h: number }>

/** The blocks there are (from `lo` up to `hi`). */
type Stretch = { lo: number; hi: number }

/** The quotes over each block, by block. */
function overByBlock(doc: BookDocument, marks: Mark[]): Map<number, Over[]> {
  const out = new Map<number, Over[]>()
  for (const m of marks) {
    const over: Over = { start: m.start, end: m.end, kind: m.quote.mine ? 'mine' : 'theirs', id: m.quote.id }
    for (let i = locate(doc, m.start).index; i < doc.blocks.length && doc.blocks[i].at < m.end; i++) out.set(i, [...(out.get(i) ?? []), over])
  }
  return out
}

type Range = { start: number; end: number }

export function Reader({ book, doc, quote }: { book: BookDetail; doc: BookDocument; quote?: number }) {
  const api = useApi()
  const qc = useQueryClient()
  const router = useRouter()
  const insets = useSafeAreaInsets()
  const { tokens } = useTheme()
  const size = textSize.use()
  const scroll = useRef<GestureScrollView>(null)
  const layouts = useRef<Layouts>(new Map())
  const start = useRef(book.reading && !book.reading.finished ? book.reading.position : 0)
  const from = useCallback((index: number): Stretch => ({ lo: index, hi: Math.min(doc.blocks.length, index + STEP) }), [doc])
  const [stretch, setStretch] = useState(() => from(locate(doc, start.current).index))
  const stretchRef = useRef(stretch)
  stretchRef.current = stretch
  // Blocks were just added or dropped, and haven't been laid out yet.
  const growing = useRef(false)
  const stretchTo = useCallback((next: Stretch) => {
    for (const i of layouts.current.keys()) if (i < next.lo || i >= next.hi) layouts.current.delete(i)
    setStretch(next)
  }, [])
  const [position, setPosition] = useState(start.current)
  const [contents, setContents] = useState(false)
  const [quoteList, setQuoteList] = useState(false)
  const [chrome, setChrome] = useState(true)
  const me = useMe()
  const clipsOn = useClipsOn()
  const canQuote = clipsOn && !!me?.permissions.clip
  const marks = useQuotes(book.id)
  const quoted = useMemo(() => overByBlock(doc, marks), [doc, marks])
  const [sel, setSel] = useState<Range | null>(null)
  const [sending, setSending] = useState(false)
  const [busy, setBusy] = useState(false)
  const [opened, setOpened] = useState<number[]>([])
  const [flash, setFlash] = useState<number | null>(null)
  // Where to scroll once the block is laid out: on opening, a jump, or new text size.
  const pending = useRef<{ index: number; fraction: number } | null>(locate(doc, start.current))
  const placed = useRef(false)
  const viewport = useRef({ height: 0, content: 0 })
  // Where on screen the text that counts as where you are is: the top of the text, under the bar.
  const line = insets.top + 64

  const saver = useMemo(
    () =>
      createProgressSaver(async (p) => {
        const r = await api.request(SaveReading, { id: book.id, position: p })
        patchReading(qc, book.id, r.saveReading.reading ?? null)
        void qc.invalidateQueries({ queryKey: ['books', 'home'] })
      }),
    [api, book.id, qc],
  )
  useEffect(() => saver.known(start.current), [saver])

  useEffect(() => {
    const sub = AppState.addEventListener('change', (s) => s !== 'active' && saver.flush())
    return () => {
      sub.remove()
      saver.flush()
    }
  }, [saver])

  useEffect(() => {
    void setImmersive(!chrome)
  }, [chrome])
  useEffect(() => () => void setImmersive(false), [])

  // Scrolls to `index`, `fraction` of the way in, starting the stretch there when it isn't in it.
  // `relayout`: it's about to be laid out again anyway (a new text size), so wait for that.
  const goTo = useCallback(
    (index: number, fraction = 0, relayout = false) => {
      const at = layouts.current.get(index)
      if (at && !relayout) {
        scroll.current?.scrollTo({ y: at.y + fraction * at.h - line, animated: false })
        return
      }
      placed.current = false
      pending.current = { index, fraction }
      if (!at) {
        growing.current = false
        stretchTo(from(index))
      }
    },
    [from, line, stretchTo],
  )

  // The same function all along, so the blocks that link elsewhere needn't render again.
  const goToRef = useRef(goTo)
  goToRef.current = goTo
  const jump = useCallback((block: number) => goToRef.current(block), [])
  const goToPosition = useCallback(
    (p: number) => {
      const { index, fraction } = locate(doc, p)
      goToRef.current(index, fraction)
    },
    [doc],
  )

  // Opened from a quote: go to it, and point it out.
  const shownQuote = useRef(false)
  useEffect(() => {
    const m = marks.find((m) => m.quote.id === quote)
    if (!m || shownQuote.current) return
    shownQuote.current = true
    goToPosition(m.start)
    setFlash(m.quote.id)
    const t = setTimeout(() => setFlash(null), 2200)
    return () => clearTimeout(t)
  }, [marks, quote, goToPosition])

  // The paragraphs on screen, where their lines are, and the position under a point on screen.
  const texts = useRef(new Map<number, { node: Measurable | null; lines: Line[] }>())
  const selRef = useRef(sel)
  selRef.current = sel
  const anchor = useRef<Range | null>(null)
  const pressed = useRef(0)
  const handleRef = useRef<Hooks['onHandle']>(() => {})

  const positionAt = useCallback(
    (x: number, y: number): number | null => {
      let best: { index: number; rect: { x: number; y: number; width: number; height: number }; lines: Line[]; d: number } | null = null
      for (const [index, t] of texts.current) {
        if (!t.node || t.lines.length === 0) continue
        const rect = rectOf(t.node)
        if (!rect || rect.height === 0) continue
        const d = y < rect.y ? rect.y - y : Math.max(0, y - rect.y - rect.height)
        if (!best || d < best.d) best = { index, rect, lines: t.lines, d }
      }
      if (!best || best.d > 80) return null
      const b = doc.blocks[best.index]
      return b.at + Math.min(lengthOf(doc, best.index), offsetAt(best.lines, x - best.rect.x, y - best.rect.y))
    },
    [doc],
  )

  /** Picks from the anchor's word to the word under (x, y). */
  const extendTo = useCallback(
    (x: number, y: number) => {
      const a = anchor.current
      const p = positionAt(x, y)
      if (!a || p === null) return
      const w = wordAt(doc, p) ?? { start: p, end: p }
      const next = { start: Math.min(a.start, w.start), end: Math.max(a.end, w.end) }
      const was = selRef.current
      if (was && was.start === next.start && was.end === next.end) return
      haptic('tick')
      setSel(next)
    },
    [doc, positionAt],
  )

  const pickWord = useCallback(
    (x: number, y: number) => {
      const p = positionAt(x, y)
      const w = p === null ? null : wordAt(doc, p)
      if (!w) return
      anchor.current = w
      haptic('longPressStart')
      setSel(w)
    },
    [doc, positionAt],
  )

  // Dragging a handle picks from the other end's word.
  handleRef.current = (which, phase, x, y) => {
    const s = selRef.current
    if (!s) return
    if (phase === 'begin') anchor.current = which === 'start' ? wordAt(doc, s.end - 1) : wordAt(doc, s.start)
    else if (phase === 'move') extendTo(x, y - size)
  }

  const hooks = useMemo<Hooks>(
    () => ({
      register: (index, node) => {
        const t = texts.current.get(index)
        if (t) t.node = node
        else if (node) texts.current.set(index, { node, lines: [] })
      },
      laidOut: (index, lines) => {
        const t = texts.current.get(index)
        if (t) t.lines = lines
        else texts.current.set(index, { node: null, lines })
      },
      linesOf: (index) => texts.current.get(index)?.lines,
      onQuote: (ids) => {
        pressed.current = Date.now()
        if (!selRef.current) setOpened(ids)
      },
      onHandle: (which, phase, x, y) => handleRef.current(which, phase, x, y),
    }),
    [],
  )

  const quoteIt = async (people: { id: number; username: string }[] = []) => {
    const s = selRef.current
    if (!s) return
    setBusy(true)
    try {
      await api.request(CreateQuote, { input: { bookId: book.id, start: s.start, end: s.end, recipients: people.map((p) => p.id) } })
      await qc.invalidateQueries({ queryKey: ['clips'] })
      haptic('success')
      toast({ title: people.length ? sentTo(people.length, people[0].username) : 'Quoted', tone: 'ok' })
      setSel(null)
    } finally {
      setBusy(false)
    }
  }

  const measure = useCallback(
    (offset: number) => {
      const { height, content } = viewport.current
      let p: number | null = null
      if (stretchRef.current.hi === doc.blocks.length && height > 0 && offset + height >= content - 4 && offset > 0) p = doc.length
      else {
        const y = offset + line
        let best = -1
        for (const [i, l] of layouts.current) if (l.y <= y && y < l.y + l.h && (best < 0 || i < best)) best = i
        if (best >= 0) {
          const l = layouts.current.get(best)!
          p = positionOf(doc, best, l.h > 0 ? (y - l.y) / l.h : 0)
        }
      }
      if (p === null) return
      setPosition(p)
      saver.update(p)
    },
    [doc, line, saver],
  )

  // Scrolling on hides the chrome; scrolling back shows it.
  const shown = useSharedValue(1)
  const last = useSharedValue(0)
  const onChrome = useCallback((on: boolean) => setChrome(on), [])
  const onScrolled = useCallback(
    (offset: number) => {
      if (!placed.current || growing.current) return
      measure(offset)
      // Near an end of what's there: add what comes next, dropping what's furthest away. Added
      // above, it stays where it is on screen.
      const { lo, hi } = stretchRef.current
      const { height, content } = viewport.current
      let next: Stretch | null = null
      if (offset < height * 1.5 && lo > 0) {
        const at = Math.max(0, lo - STEP)
        next = { lo: at, hi: Math.min(hi, at + KEEP) }
      } else if (offset + height * 2.5 > content && hi < doc.blocks.length) {
        const to = Math.min(doc.blocks.length, hi + STEP)
        next = { lo: Math.max(lo, to - KEEP), hi: to }
      }
      if (!next) return
      growing.current = true
      stretchTo(next)
    },
    [doc, measure, stretchTo],
  )
  // Only a finger (or the fling after it) moves the chrome: not a jump, or blocks added or
  // dropped above, which move the offset by far more than a scroll would.
  const touched = useSharedValue(false)
  const onScroll = useAnimatedScrollHandler({
    onBeginDrag: () => {
      touched.value = true
    },
    onEndDrag: () => {
      touched.value = false
    },
    onMomentumBegin: () => {
      touched.value = true
    },
    onMomentumEnd: () => {
      touched.value = false
    },
    onScroll: (e) => {
      const y = e.contentOffset.y
      const dy = y - last.value
      if (!touched.value || Math.abs(dy) > 400) last.value = y
      else if (dy > 8 && shown.value === 1) {
        shown.value = 0
        scheduleOnRN(onChrome, false)
      } else if (dy < -14 && shown.value === 0) {
        shown.value = 1
        scheduleOnRN(onChrome, true)
      }
      if (Math.abs(dy) > 8) last.value = y
      scheduleOnRN(onScrolled, y)
    },
  })

  const toggle = useCallback(() => {
    const on = shown.value === 0
    shown.value = on ? 1 : 0
    setChrome(on)
  }, [shown])
  // A tap on a highlight opens it (its own press), a tap elsewhere puts a selection away, or else the chrome.
  const onTap = useCallback(() => {
    setTimeout(() => {
      if (Date.now() - pressed.current < 400) return
      if (selRef.current) setSel(null)
      else toggle()
    }, 50)
  }, [toggle])
  const tap = Gesture.Tap().maxDistance(10).runOnJS(true).onEnd(onTap)
  const hold = Gesture.Pan()
    .activateAfterLongPress(350)
    .runOnJS(true)
    .onStart((e) => pickWord(e.absoluteX, e.absoluteY))
    .onUpdate((e) => extendTo(e.absoluteX, e.absoluteY))
  const gesture = Gesture.Exclusive(hold, tap)

  const top = useAnimatedStyle(() => ({
    opacity: withTiming(shown.value, { duration: 220, easing: SETTLE }),
    transform: [{ translateY: withTiming(shown.value ? 0 : -24, { duration: 260, easing: SETTLE }) }],
  }))
  const bottom = useAnimatedStyle(() => ({
    opacity: withTiming(shown.value, { duration: 220, easing: SETTLE }),
    transform: [{ translateY: withTiming(shown.value ? 0 : 24, { duration: 260, easing: SETTLE }) }],
  }))

  const onCell = useCallback(
    (index: number, e: LayoutChangeEvent) => {
      const { y, height } = e.nativeEvent.layout
      layouts.current.set(index, { y, h: height })
      const p = pending.current
      if (p && p.index === index) {
        pending.current = null
        const offset = Math.max(0, y + p.fraction * height - line)
        scroll.current?.scrollTo({ y: offset, animated: false })
        // It may not be scrolled again: say where it is now, and add what's above it.
        requestAnimationFrame(() => {
          placed.current = true
          onScrolled(offset)
        })
      }
    },
    [line, onScrolled],
  )

  const renderBlock = useCallback(
    (item: number) => {
      const b = doc.blocks[item]
      let over = quoted.get(item)
      let handles: { start?: number; end?: number } | undefined
      if (sel && b.at < sel.end && b.at + lengthOf(doc, item) > sel.start) {
        over = [...(over ?? []), { start: sel.start, end: sel.end, kind: 'selection' }]
        const first = locate(doc, sel.start).index === item
        const last = locate(doc, sel.end - 1).index === item
        if (first || last) handles = { start: first ? sel.start - b.at : undefined, end: last ? sel.end - b.at : undefined }
      }
      return <BlockView b={b} index={item} size={size} files={book.files} tokens={tokens} over={over} handles={handles} flash={over ? flash : null} hooks={hooks} onJump={jump} />
    },
    [doc, size, book.files, tokens, jump, quoted, sel, flash, hooks],
  )

  // Kept as they are while only the position changes, which is on every scroll.
  const blocks = useMemo(() => {
    const out: ReactNode[] = []
    for (let i = stretch.lo; i < stretch.hi; i++)
      out.push(
        <View key={i} onLayout={(e) => onCell(i, e)}>
          {renderBlock(i)}
        </View>,
      )
    return out
  }, [stretch, renderBlock, onCell])

  const resize = (by: number) => {
    const next = SIZES[Math.max(0, Math.min(SIZES.length - 1, SIZES.indexOf(size) + by))] ?? 18
    if (next === size) return
    const { index, fraction } = locate(doc, position)
    textSize.set(next)
    goTo(index, fraction, true)
  }

  const chapter = chapterAt(doc.chapters, position)
  const pct = percent(position, doc.length)

  return (
    <View className="flex-1 bg-canvas">
      <GestureDetector gesture={gesture}>
        <AnimatedScroll
          ref={scroll}
          onScroll={onScroll}
          scrollEventThrottle={16}
          maintainVisibleContentPosition={{ minIndexForVisible: 0 }}
          onLayout={(e) => (viewport.current.height = e.nativeEvent.layout.height)}
          onContentSizeChange={(_, h) => {
            viewport.current.content = h
            requestAnimationFrame(() => {
              growing.current = false
            })
          }}
          showsVerticalScrollIndicator={false}
          contentContainerStyle={{ paddingTop: line, paddingBottom: insets.bottom + 160, paddingHorizontal: 22 }}
        >
          {blocks}
          {stretch.hi === doc.blocks.length && (
            <Text className="font-sans mt-12 text-center text-sm text-ink-3">{pct >= 100 ? 'The end' : `${pct}% read`}</Text>
          )}
        </AnimatedScroll>
      </GestureDetector>

      <Animated.View
        pointerEvents={chrome ? 'box-none' : 'none'}
        style={[{ position: 'absolute', left: 0, right: 0, top: 0, paddingTop: insets.top, backgroundColor: tokens.canvas, borderBottomWidth: 1, borderColor: tokens.line }, top]}
      >
        <View className="h-14 flex-row items-center gap-1 px-2">
          <IconButton label="Back" onPress={() => router.back()}>
            <ArrowLeft size={22} color={tokens.ink} />
          </IconButton>
          <View className="flex-1 px-1">
            <Text className="font-sans text-[15px] font-semibold text-ink" numberOfLines={1}>
              {book.title}
            </Text>
            <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
              {chapter?.title ?? book.authors.join(', ')}
            </Text>
          </View>
        </View>
      </Animated.View>

      <Animated.View
        pointerEvents={chrome ? 'box-none' : 'none'}
        style={[{ position: 'absolute', left: 0, right: 0, bottom: 0, paddingBottom: insets.bottom, backgroundColor: tokens.canvas, borderTopWidth: 1, borderColor: tokens.line }, bottom]}
      >
        <View className="h-0.5 bg-line">
          <View className="h-full bg-ink-2" style={{ width: `${(position / Math.max(1, doc.length)) * 100}%` }} />
        </View>
        <View className="h-14 flex-row items-center gap-1 px-3">
          <Text className="font-sans flex-1 text-[13px] text-ink-2">{pct}%</Text>
          <IconButton label="Smaller text" onPress={() => resize(-1)} disabled={size === SIZES[0]}>
            <Minus size={20} color={tokens.ink} />
          </IconButton>
          <IconButton label="Larger text" onPress={() => resize(1)} disabled={size === SIZES[SIZES.length - 1]}>
            <Plus size={20} color={tokens.ink} />
          </IconButton>
          {clipsOn && (
            <IconButton label="Quotes" onPress={() => setQuoteList(true)}>
              <Highlighter size={20} color={tokens.ink} />
            </IconButton>
          )}
          <IconButton label="Contents" onPress={() => setContents(true)} disabled={doc.chapters.length === 0}>
            <List size={20} color={tokens.ink} />
          </IconButton>
        </View>
      </Animated.View>

      {sel && (
        <SelectionBar
          long={sel.end - sel.start > QUOTE_LENGTH}
          canQuote={canQuote}
          busy={busy}
          onQuote={() => void quoteIt().catch(toastError)}
          onSend={() => setSending(true)}
          onCopy={() => {
            void copy(textOf(doc, sel.start, sel.end))
            setSel(null)
          }}
          onClose={() => setSel(null)}
        />
      )}
      <SendSheet open={sending} onClose={() => setSending(false)} onSend={(people) => quoteIt(people)} />
      <QuoteSheet quotes={opened.flatMap((id) => marks.find((m) => m.quote.id === id)?.quote ?? [])} onClose={() => setOpened([])} />
      <QuoteList open={quoteList} doc={doc} marks={marks} onJump={goToPosition} onClose={() => setQuoteList(false)} />

      <Sheet open={contents} onClose={() => setContents(false)}>
        <ScrollView style={{ maxHeight: 520 }} contentContainerStyle={{ paddingBottom: 12 }}>
          {doc.chapters.map((c, i) => {
            const here = c === chapter
            return (
              <Pressable
                key={i}
                onPress={() => {
                  setContents(false)
                  jump(c.block)
                }}
                className={`flex-row items-baseline gap-3 rounded-xl py-2.5 pr-3 ${here ? 'bg-press' : ''}`}
                style={{ paddingLeft: 12 + c.depth * 14 }}
              >
                <Text className={`font-sans flex-1 text-[15px] ${here ? 'font-medium text-ink' : 'text-ink-2'}`} numberOfLines={2}>
                  {c.title}
                </Text>
                <Text className="font-sans text-xs text-ink-3">{percent(c.at, doc.length)}%</Text>
              </Pressable>
            )
          })}
        </ScrollView>
      </Sheet>
    </View>
  )
}
