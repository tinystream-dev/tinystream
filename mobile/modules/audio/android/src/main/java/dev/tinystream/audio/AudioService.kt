// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

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
  override fun onCreate() {
    super.onCreate()
    setMediaNotificationProvider(DefaultMediaNotificationProvider.Builder(this).setNotificationId(740).build())
    audio=engine(this)
    player=SessionPlayer(audio)
    session=MediaSession.Builder(this,player)
      .setBitmapLoader(DataSourceBitmapLoader.Builder(this).setDataSourceFactory(DataSource.Factory {
        DefaultHttpDataSource.Factory().setDefaultRequestProperties(audio.data?.headers ?: emptyMap()).createDataSource()
      }).build())
      .setCallback(object: MediaSession.Callback {
      override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
        val commands=MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
        for (action in listOf("star","shuffle","repeat")) commands.add(SessionCommand(action,Bundle.EMPTY))
        return MediaSession.ConnectionResult.AcceptedResultBuilder(session).setAvailableSessionCommands(commands.build()).build()
      }
      override fun onCustomCommand(session: MediaSession, controller: MediaSession.ControllerInfo, command: SessionCommand, args: Bundle): ListenableFuture<SessionResult> {
        audio.command(command.customAction)
        return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
      }
    }).build()
    addSession(session!!)
    audio.changed = {
      session?.setCustomLayout(listOf(
        CommandButton.Builder().setDisplayName(if(audio.data?.starred==true) "Unstar" else "Star").setIconResId(android.R.drawable.btn_star_big_off).setSessionCommand(SessionCommand("star",Bundle.EMPTY)).build(),
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
      Notification.Builder(this,"tinystream.music.start").setSmallIcon(android.R.drawable.ic_media_play).setContentTitle(audio.data?.title ?: "tinystream").setContentText("Starting music").build()
    }
    startForeground(740,notification)
    audio.foregroundReady()
    super.onStartCommand(intent,flags,startId)
    return START_NOT_STICKY
  }
  override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session
  override fun onTaskRemoved(rootIntent: Intent?) { if (audio.data == null) stopSelf() }
  override fun onDestroy() {
    audio.changed={}
    session?.release(); player.release()

    super.onDestroy()
  }
  companion object {
    private var shared: AudioEngine? = null
    @Synchronized fun engine(context: Context): AudioEngine = shared ?: AudioEngine(context.applicationContext).also {shared=it}
    fun existing(): AudioEngine? = shared
    fun start(context: Context) { context.startForegroundService(Intent(context,AudioService::class.java)) }
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
    val data=audio.data
    val state=State.Builder().setAvailableCommands(commands).setPlayWhenReady(audio.playing,Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
      .setPlaybackState(if(data==null) Player.STATE_IDLE else if(audio.waiting) Player.STATE_BUFFERING else Player.STATE_READY)
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
      state.setPlaylist(playlist).setCurrentMediaItemIndex(data.index.coerceIn(0,playlist.lastIndex)).setContentPositionMs { (audio.position*1000).toLong() }
    }
    return state.build()
  }
  override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
    audio.command(if(playWhenReady) "play" else "pause"); return Futures.immediateVoidFuture()
  }
  override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
    when(seekCommand) {
      Player.COMMAND_SEEK_TO_NEXT,Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> audio.command("next")
      Player.COMMAND_SEEK_TO_PREVIOUS,Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> audio.command("previous")
      else -> if(mediaItemIndex != currentMediaItemIndex) audio.command("jump",mediaItemIndex.toDouble()) else audio.command("seek",positionMs.coerceAtLeast(0)/1000.0)
    }
    return Futures.immediateVoidFuture()
  }
  override fun handleStop(): ListenableFuture<*> { audio.command("stop"); return Futures.immediateVoidFuture() }
  override fun handleSetRepeatMode(repeatMode: Int): ListenableFuture<*> { audio.command("repeat",repeatMode.toDouble()); return Futures.immediateVoidFuture() }
  override fun handleSetShuffleModeEnabled(shuffleModeEnabled: Boolean): ListenableFuture<*> { audio.command("shuffle"); return Futures.immediateVoidFuture() }
}
