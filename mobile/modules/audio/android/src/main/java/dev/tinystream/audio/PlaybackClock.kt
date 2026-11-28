// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

internal data class Mark(val key: String, val frame: Long, val offset: Double, val duration: Double)
internal data class Position(val key: String, val time: Double, val duration: Double)

internal class PlaybackClock(private val rate: Int) {
  private val marks = ArrayDeque<Mark>()
  private var current: Mark? = null
  private var lastHead = 0L
  private var wraps = 0L
  private var end: Long? = null
  init { require(rate > 0) }

  fun reset() {
    marks.clear()
    current = null
    lastHead = 0
    wraps = 0
    end = null
  }

  fun mark(mark: Mark) { marks.addLast(mark) }
  fun end(frame: Long) { end = frame }
  fun rewind(frame: Long) {
    while (marks.lastOrNull()?.let { it.frame >= frame } == true) marks.removeLast()
    end = null
  }

  fun frames(raw: Int): Long {
    val head = raw.toLong() and 0xffffffffL
    if (head < lastHead) wraps += 1L shl 32
    lastHead = head
    return wraps + head
  }

  fun advance(frame: Long, started: (String) -> Unit): Position? {
    while (marks.firstOrNull()?.let { it.frame <= frame } == true) {
      current = marks.removeFirst()
      started(current!!.key)
    }
    return current?.let {
      Position(it.key, (it.offset + (frame - it.frame).coerceAtLeast(0).toDouble() / rate)
        .coerceAtMost(if (it.duration > 0) it.duration else Double.MAX_VALUE), it.duration)
    }
  }

  fun ended(frame: Long): Boolean = end?.let { frame >= it } == true
}
