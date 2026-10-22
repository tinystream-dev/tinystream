// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import android.net.Uri
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.BaseDataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.ProgressiveMediaSource
import androidx.media3.exoplayer.upstream.DefaultLoadErrorHandlingPolicy
import androidx.media3.exoplayer.upstream.LoadErrorHandlingPolicy
import androidx.media3.extractor.Extractor
import androidx.media3.extractor.ExtractorOutput
import androidx.media3.extractor.ExtractorsFactory
import androidx.media3.extractor.SeekMap
import androidx.media3.extractor.SeekPoint
import androidx.media3.extractor.mp4.FragmentedMp4Extractor
import androidx.media3.extractor.text.SubtitleParser
import okhttp3.Call
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import org.json.JSONObject
import java.io.IOException
import java.io.InterruptedIOException
import java.util.Locale
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit

/** How a stream is made: the query `plan()` picks, minus where it starts. */
data class StreamPlan(val video: String, val height: Int, val audio: Int?, val audioMode: String)

/** The server turned the stream down; its message says why, and asking again won't help. */
class ServerError(val status: Int, message: String) : IOException(message)

/** A position's high bits say which session (stream) it's in; the rest, how far into it. */
private const val SESSION_SHIFT = 42
private const val OFFSET_MASK = (1L shl SESSION_SHIFT) - 1

/**
 * A stream's first bytes are kept for a moment after it's closed, so that
 * asking for the same start again right away replays them instead of asking
 * the server twice: ExoPlayer does exactly that when it starts somewhere
 * other than 0 (it reads enough to prepare, then seeks to where it already is).
 */
private const val REPLAY_BYTES = 4 shl 20
private const val PARK_MS = 3_000L

/** What a stream's pump reads ahead of ExoPlayer: up to 16 chunks of 64 KB. */
private const val PUMP_CHUNK = 64 shl 10
private const val PUMP_CHUNKS = 16
private val END = ByteArray(0)

private val parking = Executors.newSingleThreadScheduledExecutor { r -> Thread(r, "tinystream-stream-park").apply { isDaemon = true } }

/**
 * tinystream's stream as one seekable piece of media. Each request is a new
 * fragmented MP4 from `?start=`, with the file's own timestamps, so whatever
 * starts where, samples land at their place in the file. ExoPlayer seeks
 * within what it has buffered by itself; for anywhere else it asks the seek
 * map for a byte position, and that position names a new session (request)
 * at that time.
 */
@UnstableApi
class StreamSource(
  private val client: OkHttpClient,
  /** Where the video's endpoints live, e.g. `https://host/api/media/12`. */
  private val base: String,
  private val headers: Map<String, String>,
  private val plan: StreamPlan,
  durationSec: Double,
) {
  private val durationUs = if (durationSec > 0) (durationSec * 1_000_000).toLong() else C.TIME_UNSET

  /** Where session 0, the one a (re)prepare reads first, starts. */
  @Volatile var startUs = 0L

  /** Where each later session starts; session n is at index n - 1. */
  private val starts = ArrayList<Long>()
  private var parked: Connection? = null
  private var expiry: ScheduledFuture<*>? = null

  /** Where the stream being read started (null before the first). */
  @Volatile var loadingFromUs: Long? = null
    private set

  fun mediaSource(item: MediaItem.Builder): MediaSource =
    ProgressiveMediaSource.Factory({ StreamDataSource() }, ExtractorsFactory { arrayOf(StreamExtractor()) })
      .setLoadErrorHandlingPolicy(NoRetries)
      .createMediaSource(item.setUri(url(startUs)).build())

  fun url(atUs: Long): String {
    val q = StringBuilder("start=").append(String.format(Locale.ROOT, "%.3f", atUs / 1_000_000.0))
    q.append("&video=").append(plan.video).append("&audioMode=").append(plan.audioMode)
    if (plan.video == "transcode") q.append("&height=").append(plan.height)
    plan.audio?.let { q.append("&audio=").append(it) }
    return "$base/stream?$q"
  }

  @Synchronized
  private fun positionOf(timeUs: Long): Long {
    var i = starts.lastIndexOf(timeUs)
    if (i < 0) {
      starts.add(timeUs)
      i = starts.size - 1
    }
    return (i + 1).toLong() shl SESSION_SHIFT
  }

  @Synchronized
  private fun startOf(position: Long): Long {
    val session = (position ushr SESSION_SHIFT).toInt()
    return if (session == 0) startUs else starts[session - 1]
  }

  /** The parked stream, if it starts at `atUs` and nothing of it is lost; the rest is closed. */
  @Synchronized
  private fun unpark(atUs: Long): Connection? {
    expiry?.cancel(false)
    val p = parked ?: return null
    parked = null
    if (p.startUs == atUs && p.rewind()) return p
    p.close()
    return null
  }

  @Synchronized
  private fun park(c: Connection) {
    parked?.close()
    parked = c
    expiry?.cancel(false)
    expiry = parking.schedule({ synchronized(this) { if (parked === c) { parked = null; c.close() } } }, PARK_MS, TimeUnit.MILLISECONDS)
  }

  /** Drops the parked stream, if any; the source isn't read anymore. */
  fun release() = synchronized(this) {
    expiry?.cancel(false)
    parked?.close()
    parked = null
  }

  private fun connect(atUs: Long): Connection {
    val request = Request.Builder().url(url(atUs)).apply { headers.forEach { (k, v) -> header(k, v) } }.build()
    val call = client.newCall(request)
    val response = call.execute()
    if (!response.isSuccessful) {
      val status = response.code
      val text = response.body?.string().orEmpty()
      response.close()
      val message = runCatching { JSONObject(text).optString("error") }.getOrNull()?.takeIf { it.isNotEmpty() }
      throw ServerError(status, message ?: "the server said $status")
    }
    return Connection(atUs, call, response)
  }

  private inner class StreamDataSource : BaseDataSource(/* isNetwork= */ true) {
    private var connection: Connection? = null
    private var uri: Uri? = null
    private var ended = false

    override fun open(dataSpec: DataSpec): Long {
      transferInitializing(dataSpec)
      // ExoPlayer only comes back mid-way to retry, and a stream can't pick up from a byte offset.
      if (dataSpec.position and OFFSET_MASK != 0L) throw IOException("the stream can't resume where it broke off")
      val at = startOf(dataSpec.position)
      uri = Uri.parse(url(at))
      ended = false
      connection = unpark(at) ?: connect(at)
      loadingFromUs = at
      transferStarted(dataSpec)
      return C.LENGTH_UNSET.toLong()
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
      if (length == 0) return 0
      val n = connection!!.read(buffer, offset, length)
      if (n < 0) {
        ended = true
        return C.RESULT_END_OF_INPUT
      }
      bytesTransferred(n)
      return n
    }

    override fun getUri() = uri

    override fun getResponseHeaders() = connection?.headers ?: emptyMap()

    override fun close() {
      val c = connection ?: return
      connection = null
      if (!ended && c.replayable) park(c) else c.close()
      transferEnded()
    }
  }

  /** The stream spans the whole file and can be started anywhere. */
  private val seekMap: SeekMap = object : SeekMap {
    override fun isSeekable(): Boolean = true

    override fun getDurationUs(): Long = this@StreamSource.durationUs

    override fun getSeekPoints(timeUs: Long): SeekMap.SeekPoints = SeekMap.SeekPoints(SeekPoint(timeUs, positionOf(timeUs)))
  }

  /** The fMP4 extractor, with the seek map above in place of its own (it can't seek a live fragmented file). */
  private inner class StreamExtractor(
    private val inner: Extractor = FragmentedMp4Extractor(SubtitleParser.Factory.UNSUPPORTED, /* flags= */ 0),
  ) : Extractor by inner {
    override fun init(output: ExtractorOutput) {
      inner.init(object : ExtractorOutput by output {
        override fun seekMap(seekMap: SeekMap) = output.seekMap(this@StreamSource.seekMap)
      })
    }
  }
}

/**
 * Loading errors aren't retried by ExoPlayer, which would come back mid-way
 * through a stream: the player fails once what's buffered runs out, and the
 * view starts over from where playback is.
 */
@UnstableApi
private object NoRetries : DefaultLoadErrorHandlingPolicy() {
  override fun getRetryDelayMsFor(loadErrorInfo: LoadErrorHandlingPolicy.LoadErrorInfo) = C.TIME_UNSET

  override fun getMinimumLoadableRetryCount(dataType: Int) = 0
}

/**
 * One response from the server, with its first bytes kept so it can be read
 * again from the top. Its body is read by a thread of its own: ExoPlayer
 * interrupts its loading thread to cancel a load, and an interrupted read
 * would leave OkHttp's stream finished for good. Here only the wait for
 * the next chunk is interrupted. The pump stops reading while its queue
 * is full, so a full buffer still holds the server back.
 */
internal class Connection(val startUs: Long, private val call: Call, val response: Response) {
  private val chunks = ArrayBlockingQueue<ByteArray>(PUMP_CHUNKS)
  @Volatile private var failure: IOException? = null
  private var current: ByteArray? = null
  private var at = 0
  private var finished = false

  private var kept: ByteArray? = ByteArray(REPLAY_BYTES)
  private var keptSize = 0
  private var replay = -1

  val headers: Map<String, List<String>> = response.headers.toMultimap()

  private val pump = Thread({
    val input = response.body!!.byteStream()
    try {
      while (true) {
        val chunk = ByteArray(PUMP_CHUNK)
        val n = input.read(chunk)
        if (n < 0) break
        chunks.put(if (n == chunk.size) chunk else chunk.copyOf(n))
      }
      chunks.put(END)
    } catch (_: InterruptedException) {
      // Closed.
    } catch (e: IOException) {
      failure = e
      runCatching { chunks.put(END) }
    }
  }, "tinystream-stream").apply {
    isDaemon = true
    start()
  }

  /** Starts reading from the top again; false once too much has gone by to do that, or the stream broke. */
  fun rewind(): Boolean {
    if (!replayable) return false
    replay = 0
    return true
  }

  fun read(buffer: ByteArray, offset: Int, length: Int): Int {
    val k = kept
    if (k != null && replay in 0 until keptSize) {
      val n = minOf(length, keptSize - replay)
      System.arraycopy(k, replay, buffer, offset, n)
      replay += n
      return n
    }
    val n = next(buffer, offset, length)
    if (n > 0 && k != null) {
      if (keptSize + n <= k.size) {
        System.arraycopy(buffer, offset, k, keptSize, n)
        keptSize += n
        replay = keptSize
      } else {
        kept = null
      }
    }
    return n
  }

  private fun next(buffer: ByteArray, offset: Int, length: Int): Int {
    var c = current
    if (c == null || at >= c.size) {
      if (finished) return failure?.let { throw it } ?: -1
      c = try {
        chunks.take()
      } catch (_: InterruptedException) {
        throw InterruptedIOException()
      }
      if (c === END) {
        finished = true
        return failure?.let { throw it } ?: -1
      }
      current = c
      at = 0
    }
    val n = minOf(length, c.size - at)
    System.arraycopy(c, at, buffer, offset, n)
    at += n
    return n
  }

  val replayable get() = kept != null && failure == null && !finished

  fun close() {
    call.cancel()
    pump.interrupt()
    response.close()
  }
}
