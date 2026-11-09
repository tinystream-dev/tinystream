// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import uniffi.tinystream_decoder_android.Event
import uniffi.tinystream_decoder_android.Item
import uniffi.tinystream_decoder_android.Sequencer

internal data class AudioItem(
  val key: String, val url: String, val size: Long, val ext: String,
  val gain: Float, val crossfade: Double, val headers: Map<String, String>,
)

internal class AudioEngine(val rate: Int, private val emit: (String, Map<String, Any?>) -> Unit) {
  private val worker = Executors.newSingleThreadScheduledExecutor()
  private var decoder = Sequencer(rate.toUInt())
  private var output: AudioTrack? = null
  private val clock = PlaybackClock(rate)
  @Volatile private var requests = Requests()
  @Volatile private var generation = 0L
  @Volatile private var closed = false
  @Volatile var playing = false
    private set
  @Volatile var position: Position? = null
    private set
  private var pending = FloatArray(0)
  private var pendingAt = 0
  private var written = 0L
  private var finished = false
  private var waiting = false
  private var volume = 1f
  private var ducked = false
  private var lastProgress = 0L
  private var inputs = emptyList<AudioItem>()

  init {
    worker.scheduleWithFixedDelay({
      try { pump() } catch (error: Exception) {
        playing = false
        output?.pause()
        emit("error", mapOf("key" to position?.key, "message" to "Audio playback failed"))
      }
    }, 0, 10, TimeUnit.MILLISECONDS)
  }

  private fun native(items: List<AudioItem>) = items.map {
    Item(it.key, Source(it.url, it.size, it.headers, requests), it.ext, it.gain, it.crossfade)
  }

  fun load(items: List<AudioItem>, start: Double, ready: (Throwable?) -> Unit) {
    val revision = ++generation
    requests.cancel()
    worker.execute {
      if (closed || revision != generation) { ready(IllegalStateException("Playback cancelled")); return@execute }
      try {
        requests = Requests()
        output?.pause()
        output?.release()
        output = null
        decoder.stop()
        clock.reset()
        pending = FloatArray(0)
        pendingAt = 0
        written = 0
        position = null
        finished = false
        inputs = items
        val audio = AudioTrack.Builder()
          .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build())
          .setAudioFormat(AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_FLOAT).setSampleRate(rate).setChannelMask(AudioFormat.CHANNEL_OUT_STEREO).build())
          .setTransferMode(AudioTrack.MODE_STREAM)
          .setBufferSizeInBytes(maxOf(rate * 8 / 4, AudioTrack.getMinBufferSize(rate, AudioFormat.CHANNEL_OUT_STEREO, AudioFormat.ENCODING_PCM_FLOAT)))
          .build()
        check(audio.state == AudioTrack.STATE_INITIALIZED)
        output = audio
        audio.setVolume(volume * if (ducked) 0.2f else 1f)
        decoder.play(native(items), start.coerceAtLeast(0.0))
        if (revision != generation) { ready(IllegalStateException("Playback cancelled")); return@execute }
        if (playing) audio.play()
        ready(null)
      } catch (error: Exception) { ready(error) }
    }
  }

  fun play() { playing = true; worker.execute { output?.play() } }
  fun pause() { playing = false; worker.execute { output?.pause() } }
  fun volume(value: Float) { worker.execute { volume = value.coerceIn(0f, 1f); output?.setVolume(volume * if (ducked) 0.2f else 1f) } }
  fun duck(value: Boolean) { worker.execute { ducked = value; output?.setVolume(volume * if (ducked) 0.2f else 1f) } }
  fun gain(key: String, gain: Float) { worker.execute { decoder.gain(key, gain) } }

  fun upcoming(after: String, items: List<AudioItem>) {
    worker.execute {
      val index = inputs.indexOfFirst { it.key == after }
      if (index < 0) return@execute
      val heard = output?.let { clock.frames(it.playbackHeadPosition) } ?: 0L
      val rewind = decoder.upcoming(after, native(items), heard.toULong())
      inputs = inputs.take(index + 1) + items
      finished = false
      if (rewind >= written) {
        pending = pending.copyOf((pendingAt + (rewind - written) * 2).coerceIn(pendingAt.toLong(), pending.size.toLong()).toInt())
        clock.rewind(rewind)
      } else if (rewind >= 0) {
        val audible = inputs.firstOrNull { it.key == position?.key } ?: return@execute
        val remaining = inputs.dropWhile { it.key != audible.key }
        load(remaining, position?.time ?: 0.0) { error ->
          if (error != null) emit("error", mapOf("key" to audible.key, "message" to "Could not update the queue"))
        }
      }
    }
  }

  fun stop() {
    generation++
    requests.cancel()
    playing = false
    worker.execute {
      decoder.stop()
      output?.pause()
      output?.release()
      output = null
      clock.reset()
      pending = FloatArray(0)
      pendingAt = 0
      position = null
      finished = true
    }
  }

  private fun pump() {
    val audio = output ?: return
    if (!playing || closed) return
    val revision = generation
    val heard = clock.frames(audio.playbackHeadPosition)
    position = clock.advance(heard) { emit("track", mapOf("key" to it)) }
    val now = android.os.SystemClock.elapsedRealtime()
    if (now - lastProgress >= 200) {
      lastProgress = now
      position?.let { emit("position", mapOf("key" to it.key, "time" to it.time)) }
    }
    if (clock.ended(heard) && pendingAt >= pending.size) {
      playing = false
      audio.pause()
      emit("ended", emptyMap())
      return
    }
    if (!finished && pendingAt >= pending.size && written - heard < rate / 4) {
      val chunk = decoder.read(4096u)
      if (revision != generation) return
      pending = chunk.samples.toFloatArray()
      pendingAt = 0
      chunk.events.forEach { event ->
        when (event) {
          is Event.Mark -> clock.mark(Mark(event.key, event.frame.toLong(), event.offset, event.duration))
          is Event.End -> { clock.end(event.frame.toLong()); finished = true }
          is Event.Rewind -> clock.rewind(event.frame.toLong())
          is Event.Error -> emit("error", mapOf("key" to event.key, "message" to event.message))
        }
      }
    }
    if (pendingAt < pending.size) {
      val n = audio.write(pending, pendingAt, pending.size - pendingAt, AudioTrack.WRITE_NON_BLOCKING)
      check(n >= 0) { "Audio output failed" }
      pendingAt += n
      written += n / 2
    }
    val starved = !finished && written <= heard
    if (waiting != starved) { waiting = starved; emit("starved", mapOf("waiting" to starved)) }
  }

  fun close() {
    closed = true
    stop()
    worker.execute { decoder.close() }
    worker.shutdown()
  }
}
