// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import dev.tinystream.audio.AudioService.Companion.data
import dev.tinystream.audio.AudioService.Companion.command
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Looper
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

@UnstableApi
class AudioService : MediaSessionService() {
  private lateinit var audio: AudioEngine
  private lateinit var player: SessionPlayer
  private var session: MediaSession? = null
  private val main = android.os.Handler(Looper.getMainLooper())
  private val manager by lazy { getSystemService(android.media.AudioManager::class.java) }
  private val wake by lazy { getSystemService(android.os.PowerManager::class.java).newWakeLock(android.os.PowerManager.PARTIAL_WAKE_LOCK, "tinystream:music") }
  private var resumeAfterFocus = false
  private val focus by lazy {
    android.media.AudioFocusRequest.Builder(android.media.AudioManager.AUDIOFOCUS_GAIN)
      .setAudioAttributes(android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_MEDIA).setContentType(android.media.AudioAttributes.CONTENT_TYPE_MUSIC).build())
      .setOnAudioFocusChangeListener { change ->
        when (change) {
          android.media.AudioManager.AUDIOFOCUS_LOSS -> { resumeAfterFocus = false; command("pause") }
          android.media.AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> { resumeAfterFocus = audio.playing; command("pause") }
          android.media.AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> audio.duck(true)
          android.media.AudioManager.AUDIOFOCUS_GAIN -> { audio.duck(false); if (resumeAfterFocus) { resumeAfterFocus = false; command("play") } }
        }
      }.build()
  }
  private val noisy = object : android.content.BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) { resumeAfterFocus = false; command("pause") }
  }
  private val update = object : Runnable {
    override fun run() {
      if (!audio.playing) {
        if (wake.isHeld) wake.release()
      } else if (!wake.isHeld) wake.acquire()
      player.refresh()
      main.postDelayed(this, 1000)
    }
  }
  override fun onCreate() {
    super.onCreate()
    setMediaNotificationProvider(DefaultMediaNotificationProvider.Builder(this).setNotificationId(740).build())
    audio=engine(this)
    registerReceiver(noisy, android.content.IntentFilter(android.media.AudioManager.ACTION_AUDIO_BECOMING_NOISY), Context.RECEIVER_NOT_EXPORTED)
    player=SessionPlayer(audio)
    session=MediaSession.Builder(this,player)
      .setBitmapLoader(DataSourceBitmapLoader.Builder(this).setDataSourceFactory(DataSource.Factory {
        DefaultHttpDataSource.Factory().setDefaultRequestProperties(data?.headers ?: emptyMap()).createDataSource()
      }).build())
      .setCallback(object: MediaSession.Callback {
      override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
        val commands=MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
        for (action in listOf("star","shuffle","repeat")) commands.add(SessionCommand(action,Bundle.EMPTY))
        return MediaSession.ConnectionResult.AcceptedResultBuilder(session).setAvailableSessionCommands(commands.build()).build()
      }
      override fun onCustomCommand(session: MediaSession, controller: MediaSession.ControllerInfo, command: SessionCommand, args: Bundle): ListenableFuture<SessionResult> {
        command(command.customAction)
        return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
      }
    }).build()
    addSession(session!!)
    main.post(update)
    refresh = {
      session?.setCustomLayout(listOf(
        CommandButton.Builder().setDisplayName(if(data?.starred==true) "Unstar" else "Star").setIconResId(android.R.drawable.btn_star_big_off).setSessionCommand(SessionCommand("star",Bundle.EMPTY)).build(),
        CommandButton.Builder().setDisplayName("Shuffle").setIconResId(android.R.drawable.ic_menu_rotate).setSessionCommand(SessionCommand("shuffle",Bundle.EMPTY)).build(),
        CommandButton.Builder().setDisplayName("Repeat").setIconResId(android.R.drawable.ic_menu_revert).setSessionCommand(SessionCommand("repeat",Bundle.EMPTY)).build(),
      ))
      player.refresh()
      triggerNotificationUpdate()
    }
  }
  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val notifications=getSystemService(NotificationManager::class.java)
    val notification=notifications.activeNotifications.firstOrNull {
      it.id==740 && it.notification.extras.containsKey(Notification.EXTRA_MEDIA_SESSION)
    }?.notification ?: run {
      notifications.createNotificationChannel(NotificationChannel("tinystream.music.start","Music",NotificationManager.IMPORTANCE_LOW))
      Notification.Builder(this,"tinystream.music.start").setSmallIcon(android.R.drawable.ic_media_play).setContentTitle(data?.title ?: "tinystream").setContentText("Starting music").build()
    }
    startForeground(740,notification)
    if (manager.requestAudioFocus(focus) == android.media.AudioManager.AUDIOFOCUS_REQUEST_GRANTED) audio.play() else command("pause")
    super.onStartCommand(intent,flags,startId)
    return START_NOT_STICKY
  }
  override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session
  override fun onTaskRemoved(rootIntent: Intent?) { if (data == null) stopSelf() }
  override fun onDestroy() {
    refresh = null
    main.removeCallbacks(update)
    unregisterReceiver(noisy)
    manager.abandonAudioFocusRequest(focus)
    if (wake.isHeld) wake.release()
    pause()
    session?.release(); player.release()

    super.onDestroy()
  }
  companion object {
    private var shared: AudioEngine? = null
    private var requested = false
    private var react: com.facebook.react.bridge.ReactContext? = null
    private var task: Int? = null
    private var listener: ((String, Map<String, Any?>) -> Unit)? = null
    private var refresh: (() -> Unit)? = null
    private val dispatcher = android.os.Handler(Looper.getMainLooper())
    private fun changed() { dispatcher.post { refresh?.invoke() } }
    var data: AudioMetadata? = null
      private set
    @Synchronized internal fun engine(context: Context): AudioEngine = shared ?: AudioEngine(
      context.getSystemService(android.media.AudioManager::class.java).getProperty(android.media.AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)?.toIntOrNull() ?: 48000
    ) { event, payload ->
      android.os.Handler(Looper.getMainLooper()).post {
        listener?.invoke(event, payload)
        if (event == "track" || event == "ended" || event == "starved") refresh?.invoke()
      }
    }.also { shared = it }
    internal fun existing(): AudioEngine? = shared
    fun attach(context: Context, callback: (String, Map<String, Any?>) -> Unit) { engine(context); react = context as? com.facebook.react.bridge.ReactContext; listener = callback }
    fun detach() { listener = null }
    fun metadata(next: AudioMetadata) { data = next; changed() }
    fun command(action: String, time: Double? = null) {
      if (action == "pause") pause()
      listener?.invoke("command", mapOf("action" to action, "time" to time))
    }
    fun pause() { requested = false; shared?.pause(); changed() }
    fun stop() {
      requested = false; shared?.stop(); data = null; changed()
      dispatcher.post { react?.let { bridge -> task?.let { com.facebook.react.jstasks.HeadlessJsTaskContext.getInstance(bridge).finishTask(it) } }; task = null }
    }
    fun start(context: Context) {
      requested = true
      context.startForegroundService(Intent(context, AudioService::class.java))
      dispatcher.post { react?.let { bridge ->
        if (task == null) task = com.facebook.react.jstasks.HeadlessJsTaskContext.getInstance(bridge).startTask(
          com.facebook.react.jstasks.HeadlessJsTaskConfig("TinystreamMusic", com.facebook.react.bridge.Arguments.createMap(), 0, true))
      } }
    }
  }
}

@UnstableApi
private class SessionPlayer(private val audio: AudioEngine): SimpleBasePlayer(Looper.getMainLooper()) {
  fun refresh() = invalidateState()
  override fun getState(): State {
    val commands=Player.Commands.Builder().addAll(Player.COMMAND_PLAY_PAUSE,Player.COMMAND_STOP,Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
      Player.COMMAND_SEEK_TO_MEDIA_ITEM,Player.COMMAND_GET_TIMELINE,Player.COMMAND_GET_METADATA,Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,Player.COMMAND_SEEK_TO_NEXT,
      Player.COMMAND_SEEK_TO_PREVIOUS,Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
      Player.COMMAND_SET_REPEAT_MODE,Player.COMMAND_SET_SHUFFLE_MODE).build()
    val data=data
    val state=State.Builder().setAvailableCommands(commands).setPlayWhenReady(audio.playing,Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
      .setPlaybackState(if(data==null) Player.STATE_IDLE else if(false) Player.STATE_BUFFERING else Player.STATE_READY)
      .setRepeatMode(data?.repeat ?: Player.REPEAT_MODE_OFF).setShuffleModeEnabled(data?.shuffled ?: false)
    if(data!=null) {
      val metadata=MediaMetadata.Builder().setTitle(data.title).setArtist(data.artist).setAlbumTitle(data.album)
        .setArtworkUri(data.artwork?.let(Uri::parse)).build()
      val queue = data.queue.ifEmpty { listOf(AudioQueueItem().apply { key=data.key; title=data.title; artist=data.artist; album=data.album; duration=data.duration }) }
      val playlist = queue.map { entry ->
        val info = if(entry.key==data.key) metadata else MediaMetadata.Builder().setTitle(entry.title).setArtist(entry.artist).setAlbumTitle(entry.album).build()
        val item=MediaItem.Builder().setMediaId(entry.key).setMediaMetadata(info).build()
        MediaItemData.Builder(entry.key).setMediaItem(item).setDurationUs((entry.duration*1_000_000).toLong()).setIsSeekable(true).build()
      }
      state.setPlaylist(playlist).setCurrentMediaItemIndex(data.index.coerceIn(0,playlist.lastIndex)).setContentPositionMs { ((audio.position?.time ?: 0.0)*1000).toLong() }
    }
    return state.build()
  }
  override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
    command(if(playWhenReady) "play" else "pause"); return Futures.immediateVoidFuture()
  }
  override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
    when(seekCommand) {
      Player.COMMAND_SEEK_TO_NEXT,Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> command("next")
      Player.COMMAND_SEEK_TO_PREVIOUS,Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> command("previous")
      else -> if(mediaItemIndex != currentMediaItemIndex) command("jump",mediaItemIndex.toDouble()) else command("seek",positionMs.coerceAtLeast(0)/1000.0)
    }
    return Futures.immediateVoidFuture()
  }
  override fun handleStop(): ListenableFuture<*> { command("stop"); return Futures.immediateVoidFuture() }
  override fun handleSetRepeatMode(repeatMode: Int): ListenableFuture<*> { command("repeat",repeatMode.toDouble()); return Futures.immediateVoidFuture() }
  override fun handleSetShuffleModeEnabled(shuffleModeEnabled: Boolean): ListenableFuture<*> { command("shuffle"); return Futures.immediateVoidFuture() }
}
