// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import android.graphics.Bitmap

/**
 * rustyass, through mobile/crates/subtitles: one script and its fonts per
 * handle, used from one thread at a time.
 */
internal object Ass {
  init {
    System.loadLibrary("tinystream_subtitles")
  }

  /** A handle for `script` (an ASS file) with `fonts`, falling back on `default`; 0 when it isn't a script. */
  external fun open(script: ByteArray, fonts: Array<ByteArray>, default: ByteArray): Long

  external fun close(handle: Long)

  /**
   * Draws into a `width`×`height` frame, for a video `storageWidth`×`storageHeight`
   * that's this far inside it: negative margins when it's cropped.
   */
  external fun resize(handle: Long, width: Int, height: Int, storageWidth: Int, storageHeight: Int, top: Int, bottom: Int, left: Int, right: Int)

  /** `[x, y, width, height]` of what shows at `ms` (empty when nothing does), or null when that hasn't changed. */
  external fun render(handle: Long, ms: Long, force: Boolean): IntArray?

  /** Paints the last render into `bitmap` (ARGB_8888, the size `render` gave). */
  external fun paint(handle: Long, bitmap: Bitmap): Boolean
}
