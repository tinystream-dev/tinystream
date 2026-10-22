// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import android.content.Context
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.RendererCapabilities
import androidx.media3.exoplayer.audio.MediaCodecAudioRenderer
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.video.MediaCodecVideoRenderer

/** What a track is, as `canDecode` is asked about it. */
class Track(val codecs: String, val width: Int?, val height: Int?, val fps: Double?, val channels: Int?)

/** Video software can't decode smoothly enough on a phone. */
private const val SOFTWARE_MAX_HEIGHT = 1080

/**
 * Whether this device plays a track as it is, asked of the same renderers
 * that will play it, so profiles and levels (HEVC Main10, AV1, VP9 profile 2,
 * Dolby Vision…) and the audio output (AC-3, DTS…) count just as they will.
 */
@UnstableApi
class Codecs(context: Context) {
  private val video by lazy { MediaCodecVideoRenderer.Builder(context).setMediaCodecSelector(MediaCodecSelector.DEFAULT).build() }
  private val audio by lazy { MediaCodecAudioRenderer(context, MediaCodecSelector.DEFAULT) }

  @Synchronized
  fun canDecode(mime: String, track: Track): Boolean {
    val sample = MimeTypes.getMediaMimeType(track.codecs) ?: return false
    val isVideo = mime.startsWith("video/")
    if (isVideo != MimeTypes.isVideo(sample)) return false
    val format = Format.Builder()
      .setContainerMimeType(MimeTypes.VIDEO_MP4)
      .setSampleMimeType(sample)
      .setCodecs(track.codecs)
      .apply {
        track.width?.let(::setWidth)
        track.height?.let(::setHeight)
        track.fps?.let { setFrameRate(it.toFloat()) }
        track.channels?.let(::setChannelCount)
      }
      .build()
    val capabilities = runCatching { (if (isVideo) video else audio).supportsFormat(format) }.getOrElse { return false }
    if (RendererCapabilities.getFormatSupport(capabilities) != C.FORMAT_HANDLED) return false
    if (!isVideo) return true
    val hardware = RendererCapabilities.getHardwareAccelerationSupport(capabilities) == RendererCapabilities.HARDWARE_ACCELERATION_SUPPORTED
    return hardware || (track.height ?: 0) <= SOFTWARE_MAX_HEIGHT
  }
}
