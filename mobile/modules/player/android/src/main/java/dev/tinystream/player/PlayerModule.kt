// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import android.content.Context
import android.content.pm.ActivityInfo
import android.media.AudioManager
import android.provider.Settings
import android.view.WindowManager
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.util.UnstableApi
import expo.modules.kotlin.Promise
import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.exception.Exceptions
import expo.modules.kotlin.functions.Queues
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.records.Field
import expo.modules.kotlin.records.Record

class TrackRecord : Record {
  @Field val codecs: String = ""
  @Field val width: Int? = null
  @Field val height: Int? = null
  @Field val fps: Double? = null
  @Field val channels: Int? = null
}

class PlanRecord : Record {
  @Field val video: String = "transcode"
  @Field val height: Int = 1080
  @Field val audio: Int? = null
  @Field val audioMode: String = "aac"
}

class LoadRecord : Record {
  @Field val url: String = ""
  @Field val headers: Map<String, String> = emptyMap()
  @Field val plan: PlanRecord = PlanRecord()
  @Field val startAt: Double = 0.0
  @Field val duration: Double = 0.0
  @Field val paused: Boolean = false
  @Field val title: String? = null
  @Field val subtitle: String? = null
  @Field val artwork: String? = null
  @Field val hasPrevious: Boolean = false
  @Field val hasNext: Boolean = false
}

class SubtitlesRecord : Record {
  @Field val url: String = ""
  @Field val fonts: List<String> = emptyList()
  @Field val headers: Map<String, String> = emptyMap()
}

/** The orientations the player asks for; `default` gives the app's own back. */
private val ORIENTATIONS = mapOf(
  "landscape" to ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE,
  "portrait" to ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT,
  "locked" to ActivityInfo.SCREEN_ORIENTATION_LOCKED,
)

@UnstableApi
class PlayerModule : Module() {
  private val context: Context
    get() = appContext.reactContext ?: throw Exceptions.ReactContextLost()

  private val codecs by lazy { Codecs(context) }
  private val audio get() = context.getSystemService(AudioManager::class.java)

  /** The orientation the activity had before the player changed it. */
  private var appOrientation: Int? = null

  override fun definition() = ModuleDefinition {
    Name("TinystreamPlayer")

    AsyncFunction("canDecode") { mime: String, track: TrackRecord ->
      codecs.canDecode(mime, Track(track.codecs, track.width, track.height, track.fps, track.channels))
    }

    Function("volume") {
      mapOf("level" to audio.getStreamVolume(AudioManager.STREAM_MUSIC), "max" to audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC))
    }

    Function("setVolume") { level: Int ->
      audio.setStreamVolume(AudioManager.STREAM_MUSIC, level.coerceIn(0, audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)), 0)
    }

    AsyncFunction("brightness") {
      val own = appContext.currentActivity?.window?.attributes?.screenBrightness ?: -1f
      if (own >= 0) own.toDouble()
      else Settings.System.getInt(context.contentResolver, Settings.System.SCREEN_BRIGHTNESS, 128) / 255.0
    }.runOnQueue(Queues.MAIN)

    /** The window's own brightness (0–1), or null to follow the system's again. */
    AsyncFunction("setBrightness") { level: Double? ->
      val window = appContext.currentActivity?.window ?: return@AsyncFunction
      window.attributes = window.attributes.apply {
        screenBrightness = level?.toFloat()?.coerceIn(0.01f, 1f) ?: WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
      }
    }.runOnQueue(Queues.MAIN)

    /** Hides the system bars (they come back for a while with a swipe), or shows them again. */
    AsyncFunction("setImmersive") { on: Boolean ->
      val window = appContext.currentActivity?.window ?: return@AsyncFunction
      val controller = WindowCompat.getInsetsController(window, window.decorView)
      if (on) {
        controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        controller.hide(WindowInsetsCompat.Type.systemBars())
      } else {
        controller.show(WindowInsetsCompat.Type.systemBars())
      }
    }.runOnQueue(Queues.MAIN)

    AsyncFunction("setOrientation") { name: String ->
      val activity = appContext.currentActivity ?: return@AsyncFunction
      if (name == "default") {
        appOrientation?.let { activity.requestedOrientation = it }
        appOrientation = null
        return@AsyncFunction
      }
      if (appOrientation == null) appOrientation = activity.requestedOrientation
      activity.requestedOrientation = ORIENTATIONS[name] ?: return@AsyncFunction
    }.runOnQueue(Queues.MAIN)

    View(VideoView::class) {
      Events("onProgress", "onStatus", "onPlaybackError", "onVideoSize", "onPip", "onRemote")

      Prop("fill") { view: VideoView, on: Boolean -> view.setFill(on) }

      AsyncFunction("load") { view: VideoView, load: LoadRecord ->
        val plan = StreamPlan(load.plan.video, load.plan.height, load.plan.audio, load.plan.audioMode)
        view.load(
          Load(load.url, load.headers, plan, load.startAt, load.duration, load.paused, load.title, load.subtitle, load.artwork, load.hasPrevious, load.hasNext),
        )
      }.runOnQueue(Queues.MAIN)

      AsyncFunction("play") { view: VideoView -> view.play() }.runOnQueue(Queues.MAIN)
      AsyncFunction("pause") { view: VideoView -> view.pause() }.runOnQueue(Queues.MAIN)
      AsyncFunction("seek") { view: VideoView, seconds: Double -> view.seek(seconds) }.runOnQueue(Queues.MAIN)
      AsyncFunction("setRate") { view: VideoView, rate: Double -> view.setRate(rate) }.runOnQueue(Queues.MAIN)
      AsyncFunction("setMuted") { view: VideoView, muted: Boolean -> view.setMuted(muted) }.runOnQueue(Queues.MAIN)
      AsyncFunction("enterPip") { view: VideoView -> view.enterPip() }.runOnQueue(Queues.MAIN)

      AsyncFunction("selectSubtitles") { view: VideoView, source: SubtitlesRecord?, promise: Promise ->
        view.selectSubtitles(source?.let { SubtitleSource(it.url, it.fonts, it.headers) }) { error ->
          if (error == null) promise.resolve(null) else promise.reject(CodedException("ERR_SUBTITLES", error, null))
        }
      }.runOnQueue(Queues.MAIN)

      OnViewDestroys { view: VideoView -> view.release() }
    }
  }
}
