// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import okhttp3.Call
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import uniffi.tinystream_decoder_android.AudioException
import uniffi.tinystream_decoder_android.RangeSource

class Source(
  url: String,
  private val size: Long,
  private val headers: Map<String, String>,
  private val requests: Requests,
) : RangeSource {
  private val url = url.toHttpUrl()

  override fun len(): ULong = read {
    if (size > 0) return@read size.toULong()
    requests.execute(request().head().build()) { response ->
      if (!response.isSuccessful) throw IOException("Audio size request failed (${response.code})")
      response.header("Content-Length")?.toULongOrNull()?.takeIf { it > 0uL }
        ?: throw IOException("The server did not provide the audio size")
    }
  }

  override fun readRange(start: ULong, end: ULong): ByteArray = read {
    if (end < start || end - start >= 512uL * 1024uL) throw IOException("Invalid audio range")
    val count = (end - start + 1uL).toInt()
    requests.execute(request().header("Range", "bytes=$start-$end").build()) { response ->
      if (response.code != 206) throw IOException("The server did not honor the audio range")
      val range = response.header("Content-Range") ?: throw IOException("Missing audio range")
      if (!range.startsWith("bytes $start-$end/")) throw IOException("Unexpected audio range")
      val body = response.body ?: throw IOException("Empty audio response")
      body.byteStream().use { stream ->
        val bytes = ByteArray(count)
        var offset = 0
        while (offset < count) {
          val n = stream.read(bytes, offset, count - offset)
          if (n < 0) throw IOException("The audio download stopped")
          offset += n
        }
        if (stream.read() != -1) throw IOException("Oversized audio response")
        bytes
      }
    }
  }

  private fun request() = Request.Builder().url(url).apply {
    headers.forEach { (name, value) -> header(name, value) }
  }

  private fun <T> read(action: () -> T): T = try { action() } catch (error: Exception) {
    throw AudioException.Read(if (requests.cancelled) "Playback cancelled" else "Could not read the audio source")
  }
}

class Requests {
  @Volatile var cancelled = false
    private set
  private val calls = ConcurrentHashMap.newKeySet<Call>()

  fun <T> execute(request: Request, consume: (okhttp3.Response) -> T): T {
    if (cancelled) throw IOException("Playback cancelled")
    val call = client.newCall(request)
    calls.add(call)
    try {
      if (cancelled) call.cancel()
      return call.execute().use(consume)
    } finally { calls.remove(call) }
  }

  fun cancel() {
    cancelled = true
    calls.forEach { it.cancel() }
  }

  companion object {
    private val client = OkHttpClient.Builder()
      .followRedirects(false)
      .followSslRedirects(false)
      .connectTimeout(15, TimeUnit.SECONDS)
      .readTimeout(20, TimeUnit.SECONDS)
      .callTimeout(30, TimeUnit.SECONDS)
      .build()
  }
}
