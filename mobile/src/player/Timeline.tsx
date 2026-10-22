// SPDX-License-Identifier: AGPL-3.0-or-later
// The seek bar (web's Timeline): chapters as segments, what's buffered, and
// while scrubbing, a frame from around there with the time above it.

import { clock } from '@tinystream/shared/format'
import { useEffect, useState } from 'react'
import { StyleSheet, Text, View } from 'react-native'
import { Gesture, GestureDetector } from 'react-native-gesture-handler'
import { Img } from '../components/Img'
import { Squircle } from '../effects/Squircle'
import type { Playback } from './plan'

type Chapter = Playback['media']['chapters'][number]

const PREVIEW_STEP = 10
const PREVIEW_WIDTH = 176
/** Between chapters' segments. */
const GAP = 3

export function Timeline({
  base,
  time,
  duration,
  buffered,
  chapters,
  scrubbing,
  onScrub,
  onSeek,
}: {
  /** Where this video's endpoints live; frames come from there. */
  base: string
  time: number
  duration: number
  buffered: [number, number] | null
  chapters: Chapter[]
  /** Where a scrub (here, or across the screen) is; null when there's none. */
  scrubbing: number | null
  onScrub: (at: number | null) => void
  onSeek: (at: number) => void
}) {
  const [width, setWidth] = useState(0)
  const at = (x: number) => (width > 0 ? Math.max(0, Math.min(1, x / width)) * duration : 0)
  const shown = scrubbing ?? time
  const pct = (t: number) => (duration > 0 ? Math.max(0, Math.min(1, t / duration)) * width : 0)

  const drag = Gesture.Pan()
    .runOnJS(true)
    .activeOffsetX([-4, 4])
    .onStart((e) => onScrub(at(e.x)))
    .onUpdate((e) => onScrub(at(e.x)))
    .onEnd((e) => onSeek(at(e.x)))
    .onFinalize(() => onScrub(null))
  const tap = Gesture.Tap()
    .runOnJS(true)
    .onEnd((e, ok) => ok && onSeek(at(e.x)))

  // Chapters become segments with small gaps; without chapters, one segment.
  const segments = chapters.length > 1 ? chapters.map((c) => [c.start, c.end] as const) : [[0, duration] as const]
  const chapter = scrubbing !== null ? chapters.find((c) => scrubbing >= c.start && scrubbing < c.end) : undefined

  return (
    <GestureDetector gesture={Gesture.Race(drag, tap)}>
      <View
        hitSlop={{ top: 14, bottom: 14 }}
        onLayout={(e) => setWidth(e.nativeEvent.layout.width)}
        accessibilityRole="adjustable"
        accessibilityLabel="Seek"
        accessibilityValue={{ min: 0, max: Math.round(duration), now: Math.round(time), text: `${clock(time)} of ${clock(duration)}` }}
        style={{ height: 24, justifyContent: 'center' }}
      >
        {width > 0 &&
          segments.map(([s, e], i) => {
            const left = pct(s)
            const w = Math.max(0, pct(e) - left - (i < segments.length - 1 ? GAP : 0))
            // What's buffered within this segment, in its own coordinates.
            const from = buffered ? pct(Math.max(buffered[0], s)) - left : 0
            const to = buffered ? Math.min(w, pct(Math.min(buffered[1], e)) - left) : 0
            return (
              <View
                key={i}
                className="absolute overflow-hidden rounded-full bg-media-ink/20"
                style={{ left, width: w, height: scrubbing !== null ? 6 : 4 }}
              >
                {to > from && <View className="absolute bottom-0 top-0 bg-media-ink/25" style={{ left: from, width: to - from }} />}
                <View className="absolute bottom-0 left-0 top-0 bg-media-ink" style={{ width: Math.max(0, Math.min(w, pct(shown) - left)) }} />
              </View>
            )
          })}
        {width > 0 && (
          <View
            pointerEvents="none"
            className="absolute rounded-full bg-media-ink"
            style={{
              left: pct(shown) - (scrubbing !== null ? 9 : 6),
              width: scrubbing !== null ? 18 : 12,
              height: scrubbing !== null ? 18 : 12,
              boxShadow: '0px 0px 0px 4px rgba(255, 255, 255, 0.15)',
            }}
          />
        )}
        {scrubbing !== null && width > 0 && (
          <View
            pointerEvents="none"
            style={{ position: 'absolute', bottom: 26, alignItems: 'center', width: PREVIEW_WIDTH, left: Math.max(0, Math.min(width - PREVIEW_WIDTH, pct(scrubbing) - PREVIEW_WIDTH / 2)) }}
          >
            <Preview base={base} at={scrubbing} />
            <View className="mt-1.5 flex-row rounded-lg bg-media-panel/90 px-2 py-1">
              <Text className="font-sans text-xs text-media-ink" style={{ fontVariant: ['tabular-nums'] }}>
                {clock(scrubbing)}
              </Text>
              {chapter?.title && (
                <Text className="font-sans ml-1.5 text-xs text-media-ink/55" numberOfLines={1}>
                  {chapter.title}
                </Text>
              )}
            </View>
          </View>
        )}
      </View>
    </GestureDetector>
  )
}

/**
 * A frame from around `at`. Frames come in 10 s steps so each is made once;
 * while the next one loads, the last one stays up.
 */
function Preview({ base, at }: { base: string; at: number }) {
  const step = Math.floor(at / PREVIEW_STEP) * PREVIEW_STEP
  const [wanted, setWanted] = useState(step)
  const [shown, setShown] = useState<number | null>(null)
  const [failed, setFailed] = useState(false)
  // Wait for the finger to settle a little before asking the server.
  useEffect(() => {
    const t = setTimeout(() => setWanted(step), 60)
    return () => clearTimeout(t)
  }, [step])
  if (failed) return null
  return (
    <Squircle radius={10} edge className="bg-media-panel/90" style={{ width: PREVIEW_WIDTH, aspectRatio: 16 / 9 }}>
      {shown !== null && <Img src={`${base}/preview/${shown}`} transition={0} contentFit="cover" style={StyleSheet.absoluteFill} />}
      {wanted !== shown && (
        <Img
          key={wanted}
          src={`${base}/preview/${wanted}`}
          transition={0}
          contentFit="cover"
          style={StyleSheet.absoluteFill}
          onLoad={() => setShown(wanted)}
          onError={() => shown === null && setFailed(true)}
        />
      )}
    </Squircle>
  )
}
