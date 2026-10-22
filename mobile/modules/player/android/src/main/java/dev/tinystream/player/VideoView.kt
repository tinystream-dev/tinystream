// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.player

import android.animation.ValueAnimator
import android.app.Activity
import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Rect
import android.graphics.drawable.Icon
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.util.Rational
import android.view.SurfaceView
import android.view.animation.PathInterpolator
import androidx.core.app.OnPictureInPictureModeChangedProvider
import androidx.core.app.PictureInPictureModeChangedInfo
import androidx.core.content.ContextCompat
import androidx.core.util.Consumer
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import com.facebook.react.ReactApplication
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.viewevent.EventDispatcher
import expo.modules.kotlin.views.ExpoView
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** As web's engine.ts: read ahead at most this far, and keep this much behind. */
private const val BUFFER_AHEAD_MS = 60_000
private const val KEEP_BEHIND_MS = 30_000

/** How often the position goes to JS. */
private const val PROGRESS_MS = 200L

/** Waits between attempts to pick a broken stream up again, then it's given up on. */
private val RETRY_MS = longArrayOf(1_000, 2_000, 4_000, 8_000, 16_000, 30_000)

/** PiP windows can't be any narrower or wider than this. */
private const val PIP_MAX_RATIO = 2.39

private const val PIP_ACTION = "dev.tinystream.player.PIP"

private const val TAG = "TinystreamPlayer"

/** What went wrong, cause by cause, for the debug log. */
private fun describe(error: PlaybackException): String {
  val parts = mutableListOf(error.errorCodeName)
  var cause = error.cause
  while (cause != null && parts.size < 6) {
    parts += "${cause.javaClass.simpleName}: ${cause.message}"
    cause = cause.cause
  }
  return parts.joinToString(" ← ")
}

/** What `load` is given: the stream, and what it is for the system's media controls. */
class Load(
  val url: String,
  val headers: Map<String, String>,
  val plan: StreamPlan,
  val startAt: Double,
  val duration: Double,
  val paused: Boolean,
  val title: String?,
  val subtitle: String?,
  val artwork: String?,
  val hasPrevious: Boolean,
  val hasNext: Boolean,
)

/** The app's network security config (plain HTTP, the user's CAs) applies to it as to everything else. */
private val client: OkHttpClient by lazy {
  OkHttpClient.Builder()
    .connectTimeout(15, TimeUnit.SECONDS)
    .readTimeout(30, TimeUnit.SECONDS)
    .build()
}

/**
 * A video playing tinystream's stream: ExoPlayer on a SurfaceView, scaled
 * to fit or fill, with the system's media controls and picture-in-picture
 * while it plays. Times are seconds in the file, whatever request the
 * picture came from.
 */
@UnstableApi
class VideoView(context: Context, appContext: AppContext) : ExpoView(context, appContext) {
  private val onProgress by EventDispatcher()
  private val onStatus by EventDispatcher()
  private val onPlaybackError by EventDispatcher()
  private val onVideoSize by EventDispatcher()
  private val onPip by EventDispatcher()
  private val onRemote by EventDispatcher()

  private val handler = Handler(Looper.getMainLooper())
  private val surface = SurfaceView(context)
  private val player: ExoPlayer = ExoPlayer.Builder(context)
    .setLoadControl(
      DefaultLoadControl.Builder()
        .setBufferDurationsMs(BUFFER_AHEAD_MS - 10_000, BUFFER_AHEAD_MS, 1_000, 2_000)
        .setBackBuffer(KEEP_BEHIND_MS, /* retainBackBufferFromKeyframe= */ true)
        .build(),
    )
    .setAudioAttributes(AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build(), /* handleAudioFocus= */ true)
    .setHandleAudioBecomingNoisy(true)
    .setSeekBackIncrementMs(10_000)
    .setSeekForwardIncrementMs(10_000)
    .build()
  private val subtitles = SubtitleView(context, client) { player.currentPosition }

  private var load: Load? = null
  private var source: StreamSource? = null
  private var session: MediaSession? = null
  private var sessions = 0
  private var attempt = 0
  private var retrying: Runnable? = null
  private var failed = false

  private var videoWidth = 0
  private var videoHeight = 0
  private var pixelRatio = 1f

  /** 0 fits the whole picture in, 1 fills the view with it (cropping); in between while changing. */
  private var fill = 0f
  private var fillTo = 0f
  private var fillAnimation: ValueAnimator? = null

  private var pipActive = false

  init {
    clipChildren = true
    addView(surface)
    addView(subtitles)
    player.setVideoSurfaceView(surface)
    player.addListener(object : Player.Listener {
      override fun onPlaybackStateChanged(state: Int) {
        if (state == Player.STATE_READY) attempt = 0
        if (state == Player.STATE_ENDED && endedEarly()) restart()
        else status()
      }

      override fun onIsPlayingChanged(isPlaying: Boolean) {
        keepScreenOn = isPlaying
        status()
        progress()
        updatePip()
      }

      override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) = status()

      override fun onPlaybackParametersChanged(parameters: PlaybackParameters) = progress()

      override fun onPlayerError(error: PlaybackException) = failed(error)

      override fun onVideoSizeChanged(size: VideoSize) {
        if (size.width == 0 || size.height == 0) return
        videoWidth = size.width
        videoHeight = size.height
        pixelRatio = size.pixelWidthHeightRatio
        onVideoSize(mapOf("width" to (videoWidth * pixelRatio).roundToInt(), "height" to videoHeight))
        relayout()
        updatePip()
      }
    })
  }

  private val tick = object : Runnable {
    override fun run() {
      progress()
      handler.postDelayed(this, PROGRESS_MS)
    }
  }

  fun load(next: Load) {
    cancelRetry()
    failed = false
    attempt = 0
    source?.release()
    load = next
    source = StreamSource(client, next.url, next.headers, next.plan, next.duration)
    start((next.startAt * 1000).toLong())
    player.playWhenReady = !next.paused
    val previous = session
    session = MediaSession.Builder(context, RemotePlayer(player, next.hasPrevious, next.hasNext))
      .setId("tinystream.video.${System.identityHashCode(this)}.${++sessions}")
      .setBitmapLoader(DataSourceBitmapLoader.Builder(context).setDataSourceFactory(DefaultHttpDataSource.Factory().setDefaultRequestProperties(next.headers)).build())
      .apply { openApp()?.let(::setSessionActivity) }
      .build()
      .also { VideoSessionService.show(context, it) }
    previous?.release()
    status()
    progress()
  }

  fun play() {
    if (player.playbackState == Player.STATE_ENDED) player.seekTo(0)
    if (failed) restart()
    player.play()
  }

  fun pause() = player.pause()

  fun seek(seconds: Double) {
    val duration = load?.duration?.takeIf { it > 0 } ?: Double.MAX_VALUE
    player.seekTo((seconds.coerceIn(0.0, duration) * 1000).toLong())
    if (failed) restart()
    progress()
  }

  fun setRate(rate: Double) = player.setPlaybackSpeed(rate.toFloat())

  /** Shows `source`'s subtitles over the picture, or none; `done` gets what went wrong, if anything did. */
  fun selectSubtitles(source: SubtitleSource?, done: (String?) -> Unit) = subtitles.select(source, done)

  fun setMuted(muted: Boolean) {
    player.volume = if (muted) 0f else 1f
  }

  fun setFill(on: Boolean) {
    val to = if (on) 1f else 0f
    if (to == fillTo) return
    fillTo = to
    fillAnimation?.cancel()
    fillAnimation = ValueAnimator.ofFloat(fill, to).apply {
      duration = 260
      interpolator = PathInterpolator(0.2f, 0.8f, 0.2f, 1f)
      addUpdateListener {
        fill = it.animatedValue as Float
        relayout()
      }
      start()
    }
  }

  /** Into picture-in-picture now (the button); false when it isn't allowed. */
  fun enterPip(): Boolean {
    val activity = appContext.currentActivity ?: return false
    return runCatching { activity.enterPictureInPictureMode(pipParams()) }.getOrDefault(false)
  }

  /** Where the media notification takes you: back to the app, and the video in it. */
  private fun openApp(): PendingIntent? {
    val intent = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return null
    return PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
  }

  private fun endedEarly(): Boolean {
    val duration = load?.duration ?: return false
    return duration > 0 && player.currentPosition / 1000.0 < duration - 2
  }

  private fun failed(error: PlaybackException) {
    Log.e(TAG, "playback failed (${error.errorCodeName})", error)
    var cause: Throwable? = error
    while (cause != null && cause !is ServerError) cause = cause.cause
    val network = cause == null && error.errorCode in PlaybackException.ERROR_CODE_IO_UNSPECIFIED until PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED
    if (network && attempt < RETRY_MS.size) {
      onPlaybackError(mapOf("message" to describe(error), "retrying" to true))
      val wait = RETRY_MS[attempt++]
      retrying = Runnable { restart() }.also { handler.postDelayed(it, wait) }
      status()
      return
    }
    failed = true
    val message = (cause as? ServerError)?.message ?: if (network) "the connection to the server was lost" else "this device could not play this stream"
    onPlaybackError(mapOf("message" to message, "retrying" to false, "detail" to describe(error)))
    status()
  }

  /** (Re)starts the stream at `positionMs`, as a fresh piece of media. */
  private fun start(positionMs: Long) {
    val s = source ?: return
    val next = load ?: return
    s.startUs = positionMs * 1000
    val metadata = MediaMetadata.Builder()
      .setTitle(next.title)
      .setArtist(next.subtitle)
      .setDisplayTitle(next.title)
      .setArtworkUri(next.artwork?.let(Uri::parse))
      .build()
    player.setMediaSource(s.mediaSource(MediaItem.Builder().setMediaMetadata(metadata)), positionMs)
    player.prepare()
  }

  /** Starts the stream over from where playback is: after a failure, or when it ended before the file did. */
  private fun restart() {
    cancelRetry()
    failed = false
    start(player.currentPosition)
    status()
  }

  private fun cancelRetry() {
    retrying?.let(handler::removeCallbacks)
    retrying = null
  }

  private fun status() {
    val state = when {
      retrying != null -> "buffering"
      failed -> "error"
      else -> when (player.playbackState) {
        Player.STATE_BUFFERING -> "buffering"
        Player.STATE_READY -> "ready"
        Player.STATE_ENDED -> "ended"
        else -> if (load == null) "idle" else "buffering"
      }
    }
    onStatus(mapOf("state" to state, "playing" to player.isPlaying, "paused" to !player.playWhenReady))
  }

  private fun progress() {
    if (load == null) return
    val position = player.currentPosition / 1000.0
    val end = player.bufferedPosition / 1000.0
    // ExoPlayer keeps one stretch: from the stream's start (or as far back as it keeps) to how far it has read.
    val from = max(source?.loadingFromUs?.let { it / 1_000_000.0 } ?: position, position - KEEP_BEHIND_MS / 1000.0)
    onProgress(
      buildMap {
        put("position", position)
        if (end > from) put("buffered", listOf(min(from, position), end))
        put("rate", player.playbackParameters.speed.toDouble())
        put("playing", player.isPlaying)
      },
    )
  }

  // React Native lays this view out but not what's in it: that's done here, for the picture's own shape.
  override fun requestLayout() {
    super.requestLayout()
    post { relayout() }
  }

  override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) = relayout()

  /** Where the picture is in this view: centred, fitted or filling it (or in between). */
  private fun pictureRect(): Rect {
    val w = width
    val h = height
    if (videoWidth == 0 || w == 0 || h == 0) return Rect(0, 0, w, h)
    val vw = videoWidth * pixelRatio
    val vh = videoHeight.toFloat()
    val fit = min(w / vw, h / vh)
    val cover = max(w / vw, h / vh)
    val scale = fit + (cover - fit) * fill
    val pw = (vw * scale).roundToInt()
    val ph = (vh * scale).roundToInt()
    val x = (w - pw) / 2
    val y = (h - ph) / 2
    return Rect(x, y, x + pw, y + ph)
  }

  private fun relayout() {
    val rect = pictureRect()
    surface.measure(MeasureSpec.makeMeasureSpec(rect.width(), MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(rect.height(), MeasureSpec.EXACTLY))
    surface.layout(rect.left, rect.top, rect.right, rect.bottom)
    // Subtitles cover the whole view, so the dialogue of a cropped picture stays in sight.
    subtitles.measure(MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY), MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY))
    subtitles.layout(0, 0, width, height)
    subtitles.place(width, height, rect, (videoWidth * pixelRatio).roundToInt(), videoHeight)
    if (rect != laidOut) {
      laidOut = rect
      handler.removeCallbacks(pipUpdate)
      handler.postDelayed(pipUpdate, 100)
    }
  }

  private var laidOut = Rect()
  private val pipUpdate = Runnable { updatePip() }

  // Picture-in-picture: entered by itself when leaving the app while playing.

  private fun pipParams(): PictureInPictureParams {
    val builder = PictureInPictureParams.Builder()
      .setAutoEnterEnabled(player.isPlaying)
      .setSeamlessResizeEnabled(false)
      .setActions(pipActions())
    if (videoWidth > 0) {
      val ratio = (videoWidth * pixelRatio / videoHeight).toDouble().coerceIn(1 / PIP_MAX_RATIO, PIP_MAX_RATIO)
      builder.setAspectRatio(Rational((ratio * 10_000).roundToInt(), 10_000))
      val rect = pictureRect()
      val visible = Rect()
      if (getGlobalVisibleRect(visible)) {
        rect.offset(visible.left - scrollX, visible.top - scrollY)
        if (rect.intersect(visible)) builder.setSourceRectHint(rect)
      }
    }
    return builder.build()
  }

  private fun pipActions(): List<RemoteAction> {
    fun action(id: String, icon: Int, title: String, code: Int) = RemoteAction(
      Icon.createWithResource(context, icon),
      title,
      title,
      PendingIntent.getBroadcast(
        context,
        code,
        Intent(PIP_ACTION).setPackage(context.packageName).putExtra("action", id),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
      ),
    )
    return listOf(
      action("back", R.drawable.tinystream_pip_back, "Back 10 seconds", 1),
      if (player.playWhenReady) action("pause", R.drawable.tinystream_pip_pause, "Pause", 2)
      else action("play", R.drawable.tinystream_pip_play, "Play", 3),
      action("forward", R.drawable.tinystream_pip_forward, "Forward 10 seconds", 4),
    )
  }

  private fun updatePip() {
    if (!isAttachedToWindow) return
    val activity = appContext.currentActivity ?: return
    runCatching { activity.setPictureInPictureParams(pipParams()) }
  }

  private val pipReceiver = object : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
      when (intent.getStringExtra("action")) {
        "back" -> seek(player.currentPosition / 1000.0 - 10)
        "forward" -> seek(player.currentPosition / 1000.0 + 10)
        "play" -> play()
        "pause" -> pause()
      }
      updatePip()
    }
  }

  private val pipListener = Consumer<PictureInPictureModeChangedInfo> { info ->
    pipActive = info.isInPictureInPictureMode
    onPip(mapOf("active" to pipActive))
    keepReactDrawing()
  }

  /** Leaving the app (or closing the PiP window) pauses: there's no video playing out of sight. */
  private val lifecycle = LifecycleEventObserver { _, event ->
    when (event) {
      // After the activity's own onPause, which this comes before.
      Lifecycle.Event.ON_PAUSE -> handler.post { keepReactDrawing() }
      Lifecycle.Event.ON_STOP -> {
        player.pause()
        if (reactKeptDrawing) reactHost()?.onHostPause(activity)
        reactKeptDrawing = false
      }
      else -> {}
    }
  }

  /**
   * The activity is paused in PiP, and a paused React Native stops putting
   * what JS renders on screen: the controls would stay up, at full-screen
   * size, in the small window. So it's kept going until the window closes.
   */
  private fun keepReactDrawing() {
    val a = activity ?: return
    if (!a.isInPictureInPictureMode || a.lifecycle()?.currentState?.isAtLeast(Lifecycle.State.RESUMED) == true) return
    reactHost()?.onHostResume(a)
    reactKeptDrawing = true
  }

  private var reactKeptDrawing = false

  private fun reactHost() = (activity?.application as? ReactApplication)?.reactHost

  private fun Activity.lifecycle() = (this as? LifecycleOwner)?.lifecycle

  private var activity: Activity? = null

  override fun onAttachedToWindow() {
    super.onAttachedToWindow()
    handler.post(tick)
    ContextCompat.registerReceiver(context, pipReceiver, IntentFilter(PIP_ACTION), ContextCompat.RECEIVER_NOT_EXPORTED)
    val a = appContext.currentActivity
    activity = a
    (a as? OnPictureInPictureModeChangedProvider)?.addOnPictureInPictureModeChangedListener(pipListener)
    (a as? LifecycleOwner)?.lifecycle?.addObserver(lifecycle)
    updatePip()
  }

  override fun onDetachedFromWindow() {
    super.onDetachedFromWindow()
    handler.removeCallbacks(tick)
    runCatching { context.unregisterReceiver(pipReceiver) }
    val a = activity
    (a as? OnPictureInPictureModeChangedProvider)?.removeOnPictureInPictureModeChangedListener(pipListener)
    (a as? LifecycleOwner)?.lifecycle?.removeObserver(lifecycle)
    runCatching { a?.setPictureInPictureParams(PictureInPictureParams.Builder().setAutoEnterEnabled(false).setActions(emptyList()).build()) }
    activity = null
  }

  fun release() {
    cancelRetry()
    handler.removeCallbacksAndMessages(null)
    fillAnimation?.cancel()
    subtitles.release()
    session?.let {
      VideoSessionService.hide(context, it)
      it.release()
    }
    session = null
    player.release()
    source?.release()
    source = null
  }

  /**
   * The player as the system's media controls and headphone buttons see it:
   * previous and next episode go to JS, which knows what they are.
   */
  private inner class RemotePlayer(player: Player, private val hasPrevious: Boolean, private val hasNext: Boolean) : ForwardingPlayer(player) {
    override fun getAvailableCommands(): Player.Commands =
      super.getAvailableCommands().buildUpon()
        .addIf(Player.COMMAND_SEEK_TO_PREVIOUS, hasPrevious)
        .addIf(Player.COMMAND_SEEK_TO_NEXT, hasNext)
        .build()

    override fun isCommandAvailable(command: Int) = when (command) {
      Player.COMMAND_SEEK_TO_PREVIOUS -> hasPrevious
      Player.COMMAND_SEEK_TO_NEXT -> hasNext
      else -> super.isCommandAvailable(command)
    }

    // Headphones and the like can't start a video nobody can see.
    override fun play() {
      if (visible()) super.play()
    }

    override fun setPlayWhenReady(playWhenReady: Boolean) {
      if (!playWhenReady || visible()) super.setPlayWhenReady(playWhenReady)
    }

    private fun visible() = (activity as? LifecycleOwner)?.lifecycle?.currentState?.isAtLeast(Lifecycle.State.STARTED) ?: false

    override fun seekToPrevious() {
      if (hasPrevious) onRemote(mapOf("action" to "previous"))
    }

    override fun seekToNext() {
      if (hasNext) onRemote(mapOf("action" to "next"))
    }
  }
}
