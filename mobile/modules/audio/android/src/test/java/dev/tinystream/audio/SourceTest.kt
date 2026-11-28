// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import java.net.ServerSocket
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread
import org.junit.Assert.*
import org.junit.Test
import uniffi.tinystream_decoder_android.AudioException

class SourceTest {
  private fun server(response: String, test: (String, () -> String) -> Unit) {
    ServerSocket(0).use { server ->
      var headers = ""
      val worker = thread(isDaemon = true) {
        server.accept().use { socket ->
          socket.soTimeout = 3000
          val reader = socket.getInputStream().bufferedReader()
          val lines = mutableListOf<String>()
          while (true) { val line = reader.readLine() ?: break; if (line.isEmpty()) break; lines.add(line) }
          headers = lines.joinToString("\n")
          socket.getOutputStream().write(response.toByteArray())
        }
      }
      test("http://127.0.0.1:${server.localPort}/audio", { worker.join(3000); headers })
    }
  }
  @Test fun readsExactlyTheRequestedAuthenticatedRange() {
    server("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 1-3/5\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbcd") { url, headers ->
      val source = Source(url, 5, mapOf("Authorization" to "Bearer test-token"), Requests())
      assertArrayEquals("bcd".toByteArray(), source.readRange(1uL, 3uL))
      val sent = headers()
      assertTrue(sent.contains("Authorization: Bearer test-token"))
      assertTrue(sent.contains("Range: bytes=1-3"))
    }
  }
  @Test fun rejectsWrongRangesAndRedirects() {
    for (response in listOf(
      "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-2/5\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc",
      "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )) server(response) { url, headers ->
      assertThrows(AudioException.Read::class.java) { Source(url, 5, emptyMap(), Requests()).readRange(1uL, 3uL) }
      headers()
    }
  }
  @Test fun cancellingInterruptsAnOpenResponse() {
    ServerSocket(0).use { server ->
      val accepted = CountDownLatch(1)
      val release = CountDownLatch(1)
      val finished = CountDownLatch(1)
      thread(isDaemon = true) { server.accept().use { accepted.countDown(); release.await(5, TimeUnit.SECONDS) } }
      val requests = Requests()
      thread(isDaemon = true) {
        try { Source("http://127.0.0.1:${server.localPort}/audio", 5, emptyMap(), requests).readRange(0uL, 4uL) }
        catch (_: AudioException.Read) {} finally { finished.countDown() }
      }
      try {
        assertTrue(accepted.await(3, TimeUnit.SECONDS))
        requests.cancel()
        assertTrue(finished.await(2, TimeUnit.SECONDS))
      } finally { release.countDown() }
    }
  }
}
