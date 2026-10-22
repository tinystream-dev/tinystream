// SPDX-License-Identifier: AGPL-3.0-or-later
// Renders the video's subtitles with rustyass (web/subtitles) into the
// canvas laid over it, redrawing only the frames where something changed.

import init, { Subtitles } from '#wasm/tinystream_subtitles.js'

export type ToWorker =
  | { type: 'init'; canvas: OffscreenCanvas; defaultFont: string; fonts: string[] }
  | { type: 'track'; script: Uint8Array | null; ms: number }
  /** The frame is the video's content box in device pixels; the storage size the video's own. */
  | { type: 'resize'; width: number; height: number; storageWidth: number; storageHeight: number; ms: number }
  /** Draws the frame at `ms`, if it isn't what's drawn already. */
  | { type: 'render'; ms: number }

/** One for every message, in order. */
export type FromWorker = { type: 'done' } | { type: 'error'; message: string }

declare const self: {
  postMessage(m: FromWorker): void
  onmessage: ((e: MessageEvent<ToWorker>) => void) | null
}

let subtitles: Subtitles | null = null
let canvas: OffscreenCanvas | null = null
let context: OffscreenCanvasRenderingContext2D | null = null

async function bytes(url: string): Promise<Uint8Array | null> {
  const response = await fetch(url)
  if (!response.ok) return null
  return new Uint8Array(await response.arrayBuffer())
}

async function start(m: Extract<ToWorker, { type: 'init' }>) {
  const [, font, ...fonts] = await Promise.all([init(), bytes(m.defaultFont), ...m.fonts.map((url) => bytes(url).catch(() => null))])
  if (!font) throw new Error("can't load the default font")
  subtitles = new Subtitles(font)
  for (const f of fonts) if (f) subtitles.addFont(f)
  canvas = m.canvas
  context = canvas.getContext('2d')
}

function draw(ms: number, force: boolean) {
  const frame = subtitles!.render(ms, force)
  if (!frame) return
  context!.clearRect(0, 0, canvas!.width, canvas!.height)
  if (frame.width) context!.putImageData(new ImageData(frame.pixels() as Uint8ClampedArray<ArrayBuffer>, frame.width, frame.height), frame.x, frame.y)
  frame.free()
}

function handle(m: ToWorker) {
  switch (m.type) {
    case 'track':
      if (m.script === null) subtitles!.freeTrack()
      else subtitles!.setTrack(m.script)
      draw(m.ms, true)
      break
    case 'resize':
      canvas!.width = m.width
      canvas!.height = m.height
      subtitles!.resize(m.width, m.height, m.storageWidth, m.storageHeight, 0, 0, 0, 0)
      draw(m.ms, true)
      break
    case 'render':
      draw(m.ms, false)
      break
  }
}

let queue = Promise.resolve()

self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    try {
      if (data.type === 'init') await start(data)
      else handle(data)
      self.postMessage({ type: 'done' })
    } catch (error) {
      self.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  })
}
