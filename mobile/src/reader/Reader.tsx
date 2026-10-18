// SPDX-License-Identifier: AGPL-3.0-or-later
// A book as one long scroll (web/src/books/Reader.tsx). Its blocks are a
// list that starts at the one it was left on, with what came before added
// above as it's scrolled back to. Where the top of the screen is in it is
// kept in characters (packages/shared/src/books.ts), so the web, or another
// phone, opens on the same sentence. Scrolling on hides everything but the
// text, the system's bars too; scrolling back or a tap brings it back.

import { useQueryClient } from '@tanstack/react-query'
import {
  type Block,
  type BookDocument,
  type Span,
  chapterAt,
  createProgressSaver,
  fileUrl,
  locate,
  percent,
  positionOf,
  styleOf,
} from '@tinystream/shared/books'
import { useRouter } from 'expo-router'
import { ArrowLeft, List, Minus, Plus } from 'lucide-react-native'
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { AppState, type LayoutChangeEvent, Linking, Pressable, ScrollView, Text, type TextStyle, View, type ViewProps } from 'react-native'
import { FlatList, Gesture, GestureDetector } from 'react-native-gesture-handler'
import Animated, { Easing, useAnimatedScrollHandler, useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { scheduleOnRN } from 'react-native-worklets'
import { setImmersive } from '../../modules/player'
import { type BookDetail, SaveReading, patchReading } from '../books'
import { Img } from '../components/Img'
import { Sheet } from '../components/Sheet'
import { IconButton } from '../components/ui'
import { useApi } from '../session'
import { stored } from '../storage'
import { useTheme } from '../theme/ThemeProvider'
import { withAlpha } from '../theme/materials'

const SIZES = [15, 16, 17, 18, 20, 22, 24]
const textSize = stored<number>('reader.size', 18)

const SETTLE = Easing.bezier(0.16, 1, 0.3, 1)

const AnimatedList = Animated.createAnimatedComponent(FlatList<number>)

/** How many blocks are added above at a time, scrolling back. */
const BEFORE = 40

type Tokens = ReturnType<typeof useTheme>['tokens']

function SpanView({ span, tokens, onJump }: { span: Span; tokens: Tokens; onJump: (block: number) => void }) {
  const s = styleOf(span.f)
  const style: TextStyle = {}
  if (s.bold) style.fontWeight = '600'
  if (s.italic) style.fontStyle = 'italic'
  if (s.underline || span.href || span.to != null) style.textDecorationLine = s.strike ? 'underline line-through' : 'underline'
  else if (s.strike) style.textDecorationLine = 'line-through'
  if (s.code) style.fontFamily = 'monospace'
  if (s.sup || s.sub) style.fontSize = 12
  const onPress = span.href ? () => void Linking.openURL(span.href!) : span.to != null ? () => onJump(span.to!) : undefined
  return (
    <Text style={[style, (span.href || span.to != null) && { textDecorationColor: tokens['ink-3'] }]} onPress={onPress}>
      {span.t}
    </Text>
  )
}

const BlockView = memo(function BlockView({ b, size, files, tokens, onJump }: { b: Block; size: number; files: string; tokens: Tokens; onJump: (block: number) => void }) {
  const base: TextStyle = { fontFamily: 'Geist', fontSize: size, lineHeight: Math.round(size * 1.65), color: tokens.ink, textAlign: b.c ? 'center' : 'left' }
  const selection = withAlpha(tokens.ink, 0.3)
  const spans = b.s?.map((s, i) => <SpanView key={i} span={s} tokens={tokens} onJump={onJump} />)
  switch (b.k) {
    case 'h': {
      const scale = [1, 1.5, 1.3, 1.15][b.l ?? 1] ?? 1.05
      return (
        <Text selectable selectionColor={selection} style={[base, { fontSize: size * scale, lineHeight: Math.round(size * scale * 1.3), fontWeight: '600', marginTop: size * 1.6, marginBottom: size * 0.8 }]}>
          {spans}
        </Text>
      )
    }
    case 'quote':
      return (
        <View style={{ borderLeftWidth: 2, borderColor: tokens['line-strong'], paddingLeft: size, marginLeft: ((b.l ?? 1) - 1) * size, marginBottom: size * 0.9 }}>
          <Text selectable selectionColor={selection} style={[base, { color: tokens['ink-2'] }]}>{spans}</Text>
        </View>
      )
    case 'pre':
      return <Text selectable selectionColor={selection} style={[base, { fontFamily: 'monospace', fontSize: size * 0.82, lineHeight: Math.round(size * 1.35), marginBottom: size * 0.9 }]}>{spans}</Text>
    case 'li':
      return (
        <View style={{ flexDirection: 'row', paddingLeft: ((b.l ?? 1) - 1) * size * 1.25, marginBottom: size * 0.4 }}>
          <Text style={[base, { color: tokens['ink-3'], width: size * 1.25 }]}>•</Text>
          <Text selectable selectionColor={selection} style={[base, { flex: 1 }]}>{spans}</Text>
        </View>
      )
    case 'img':
      return <Picture src={fileUrl(files, b.src ?? '')} size={size} />
    case 'hr':
      return <View style={{ alignSelf: 'center', width: 64, height: 1, backgroundColor: tokens['line-strong'], marginVertical: size * 2 }} />
    default:
      return <Text selectable selectionColor={selection} style={[base, { marginBottom: size * 0.9 }]}>{spans}</Text>
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

/** Each block's place in the list, in content coordinates, as the list lays them out. */
type Layouts = Map<number, { y: number; h: number }>

export function Reader({ book, doc }: { book: BookDetail; doc: BookDocument }) {
  const api = useApi()
  const qc = useQueryClient()
  const router = useRouter()
  const insets = useSafeAreaInsets()
  const { tokens } = useTheme()
  const size = textSize.use()
  const list = useRef<FlatList<number>>(null)
  const layouts = useRef<Layouts>(new Map())
  const start = useRef(book.reading && !book.reading.finished ? book.reading.position : 0)
  const [first, setFirst] = useState(() => locate(doc, start.current).index)
  const [position, setPosition] = useState(start.current)
  const [contents, setContents] = useState(false)
  const [chrome, setChrome] = useState(true)
  // Where to scroll once the block is laid out: on opening, a jump, or new text size.
  const pending = useRef<{ index: number; fraction: number } | null>(locate(doc, start.current))
  const placed = useRef(false)
  const viewport = useRef({ height: 0, content: 0 })
  const line = insets.top + 12

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

  const data = useMemo(() => Array.from({ length: doc.blocks.length - first }, (_, k) => first + k), [doc, first])

  // Starts the list at `index` (or scrolls to it, when it already does), `fraction` of the way in.
  // `relayout`: it's about to be laid out again anyway (a new text size), so wait for that.
  const goTo = useCallback(
    (index: number, fraction = 0, relayout = false) => {
      const at = layouts.current.get(index)
      if (index === first && at && !relayout) {
        list.current?.scrollToOffset({ offset: at.y + fraction * at.h - line, animated: false })
        return
      }
      pending.current = { index, fraction }
      if (index !== first) {
        layouts.current.clear()
        setFirst(index)
      }
    },
    [first, line],
  )

  // The same function all along, so the blocks that link elsewhere needn't render again.
  const goToRef = useRef(goTo)
  goToRef.current = goTo
  const jump = useCallback((block: number) => goToRef.current(block), [])

  const measure = useCallback(
    (offset: number) => {
      if (!placed.current) return
      const { height, content } = viewport.current
      let p: number | null = null
      if (height > 0 && offset + height >= content - 4 && offset > 0) p = doc.length
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
      measure(offset)
      // Near the top of what's there: add what came before (it stays where it is on screen).
      if (placed.current && offset < 800 && first > 0) {
        layouts.current.clear()
        setFirst((f) => Math.max(0, f - BEFORE))
      }
    },
    [measure, first],
  )
  const onScroll = useAnimatedScrollHandler((e) => {
    const y = e.contentOffset.y
    const dy = y - last.value
    if (dy > 8 && shown.value === 1) {
      shown.value = 0
      scheduleOnRN(onChrome, false)
    } else if (dy < -14 && shown.value === 0) {
      shown.value = 1
      scheduleOnRN(onChrome, true)
    }
    if (Math.abs(dy) > 8) last.value = y
    scheduleOnRN(onScrolled, y)
  })

  const toggle = useCallback(() => {
    const on = shown.value === 0
    shown.value = on ? 1 : 0
    setChrome(on)
  }, [shown])
  const tap = Gesture.Tap().maxDistance(10).runOnJS(true).onEnd(toggle)

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
        list.current?.scrollToOffset({ offset: y + p.fraction * height - line, animated: false })
        requestAnimationFrame(() => {
          placed.current = true
        })
      }
    },
    [line],
  )

  const Cell = useMemo(
    () =>
      function Cell({ item, onLayout, ...rest }: ViewProps & { item: number; index: number; cellKey: string }) {
        const { index: _, cellKey: __, ...props } = rest
        return (
          <View
            {...props}
            onLayout={(e) => {
              onLayout?.(e)
              onCell(item, e)
            }}
          />
        )
      },
    [onCell],
  )

  const renderItem = useCallback(
    ({ item }: { item: number }) => <BlockView b={doc.blocks[item]} size={size} files={book.files} tokens={tokens} onJump={jump} />,
    [doc, size, book.files, tokens, jump],
  )

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
      <GestureDetector gesture={tap}>
        <AnimatedList
          ref={list}
          data={data}
          keyExtractor={(i) => String(i)}
          renderItem={renderItem}
          CellRendererComponent={Cell}
          onScroll={onScroll}
          scrollEventThrottle={16}
          maintainVisibleContentPosition={{ minIndexForVisible: 0 }}
          onLayout={(e) => (viewport.current.height = e.nativeEvent.layout.height)}
          onContentSizeChange={(_, h) => (viewport.current.content = h)}
          initialNumToRender={12}
          maxToRenderPerBatch={12}
          windowSize={9}
          showsVerticalScrollIndicator={false}
          contentContainerStyle={{ paddingTop: insets.top + 64, paddingBottom: insets.bottom + 160, paddingHorizontal: 22 }}
          ListFooterComponent={
            <Text className="font-sans mt-12 text-center text-sm text-ink-3">{pct >= 100 ? 'The end' : `${pct}% read`}</Text>
          }
        />
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
          <IconButton label="Contents" onPress={() => setContents(true)} disabled={doc.chapters.length === 0}>
            <List size={20} color={tokens.ink} />
          </IconButton>
        </View>
      </Animated.View>

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
