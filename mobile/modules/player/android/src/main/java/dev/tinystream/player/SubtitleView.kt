// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Rect
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.util.Log
import android.view.Choreographer
import android.view.View
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.IOException
import java.util.concurrent.Executors

private const val TAG = "TinystreamSubtitles"

/** As web's subtitles.ts: the old track fades out quickly, the new one in more slowly. */
private const val FADE_OUT_MS = 160L
private const val FADE_IN_MS = 300L

private const val DEFAULT_FONT = "subtitles/LiberationSans-Regular.ttf"

private val loader = Executors.newSingleThreadExecutor { r -> Thread(r, "tinystream-subtitles-load").apply { isDaemon = true } }

/** A subtitle track: its ASS file and the video's fonts, and the headers that fetch them. */
data class SubtitleSource(val url: String, val fonts: List<String>, val headers: Map<String, String>)

/** What rustyass draws into (see Ass.resize). */
private data class Frame(val width: Int, val height: Int, val storageWidth: Int, val storageHeight: Int, val picture: Rect)

/**
 * A video's subtitles, drawn by rustyass over the picture: on a thread of its
 * own, every frame the time has moved (as `clock` tells it) and only when
 * what shows has changed, into a bitmap covering just that.
 */
class SubtitleView(context: Context, private val client: OkHttpClient, private val clock: () -> Long) : View(context) {
  private val thread = HandlerThread("tinystream-subtitles").apply { start() }
  private val worker = Handler(thread.looper)
  private val main = Handler(Looper.getMainLooper())

  // The worker's: the open track, the frame it was laid out for, and the bitmaps it paints into, in turn.
  private var handle = 0L
  private var applied: Frame? = null
  private val buffers = arrayOfNulls<Bitmap>(2)
  private var back = 0

  // The main thread's.
  private var source: SubtitleSource? = null
  private var revision = 0
  /** Which track is open, so frames of the one before are dropped. */
  private var track = 0
  private var frame: Frame? = null
  private var active = false
  private var busy = false
  private var dirty = false
  private var lastMs = -1L
  private var shown: Bitmap? = null
  private var shownX = 0f
  private var shownY = 0f
  private var released = false

  init {
    setWillNotDraw(false)
    alpha = 0f
  }

  /** Shows `next` (null hides them); `done` gets what went wrong, if anything did. */
  fun select(next: SubtitleSource?, done: (String?) -> Unit) {
    if (next == source || released) return done(null)
    source = next
    val rev = ++revision
    if (next == null) {
      animate().cancel()
      animate().alpha(0f).setDuration(FADE_OUT_MS).withEndAction {
        if (rev != revision) return@withEndAction
        stop()
        worker.post { close() }
      }
      return done(null)
    }
    loader.execute {
      val loaded = runCatching {
        val script = fetch(next.url, next.headers)
        // A font that won't come is only a font less, as on web.
        val fonts = next.fonts.mapNotNull { url -> runCatching { fetch(url, next.headers) }.onFailure { Log.w(TAG, "font $url", it) }.getOrNull() }
        script to fonts
      }
      main.post {
        if (rev != revision || released) return@post done(null)
        val (script, fonts) = loaded.getOrElse {
          fail()
          return@post done(it.message ?: "couldn't load the subtitles")
        }
        worker.post {
          close()
          handle = Ass.open(script, fonts.toTypedArray(), defaultFont(context))
          val opened = handle != 0L
          main.post {
            if (rev != revision || released) return@post done(null)
            if (!opened) {
              fail()
              return@post done("these subtitles can't be read")
            }
            track++
            animate().cancel()
            alpha = 0f
            shown = null
            invalidate()
            start()
            animate().alpha(1f).setDuration(FADE_IN_MS)
            done(null)
          }
        }
      }
    }
  }

  /** Lays the subtitles out in a `width`×`height` view, around the picture at `picture` of a video `storageWidth`×`storageHeight`. */
  fun place(width: Int, height: Int, picture: Rect, storageWidth: Int, storageHeight: Int) {
    if (width <= 0 || height <= 0) return
    val sw = if (storageWidth > 0) storageWidth else picture.width()
    val sh = if (storageHeight > 0) storageHeight else picture.height()
    val next = Frame(width, height, sw, sh, Rect(picture))
    if (next == frame) return
    frame = next
    dirty = true
  }

  fun release() {
    released = true
    revision++
    stop()
    animate().cancel()
    main.removeCallbacksAndMessages(null)
    worker.post {
      close()
      thread.quitSafely()
    }
  }

  override fun onDraw(canvas: Canvas) {
    shown?.let { canvas.drawBitmap(it, shownX, shownY, null) }
  }

  override fun onAttachedToWindow() {
    super.onAttachedToWindow()
    if (active) Choreographer.getInstance().postFrameCallback(tick)
  }

  override fun onDetachedFromWindow() {
    super.onDetachedFromWindow()
    Choreographer.getInstance().removeFrameCallback(tick)
  }

  private val tick = object : Choreographer.FrameCallback {
    override fun doFrame(frameTimeNanos: Long) {
      if (!active) return
      step()
      Choreographer.getInstance().postFrameCallback(this)
    }
  }

  private fun start() {
    active = true
    dirty = true
    Choreographer.getInstance().removeFrameCallback(tick)
    if (isAttachedToWindow) Choreographer.getInstance().postFrameCallback(tick)
  }

  /** What was showing goes too: it isn't what was asked for anymore. */
  private fun fail() {
    source = null
    stop()
    worker.post { close() }
  }

  private fun stop() {
    track++
    active = false
    Choreographer.getInstance().removeFrameCallback(tick)
    shown = null
    invalidate()
  }

  /** Asks the worker for the frame at the time now, unless it's still on the last one or nothing moved. */
  private fun step() {
    val f = frame ?: return
    if (busy) return
    val ms = clock()
    if (ms == lastMs && !dirty) return
    val force = dirty
    dirty = false
    lastMs = ms
    busy = true
    val open = track
    worker.post {
      val drawn = render(f, ms, force)
      main.post {
        busy = false
        if (drawn == null || open != track || !active) return@post
        shown = drawn.first
        shownX = drawn.second.left.toFloat()
        shownY = drawn.second.top.toFloat()
        invalidate()
      }
    }
  }

  /** On the worker: what shows at `ms` and where, or null when that hasn't changed. */
  private fun render(f: Frame, ms: Long, force: Boolean): Pair<Bitmap?, Rect>? {
    if (handle == 0L) return null
    val resized = f != applied
    if (resized) {
      val p = f.picture
      Ass.resize(handle, f.width, f.height, f.storageWidth, f.storageHeight, p.top, f.height - p.bottom, p.left, f.width - p.right)
      applied = f
    }
    val (x, y, w, h) = Ass.render(handle, ms, force || resized) ?: return null
    val bitmap = if (w > 0 && h > 0) buffer(w, h).takeIf { Ass.paint(handle, it) } else null
    return bitmap to Rect(x, y, x + w, y + h)
  }

  /** The bitmap not on screen, at `w`×`h`; the next one after it is the other. */
  private fun buffer(w: Int, h: Int): Bitmap {
    val old = buffers[back]
    val bitmap = if (old != null && old.allocationByteCount >= w * h * 4) {
      if (old.width != w || old.height != h) old.reconfigure(w, h, Bitmap.Config.ARGB_8888)
      old
    } else {
      Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888).also { buffers[back] = it }
    }
    back = 1 - back
    return bitmap
  }

  private fun close() {
    if (handle != 0L) Ass.close(handle)
    handle = 0L
    applied = null
  }

  private fun fetch(url: String, headers: Map<String, String>): ByteArray {
    val request = Request.Builder().url(url).apply { headers.forEach { (k, v) -> header(k, v) } }.build()
    client.newCall(request).execute().use { response ->
      if (!response.isSuccessful) throw IOException("the server answered ${response.code}")
      return response.body?.bytes() ?: throw IOException("the server sent nothing")
    }
  }

  private companion object {
    @Volatile private var font: ByteArray? = null

    fun defaultFont(context: Context): ByteArray = font ?: context.assets.open(DEFAULT_FONT).use { it.readBytes() }.also { font = it }
  }
}
