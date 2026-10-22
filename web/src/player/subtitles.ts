// SPDX-License-Identifier: AGPL-3.0-or-later
// ASS/SSA rendering with rustyass, in a worker (subtitles.worker.ts) drawing
// over the video, with the file's own embedded fonts.

import defaultFont from '../../../assets/fonts/NotoSans-Regular.ttf?url'
import type { FromWorker, ToWorker } from './subtitles.worker'

export class SubtitleRenderer {
  private worker: Worker | null = null
  private replies: { resolve: () => void; reject: (error: Error) => void }[] = []
  private ready: Promise<void> = Promise.resolve()
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
      this.replies.push({ resolve, reject })
      this.worker!.postMessage(message, transfer)
    })
  }

  private start(canvas: HTMLCanvasElement) {
    const offscreen = canvas.transferControlToOffscreen()
    this.worker = new Worker(new URL('./subtitles.worker.ts', import.meta.url), { type: 'module' })
    this.worker.onmessage = ({ data }: MessageEvent<FromWorker>) => {
      const reply = this.replies.shift()
      if (data.type === 'error') reply?.reject(new Error(data.message))
      else reply?.resolve()
    }
    const absolute = (url: string) => new URL(url, location.href).href
    this.ready = this.request({ type: 'init', canvas: offscreen, defaultFont: absolute(defaultFont), fonts: this.fonts.map(absolute) }, [offscreen])
    this.observer = new ResizeObserver(() => this.resize())
    this.observer.observe(this.video)
    this.video.addEventListener('resize', this.resize)
    this.frameCallback = this.video.requestVideoFrameCallback(this.frame)
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

  async show(track: string | null) {
    if (this.destroyed || track === this.requested) return
    this.requested = track
    const revision = ++this.revision
    this.fetch?.abort()
    this.animation?.cancel()
    this.clearGlint()
    if (!track && !this.worker) return
    if (!this.worker) {
      this.canvas = document.createElement('canvas')
      this.canvas.className = 'subtitles'
      this.canvas.style.position = 'absolute'
      this.canvas.style.pointerEvents = 'none'
      this.canvas.style.visibility = 'hidden'
      this.video.insertAdjacentElement('afterend', this.canvas)
      this.start(this.canvas)
      this.resize()
    }
    const controller = new AbortController()
    this.fetch = controller
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const blurred = { filter: 'blur(6px) brightness(1.4)', opacity: 0.55 }
    const clear = { filter: 'blur(0px) brightness(1)', opacity: 1 }
    const fade = !reduced && this.current !== null
      ? this.canvas!.animate([clear, blurred], { duration: 160, easing: 'ease-out', fill: 'forwards' })
      : null
    this.animation = fade
    try {
      if (fade) this.shine()
      const content = track
        ? fetch(this.url(track), { signal: controller.signal }).then(async (response) => {
            if (!response.ok) throw new Error(`subtitle request failed: ${response.status}`)
            return new Uint8Array(await response.arrayBuffer())
          })
        : Promise.resolve(null)
      const [script] = await Promise.all([content, this.ready, fade?.finished])
      this.update = this.update.catch(() => {}).then(async () => {
        if (revision !== this.revision) return
        await this.request({ type: 'track', script, ms: this.ms() }, script ? [script.buffer] : [])
        if (revision !== this.revision) return
        this.current = track
      })
      await this.update
      if (revision !== this.revision) return
      fade?.cancel()
      this.canvas!.style.visibility = track === null ? 'hidden' : 'visible'
      this.animation = !reduced && track !== null
        ? this.canvas!.animate([
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
    this.fetch?.abort()
    this.animation?.cancel()
    this.clearGlint()
    this.observer?.disconnect()
    this.video.removeEventListener('resize', this.resize)
    this.video.cancelVideoFrameCallback(this.frameCallback)
    this.worker?.terminate()
    this.worker = null
    for (const reply of this.replies.splice(0)) reply.reject(new Error('the subtitles are gone'))
    this.canvas?.remove()
    this.canvas = null
  }
}
