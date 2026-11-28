// SPDX-License-Identifier: AGPL-3.0-or-later
// ASS/SSA rendering with rustyass, in a worker (subtitles.worker.ts) drawing
// over the video, with the file's own embedded fonts.

import defaultFont from '../../../assets/fonts/NotoSans-Regular.ttf?url'
import type { FromWorker, ToWorker } from './subtitles.worker'

export class SubtitleRenderer {
  private worker: Worker | null = null
  private replies: { resolve: () => void; reject: (error: Error) => void }[] = []
  private ready: Promise<void> = Promise.resolve()
  private started = false
  /** The script showing, for a worker that starts over. */
  private script: Uint8Array | null = null
  /** Settles when a worker that broke has been replaced. */
  private restarted: Promise<void> | null = null
  private restartTimer: ReturnType<typeof setTimeout> | undefined
  private failures = 0
  private failedAt = -Infinity
  private observer: ResizeObserver | null = null
  private frameCallback = 0
  private size = ''
  private drawing = false
  private behind = false
  private current: string | null = null
  private requested: string | null = null
  private canvas: HTMLCanvasElement | null = null
  private glint: HTMLCanvasElement | null = null
  private animation: Animation | null = null
  private fetch: AbortController | null = null
  private update: Promise<void> = Promise.resolve()
  private revision = 0
  private destroyed = false

  constructor(
    private video: HTMLVideoElement,
    /** Where this video's endpoints live, e.g. `/api/media/12`. */
    private base: string,
    private fonts: string[],
  ) {}

  private url(track: string) {
    return `${this.base}/subtitles/${track}`
  }

  private request(message: ToWorker, transfer: Transferable[] = []) {
    return new Promise<void>((resolve, reject) => {
      if (!this.worker) return reject(new Error('the subtitles are starting over'))
      this.replies.push({ resolve, reject })
      this.worker.postMessage(message, transfer)
    })
  }

  private start() {
    this.started = true
    this.observer = new ResizeObserver(() => this.resize())
    this.observer.observe(this.video)
    this.video.addEventListener('resize', this.resize)
    this.video.addEventListener('loadeddata', this.rearm)
    this.frameCallback = this.video.requestVideoFrameCallback(this.frame)
    this.spawn()
  }

  /** Starts a worker on a new canvas, showing the script that was showing. */
  private spawn() {
    const canvas = document.createElement('canvas')
    canvas.className = 'subtitles'
    canvas.style.position = 'absolute'
    canvas.style.pointerEvents = 'none'
    canvas.style.visibility = this.script ? 'visible' : 'hidden'
    this.video.insertAdjacentElement('afterend', canvas)
    this.canvas = canvas
    const offscreen = canvas.transferControlToOffscreen()
    const worker = new Worker(new URL('./subtitles.worker.ts', import.meta.url), { type: 'module' })
    this.worker = worker
    worker.onmessage = ({ data }: MessageEvent<FromWorker>) => {
      if (worker !== this.worker) return
      const reply = this.replies.shift()
      if (data.type === 'done') return reply?.resolve()
      const error = new Error(data.message)
      reply?.reject(error)
      if (data.fatal) this.fail(error)
    }
    // When the worker's script can't even load.
    worker.onerror = (e) => {
      if (worker === this.worker) this.fail(new Error(e.message || 'the subtitle worker failed'))
    }
    const absolute = (url: string) => new URL(url, location.href).href
    this.ready = this.request({ type: 'init', canvas: offscreen, defaultFont: absolute(defaultFont), fonts: this.fonts.map(absolute) }, [offscreen])
    this.ready.catch(() => {})
    this.size = ''
    this.resize()
    if (this.script) this.request({ type: 'track', script: this.script, ms: this.ms() }).catch(() => {})
  }

  /** Throws a broken worker away (with what it drew last) and starts another, sooner the first times. */
  private fail(error: Error) {
    if (this.destroyed || !this.worker) return
    console.warn('subtitles', error)
    this.worker.terminate()
    this.worker = null
    for (const reply of this.replies.splice(0)) reply.reject(error)
    this.animation?.cancel()
    this.clearGlint()
    this.canvas?.remove()
    this.canvas = null
    const now = performance.now()
    this.failures = now - this.failedAt > 60_000 ? 0 : this.failures + 1
    this.failedAt = now
    this.restarted = new Promise((resolve) => {
      this.restartTimer = setTimeout(() => {
        this.restarted = null
        if (!this.destroyed) this.spawn()
        resolve()
      }, Math.min(30_000, 250 * 2 ** this.failures))
    })
  }

  /** A new source can drop the frame callback, which would leave the subtitles still. */
  private rearm = () => {
    this.video.cancelVideoFrameCallback(this.frameCallback)
    this.frameCallback = this.video.requestVideoFrameCallback(this.frame)
    void this.draw(this.video.currentTime)
  }

  /** Lays the canvas over the video's content box, and renders at its size in device pixels. */
  private resize = () => {
    const { clientWidth, clientHeight, offsetLeft, offsetTop, videoWidth, videoHeight } = this.video
    if (!this.canvas || !videoWidth || !videoHeight || !clientWidth || !clientHeight) return
    const scale = Math.min(clientWidth / videoWidth, clientHeight / videoHeight)
    const [width, height] = [videoWidth * scale, videoHeight * scale]
    const style = this.canvas.style
    style.width = `${Math.round(width)}px`
    style.height = `${Math.round(height)}px`
    style.left = `${offsetLeft + (clientWidth - width) / 2}px`
    style.top = `${offsetTop + (clientHeight - height) / 2}px`
    const ratio = devicePixelRatio || 1
    const frame = { width: Math.round(width * ratio), height: Math.round(height * ratio), storageWidth: videoWidth, storageHeight: videoHeight }
    const size = Object.values(frame).join()
    if (size === this.size) return
    this.size = size
    this.request({ type: 'resize', ...frame, ms: this.ms() }).catch((e) => {
      if (!this.destroyed) console.warn('subtitles', e)
    })
  }

  private ms(time = this.video.currentTime) {
    return Math.round(time * 1000)
  }

  private frame = (_: number, metadata: VideoFrameCallbackMetadata) => {
    if (this.destroyed) return
    this.frameCallback = this.video.requestVideoFrameCallback(this.frame)
    void this.draw(metadata.mediaTime)
  }

  /** Draws a frame at a time; frames that come in meanwhile are skipped, but for the latest. */
  private async draw(time: number) {
    if (!this.worker) return
    if (this.drawing) {
      this.behind = true
      return
    }
    this.drawing = true
    try {
      await this.request({ type: 'render', ms: this.ms(time) })
    } catch (error) {
      if (!this.destroyed) console.warn('subtitles', error)
    } finally {
      this.drawing = false
    }
    if (this.behind && !this.destroyed) {
      this.behind = false
      void this.draw(this.video.currentTime)
    }
  }

  private shine() {
    const canvas = this.canvas!
    if (!canvas.width || !canvas.height) return
    const glint = document.createElement('canvas')
    glint.width = canvas.width
    glint.height = canvas.height
    const context = glint.getContext('2d')
    if (!context) return
    context.drawImage(canvas, 0, 0)
    glint.className = 'subtitle-glint'
    glint.style.cssText = canvas.style.cssText
    glint.style.filter = 'brightness(2) drop-shadow(0 0 3px rgb(255 255 255 / 0.5))'
    glint.style.maskImage = 'linear-gradient(110deg, transparent 40%, white 50%, transparent 60%)'
    glint.style.maskSize = '300% 100%'
    canvas.insertAdjacentElement('afterend', glint)
    this.glint = glint
    glint.animate([
      { maskPosition: '100% 0', opacity: 0 },
      { maskPosition: '50% 0', opacity: 0.8, offset: 0.5 },
      { maskPosition: '0% 0', opacity: 0 },
    ], { duration: 260, easing: 'ease-out', fill: 'forwards' })
  }

  private clearGlint() {
    this.glint?.remove()
    this.glint = null
  }

  /** Fetches a track's script, trying again a couple of times when the network or the server fails. */
  private async load(track: string, signal: AbortSignal) {
    for (let attempt = 0; ; attempt++) {
      try {
        const response = await fetch(this.url(track), { signal })
        if (!response.ok) throw new Error(`subtitle request failed: ${response.status}`)
        return new Uint8Array(await response.arrayBuffer())
      } catch (error) {
        if (signal.aborted || attempt >= 2) throw error
        await new Promise((resolve) => setTimeout(resolve, 1000 * 2 ** attempt))
      }
    }
  }

  /** Has the worker show a script, waiting out a worker that breaks meanwhile. */
  private async apply(script: Uint8Array | null) {
    for (let attempt = 0; ; attempt++) {
      try {
        await this.restarted
        await this.ready
        await this.request({ type: 'track', script, ms: this.ms() })
        this.script = script
        return
      } catch (error) {
        if (this.destroyed || attempt >= 2 || !this.restarted) throw error
      }
    }
  }

  async show(track: string | null) {
    if (this.destroyed || track === this.requested) return
    this.requested = track
    const revision = ++this.revision
    this.fetch?.abort()
    this.animation?.cancel()
    this.clearGlint()
    if (!track && !this.started) return
    if (!this.started) this.start()
    const controller = new AbortController()
    this.fetch = controller
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const blurred = { filter: 'blur(6px) brightness(1.4)', opacity: 0.55 }
    const clear = { filter: 'blur(0px) brightness(1)', opacity: 1 }
    const fade = !reduced && this.current !== null && this.canvas
      ? this.canvas.animate([clear, blurred], { duration: 160, easing: 'ease-out', fill: 'forwards' })
      : null
    this.animation = fade
    try {
      if (fade) this.shine()
      const [script] = await Promise.all([track ? this.load(track, controller.signal) : null, fade?.finished])
      this.update = this.update.catch(() => {}).then(async () => {
        if (revision !== this.revision) return
        await this.apply(script)
        this.current = track
      })
      await this.update
      if (revision !== this.revision) return
      fade?.cancel()
      if (this.canvas) this.canvas.style.visibility = track === null ? 'hidden' : 'visible'
      this.animation = !reduced && track !== null && this.canvas
        ? this.canvas.animate([
            { ...blurred, filter: 'blur(6px) brightness(2)' },
            { filter: 'blur(2px) brightness(1.6)', opacity: 1, offset: 0.3 },
            clear,
          ], { duration: 480, easing: 'cubic-bezier(0.16, 1, 0.3, 1)' })
        : null
      await this.animation?.finished
    } catch (error) {
      if (revision !== this.revision) return
      this.requested = this.current
      throw error
    } finally {
      if (revision === this.revision) {
        this.animation?.cancel()
        this.animation = null
        this.fetch = null
        this.clearGlint()
      }
    }
  }

  destroy() {
    this.destroyed = true
    ++this.revision
    clearTimeout(this.restartTimer)
    this.fetch?.abort()
    this.animation?.cancel()
    this.clearGlint()
    this.observer?.disconnect()
    this.video.removeEventListener('resize', this.resize)
    this.video.removeEventListener('loadeddata', this.rearm)
    this.video.cancelVideoFrameCallback(this.frameCallback)
    this.worker?.terminate()
    this.worker = null
    for (const reply of this.replies.splice(0)) reply.reject(new Error('the subtitles are gone'))
    this.canvas?.remove()
    this.canvas = null
  }
}
