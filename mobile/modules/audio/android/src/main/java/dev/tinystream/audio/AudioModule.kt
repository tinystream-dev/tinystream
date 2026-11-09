// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.records.Field
import expo.modules.kotlin.records.Record

class AudioInput : Record {
  @Field var key: String = ""
  @Field var url: String = ""
  @Field var size: Long = 0
  @Field var ext: String = ""
  @Field var gain: Double = 1.0
  @Field var crossfade: Double = 0.0
  @Field var headers: Map<String, String> = emptyMap()
}
class AudioQueueItem : Record {
  @Field var key: String = ""
  @Field var title: String = ""
  @Field var artist: String = ""
  @Field var album: String = ""
  @Field var duration: Double = 0.0
}
class AudioMetadata : Record {
  @Field var queue: List<AudioQueueItem> = emptyList()
  @Field var index: Int = 0
  @Field var key: String = ""
  @Field var title: String = ""
  @Field var artist: String = ""
  @Field var album: String = ""
  @Field var artwork: String? = null
  @Field var headers: Map<String, String> = emptyMap()
  @Field var duration: Double = 0.0
  @Field var starred: Boolean = false
  @Field var shuffled: Boolean = false
  @Field var repeat: Int = 0
}
class AudioModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("TinystreamAudio")
    Events("track", "ended", "error", "starved", "position", "command")
    OnCreate {
      AudioService.attach(requireNotNull(appContext.reactContext)) { event, data -> sendEvent(event, data) }
    }
    AsyncFunction("load") { items: List<AudioInput>, start: Double, promise: expo.modules.kotlin.Promise ->
      AudioService.engine(requireNotNull(appContext.reactContext)).load(items.map { AudioItem(it.key, it.url, it.size, it.ext, it.gain.toFloat(), it.crossfade, it.headers) }, start) { error ->
        if (error == null) promise.resolve(null) else promise.reject("ERR_AUDIO_LOAD", "Could not start audio playback", error)
      }
    }
    AsyncFunction("play") { AudioService.start(requireNotNull(appContext.reactContext)) }
    Function("pause") { AudioService.pause() }
    Function("stop") { AudioService.stop() }
    Function("upcoming") { after: String, items: List<AudioInput> -> AudioService.existing()?.upcoming(after, items.map { AudioItem(it.key, it.url, it.size, it.ext, it.gain.toFloat(), it.crossfade, it.headers) }) }
    Function("gain") { key: String, gain: Double -> AudioService.existing()?.gain(key, gain.toFloat()) }
    Function("volume") { volume: Double -> AudioService.existing()?.volume(volume.toFloat()) }
    Function("metadata") { data: AudioMetadata -> AudioService.metadata(data) }
    Function("rate") { AudioService.engine(requireNotNull(appContext.reactContext)).rate }
    OnDestroy { AudioService.detach() }
  }
}
