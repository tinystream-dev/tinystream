// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import java.io.RandomAccessFile
import java.net.URI
import java.util.concurrent.TimeUnit
import okhttp3.OkHttpClient
import okhttp3.Request
import uniffi.tinystream_decoder_android.AudioException
import uniffi.tinystream_decoder_android.RangeSource

class Source(private val url: String, private val size: Long, private val headers: Map<String, String>) : RangeSource {
  override fun len(): ULong = io {
    if (size > 0) size.toULong()
    else if (url.startsWith("file:")) java.io.File(URI(url)).length().toULong()
    else client.newCall(request().head().build()).execute().use { response ->
      if (!response.isSuccessful) throw java.io.IOException("the server said ${response.code}")
      (response.header("Content-Length")?.toULongOrNull() ?: throw java.io.IOException("missing file length"))
    }
  }

  override fun readRange(start: ULong, end: ULong): ByteArray = io {
    if (url.startsWith("file:")) {
      RandomAccessFile(java.io.File(URI(url)), "r").use { file ->
        file.seek(start.toLong())
        ByteArray((end - start + 1u).toInt()).also(file::readFully)
      }
    } else client.newCall(request().header("Range", "bytes=$start-$end").build()).execute().use { response ->
      if (response.code != 206 && !(response.code == 200 && start == 0uL)) throw java.io.IOException("invalid range response (${response.code})")
      val bytes = response.body?.bytes() ?: throw java.io.IOException("empty range response")
      if (bytes.size.toULong() != end - start + 1u) throw java.io.IOException("incomplete range response")
      bytes
    }
  }

  private fun request(): Request.Builder = Request.Builder().url(url).apply { headers.forEach { (k,v) -> header(k,v) } }
  private fun <T> io(read: () -> T): T = try { read() } catch (e: Exception) {
    throw AudioException.Read(e.message?.takeIf { !it.contains("://") } ?: "can't read the audio source")
  }
  companion object { val client = OkHttpClient.Builder().connectTimeout(15, TimeUnit.SECONDS).readTimeout(20, TimeUnit.SECONDS).build() }
}
