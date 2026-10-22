// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import okhttp3.OkHttpClient
import okhttp3.Request
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.io.InterruptedIOException
import java.net.ServerSocket
import kotlin.concurrent.thread
import kotlin.random.Random

class ConnectionTest {
  private val server = ServerSocket(0)
  private val client = OkHttpClient()

  @After
  fun close() = server.close()

  /** Serves `body` once, slowly, so reads wait on the network. */
  private fun serve(body: ByteArray) = thread(isDaemon = true) {
    server.accept().use { socket ->
      val input = socket.getInputStream().bufferedReader()
      while (input.readLine().isNotEmpty()) Unit
      val out = socket.getOutputStream()
      out.write("HTTP/1.1 200 OK\r\nContent-Length: ${body.size}\r\n\r\n".toByteArray())
      var at = 0
      while (at < body.size) {
        val n = minOf(32 shl 10, body.size - at)
        out.write(body, at, n)
        out.flush()
        at += n
        Thread.sleep(2)
      }
    }
  }

  private fun open(): Connection {
    val call = client.newCall(Request.Builder().url("http://127.0.0.1:${server.localPort}/stream").build())
    return Connection(0, call, call.execute())
  }

  private fun Connection.readAll(into: ByteArrayOutputStream = ByteArrayOutputStream()): ByteArray {
    val buffer = ByteArray(10_000)
    while (true) {
      val n = read(buffer, 0, buffer.size)
      if (n < 0) return into.toByteArray()
      into.write(buffer, 0, n)
    }
  }

  @Test
  fun `an interrupted read leaves the stream whole to read again from the top`() {
    val body = Random(1).nextBytes(3 shl 20)
    serve(body)
    val c = open()
    var interrupted = false
    val reader = thread {
      val buffer = ByteArray(10_000)
      try {
        while (true) if (c.read(buffer, 0, buffer.size) < 0) break
      } catch (_: InterruptedIOException) {
        interrupted = true
      }
    }
    Thread.sleep(50)
    reader.interrupt()
    reader.join()
    assertTrue("the read should have been interrupted mid-way", interrupted)
    assertTrue(c.rewind())
    assertArrayEquals(body, c.readAll())
    c.close()
  }

  @Test
  fun `a stream read too far can't be rewound`() {
    val body = Random(2).nextBytes(6 shl 20)
    serve(body)
    val c = open()
    assertArrayEquals(body, c.readAll())
    assertFalse(c.rewind())
    c.close()
  }

  @Test
  fun `rewinding replays exactly what was read, then goes on`() {
    val body = Random(3).nextBytes(1 shl 20)
    serve(body)
    val c = open()
    val buffer = ByteArray(1000)
    var read = 0
    while (read < 200_000) read += c.read(buffer, 0, buffer.size).also { assertTrue(it > 0) }
    assertTrue(c.rewind())
    val again = c.readAll()
    assertEquals(body.size, again.size)
    assertArrayEquals(body, again)
    c.close()
  }
}
