// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import org.junit.Assert.*
import org.junit.Test

class PlaybackClockTest {
  @Test fun transitionsFollowAudibleFrames() {
    val clock = PlaybackClock(48000)
    val started = mutableListOf<String>()
    clock.mark(Mark("a", 0, 12.0, 100.0))
    clock.mark(Mark("b", 48000, 0.0, 100.0))
    assertEquals(12.5, clock.advance(24000, started::add)!!.time, 0.0001)
    assertEquals(listOf("a"), started)
    assertEquals("b", clock.advance(48000, started::add)!!.key)
    assertEquals(listOf("a", "b"), started)
    clock.end(96000)
    assertFalse(clock.ended(95999))
    assertTrue(clock.ended(96000))
  }
  @Test fun unsignedHeadWrapsAndResetDiscardsOldMarks() {
    val clock = PlaybackClock(48000)
    assertEquals(4294967295L, clock.frames(-1))
    assertEquals(4294967306L, clock.frames(10))
    clock.mark(Mark("old", 0, 0.0, 10.0))
    clock.reset()
    assertEquals(0L, clock.frames(0))
    assertNull(clock.advance(0) { fail("Old track survived reset") })
  }
  @Test fun rewindingDropsDecodedButUnheardTransitions() {
    val clock = PlaybackClock(48000)
    clock.mark(Mark("a", 0, 0.0, 10.0))
    clock.mark(Mark("discarded", 48000, 0.0, 10.0))
    clock.rewind(48000)
    assertEquals("a", clock.advance(50000) {}!!.key)
  }
}
