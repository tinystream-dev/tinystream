// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import android.content.Context
import android.content.Intent
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Publishes the playing video's session to the system: the media
 * notification, the controls in quick settings, and Bluetooth buttons.
 * VideoView owns the session; this only shows it while the player is open.
 */
@UnstableApi
class VideoSessionService : MediaSessionService() {
  override fun onCreate() {
    super.onCreate()
    running = this
    session?.let(::addSession)
  }

  override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

  override fun onDestroy() {
    running = null
    super.onDestroy()
  }

  companion object {
    private var session: MediaSession? = null
    private var running: VideoSessionService? = null

    /** Puts `next` in the system's media controls, in place of whichever session was there. */
    fun show(context: Context, next: MediaSession) {
      val service = running
      session?.let { service?.removeSession(it) }
      session = next
      if (service != null) service.addSession(next)
      else context.startService(Intent(context, VideoSessionService::class.java))
    }

    /** Takes `old` out of the system's media controls, if it's the one there. */
    fun hide(context: Context, old: MediaSession) {
      if (session !== old) return
      running?.removeSession(old)
      session = null
      context.stopService(Intent(context, VideoSessionService::class.java))
    }
  }
}
