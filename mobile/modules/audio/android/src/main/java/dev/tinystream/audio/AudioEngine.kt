// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import java.util.concurrent.Executors
import com.facebook.react.bridge.ReactContext
import com.facebook.react.bridge.Arguments
import com.facebook.react.jstasks.HeadlessJsTaskConfig
import com.facebook.react.jstasks.HeadlessJsTaskContext
import uniffi.tinystream_decoder_android.Event
import uniffi.tinystream_decoder_android.Item
import uniffi.tinystream_decoder_android.Sequencer

class AudioEngine(private val context: Context) {
  val rate = (context.getSystemService(Context.AUDIO_SERVICE) as AudioManager).getProperty(AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)?.toIntOrNull() ?: 48000
  private val worker = Executors.newSingleThreadExecutor()
  private val main = Handler(Looper.getMainLooper())
  private var decoder = Sequencer(rate.toUInt())
  private var output: AudioTrack? = null
  private var pending = FloatArray(0)
  private var pendingAt = 0
  private val marks = mutableListOf<Event.Mark>()
  private var mark: Event.Mark? = null
  private var end: Long? = null
  private var lastHead = 0L
  private var wraps = 0L
  private var written = 0L
  private var inputs: List<AudioItem> = emptyList()
  private var generation = 0L
  @Volatile var playing = false
    private set
  @Volatile var position = 0.0
    private set
  @Volatile var key: String? = null
    private set
  @Volatile var waiting = false
    private set
  @Volatile var duration = 0.0
    private set
  @Volatile var data: AudioMetadata? = null
    private set
  var changed: () -> Unit = {}
  @Volatile var emit: (String, Map<String, Any?>) -> Unit = { _, _ -> }
  private var level = 1f
  private var duck = 1f
  private var resumeOnFocus = false
  @Volatile private var playRequested = false
  private var react: ReactContext? = null
  private var task: Int? = null
  fun attach(context: ReactContext) { react = context }
  private fun keepController() {
    val bridge = react ?: return
    if (task == null) task = HeadlessJsTaskContext.getInstance(bridge).startTask(HeadlessJsTaskConfig("TinystreamMusic", Arguments.createMap(), 0, true))
  }
  private fun releaseController() {
    val bridge = react ?: return
    task?.let { HeadlessJsTaskContext.getInstance(bridge).finishTask(it) }; task = null
  }
  private var closed = false
  private var scheduled = false
  private val manager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
  private val wake = (context.getSystemService(Context.POWER_SERVICE) as PowerManager).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK,"tinystream:music")
  private val focus = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
    .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build())
    .setOnAudioFocusChangeListener({ change ->
      when (change) {
        AudioManager.AUDIOFOCUS_LOSS -> { resumeOnFocus = false; command("pause") }
        AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> { resumeOnFocus = playing; pause(false); command("focusPause") }
        AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> { duck = 0.2f; volume(level) }
        AudioManager.AUDIOFOCUS_GAIN -> { duck = 1f; volume(level); if (resumeOnFocus) { resumeOnFocus = false; command("play") } }
      }
    }, main).build()
  private val noisy = object : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) { resumeOnFocus = false; command("pause") }
  }
  init { context.registerReceiver(noisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), Context.RECEIVER_NOT_EXPORTED) }
  fun command(action: String, time: Double? = null) {
    if (action == "pause") pause()
    emit("command", mapOf("action" to action, "time" to time))
  }
  private fun notifyChanged() = main.post { changed() }
  private fun native(items: List<AudioItem>) = items.map { Item(it.key, Source(it.url,it.size,it.headers),it.ext,it.gain.toFloat(),it.crossfade) }
  fun load(items: List<AudioItem>, start: Double) {
    worker.submit { loadNow(items,start) }.get()
  }
  private fun loadNow(items: List<AudioItem>, start: Double) {
      inputs=items
      generation++
      output?.pause(); output?.flush(); output?.release()
      output = AudioTrack.Builder()
        .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build())
        .setAudioFormat(AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_FLOAT).setSampleRate(rate).setChannelMask(AudioFormat.CHANNEL_OUT_STEREO).build())
        .setTransferMode(AudioTrack.MODE_STREAM).setBufferSizeInBytes(maxOf(rate*8/3,AudioTrack.getMinBufferSize(rate,AudioFormat.CHANNEL_OUT_STEREO,AudioFormat.ENCODING_PCM_FLOAT))).build()
      output!!.setVolume(level*duck)
      marks.clear(); mark=null; end=null; pending=FloatArray(0); pendingAt=0; written=0; lastHead=0; wraps=0
      key=null; position=start; duration=0.0; waiting=true
      decoder.play(native(items),start)
      if (playing) output!!.play()
      schedule()
  }
  fun resume() {
    playRequested = true
    AudioService.start(context)
  }
  fun foregroundReady() {
    if (!playRequested) return
    if (manager.requestAudioFocus(focus) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) { command("pause"); return }
    keepController()
    playing = true
    if (!wake.isHeld) wake.acquire()
    worker.execute { output?.play(); schedule() }
    notifyChanged()
  }
  fun pause(abandon: Boolean = true) {
    playing = false
    playRequested = false
    if (abandon) { resumeOnFocus=false; manager.abandonAudioFocusRequest(focus) }
    if (wake.isHeld) wake.release()
    worker.execute { output?.pause(); progress() }
    notifyChanged()
  }
  fun stop() {
    pause()
    worker.execute { generation++; decoder.stop(); output?.flush(); output?.release(); output=null; marks.clear(); mark=null; pending=FloatArray(0); end=null; key=null; position=0.0; data=null; notifyChanged(); main.post { releaseController(); context.stopService(Intent(context,AudioService::class.java)) } }
  }
  fun upcoming(after: String, items: List<AudioItem>) { worker.execute {
    val at=inputs.indexOfFirst { it.key==after }
    if(at<0) return@execute
    val old=inputs.drop(at+1)
    inputs=inputs.take(at+1)+items
    end=null
    val rewind=decoder.upcoming(after,native(items),head().toULong())
    if(rewind>=0) {
      if(rewind<written) {
        progress()
        val audible=inputs.firstOrNull {it.key==key} ?: inputs[at]
        loadNow(listOf(audible)+items,position)
      } else {
        pending=pending.copyOf((pendingAt+(rewind-written)*2).toInt().coerceIn(pendingAt,pending.size))
        marks.removeAll {it.frame.toLong()>=rewind}
      }
    } else if(old.map {it.key to it.ext} != items.map {it.key to it.ext} && key==after && marks.any {it.key!=after && it.frame.toLong()<written}) {
      progress(); loadNow(listOf(inputs[at])+items,position)
    }
    schedule()
  } }
  fun gain(key: String, gain: Float) { worker.execute { decoder.gain(key,gain) } }
  fun volume(value: Float) { level=value.coerceIn(0f,1f); worker.execute { output?.setVolume(level*duck) } }
  fun metadata(next: AudioMetadata) { data=next; notifyChanged() }
  private fun head(): Long {
    val current=output?.playbackHeadPosition?.toLong()?.and(0xffffffffL) ?: 0L
    if (current<lastHead) wraps+=1L shl 32
    lastHead=current
    return wraps+current
  }
  private fun progress() {
    val frame=head()
    while (marks.isNotEmpty() && marks.first().frame.toLong() <= frame) {
      mark=marks.removeAt(0); key=mark!!.key; duration=mark!!.duration
      emit("track",mapOf("key" to key)); notifyChanged()
    }
    mark?.let { position=(it.offset+(frame-it.frame.toLong()).coerceAtLeast(0).toDouble()/rate).coerceAtMost(it.duration.takeIf { n->n>0 } ?: Double.MAX_VALUE) }
    emit("position",mapOf("key" to key,"time" to position,"playing" to playing))
    end?.let { if (frame>=it && pending.isEmpty()) { end=null; playing=false; if(wake.isHeld)wake.release(); manager.abandonAudioFocusRequest(focus); emit("ended",emptyMap()); notifyChanged() } }
  }
  private fun schedule() {
    if (scheduled || closed) return
    scheduled=true
    worker.execute { pump() }
  }
  private fun pump() {
    scheduled=false
    val audio=output ?: return
    progress()
    if (pendingAt>=pending.size && end==null && written-head()<rate/3) {
      val chunk=decoder.read(4096u)
      pending=chunk.samples.toFloatArray(); pendingAt=0
      for(event in chunk.events) when(event) {
        is Event.Mark -> marks.add(event)
        is Event.Rewind -> { marks.removeAll {it.frame>=event.frame}; end=null }
        is Event.End -> end=event.frame.toLong()
        is Event.Error -> emit("error",mapOf("key" to event.key,"message" to event.message))
      }
    }
    if (pendingAt<pending.size) {
      val n=audio.write(pending,pendingAt,pending.size-pendingAt,AudioTrack.WRITE_NON_BLOCKING)
      if (n<0) { emit("error",mapOf("key" to key,"message" to "audio output failed ($n)")); pause(); return }
      pendingAt+=n; written+=n/2
      if(pendingAt>=pending.size) { pending=FloatArray(0); pendingAt=0 }
    }
    val starved=playing && end==null && written<=head()
    if (waiting!=starved) { waiting=starved; emit("starved",mapOf("waiting" to starved)); notifyChanged() }
    if (playing) main.postDelayed({worker.execute {schedule()}},20)
  }
  fun close() {
    closed=true; pause(); context.unregisterReceiver(noisy)
    worker.execute { decoder.stop(); decoder.close(); output?.release() }
    worker.shutdown()
  }
}
