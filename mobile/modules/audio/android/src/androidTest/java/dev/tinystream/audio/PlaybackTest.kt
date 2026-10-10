// SPDX-License-Identifier: AGPL-3.0-or-later
package dev.tinystream.audio

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.tinystream_decoder_android.Sequencer
import uniffi.tinystream_decoder_android.Item

@RunWith(AndroidJUnit4::class)
class PlaybackTest {
  private val instrumentation = InstrumentationRegistry.getInstrumentation()
  private val context get() = instrumentation.targetContext
  private fun fixture(ext: String): File = File(context.cacheDir,"tone.$ext").also { file ->
    instrumentation.context.assets.open("tone.$ext").use { source -> file.outputStream().use(source::copyTo) }
  }
  private fun screen(key: Int) {
    val pipe=instrumentation.uiAutomation.executeShellCommand("input keyevent $key")
    java.io.FileInputStream(pipe.fileDescriptor).use { it.readBytes() }
    pipe.close()
  }
  @Test fun nativeCodecsTrimEncoderPadding() {
    for(ext in listOf("mp3","opus")) {
      val file=fixture(ext)
      Sequencer(48000u).use { decoder ->
        decoder.play(listOf(Item(ext,Source(file.toURI().toString(),file.length(),emptyMap()),ext,1f,0.0)),0.0)
        var frames=0
        do { val output=decoder.read(4096u); frames+=output.samples.size/2 } while(output.samples.isNotEmpty())
        assertEquals("$ext should produce precisely one second",48000,frames)
      }
    }
  }
  @Test fun audioTrackReportsAudibleTransitionsAndPauses() {
    val file=fixture("mp3")
    val audio=AudioService.engine(context)
    val first=CountDownLatch(1)
    val second=CountDownLatch(1)
    val ended=CountDownLatch(1)
    val errors=java.util.concurrent.CopyOnWriteArrayList<String>()
    audio.emit={ event,data ->
      if(event=="track" && data["key"]=="one") first.countDown()
      if(event=="track" && data["key"]=="two") second.countDown()
      if(event=="ended") ended.countDown()
      if(event=="error") errors.add(data.toString())
    }
    fun item(key: String)=AudioItem().apply { this.key=key; url=file.toURI().toString(); size=file.length(); ext="mp3" }
    try {
      audio.load(listOf(item("one"),item("two")),0.0)
      instrumentation.runOnMainSync {
        audio.metadata(AudioMetadata().apply { key="one"; title="Test tone"; duration=1.0 })
        audio.resume()
      }
      assertTrue("first audible mark",first.await(15,TimeUnit.SECONDS))
      Thread.sleep(200)
      instrumentation.runOnMainSync { audio.pause() }
      Thread.sleep(100)
      val paused=audio.position
      Thread.sleep(200)
      assertEquals("pause holds the playback head",paused,audio.position,0.03)
      screen(223)
      instrumentation.runOnMainSync { audio.resume() }
      assertTrue("second audible mark",second.await(15,TimeUnit.SECONDS))
      assertTrue("end after queued audio is heard",ended.await(15,TimeUnit.SECONDS))
      assertTrue(errors.toString(),errors.isEmpty())
    } finally { screen(224); instrumentation.runOnMainSync { audio.stop() } }
  }
  @Test fun notificationCommandsReachTheQueueController() {
    val file=fixture("mp3")
    val audio=AudioService.engine(context)
    val first=CountDownLatch(1)
    val next=CountDownLatch(1)
    val seek=CountDownLatch(1)
    audio.emit={ event,data ->
      if(event=="track") first.countDown()
      if(event=="command" && data["action"]=="next") next.countDown()
      if(event=="command" && data["action"]=="seek") seek.countDown()
    }
    val items=listOf("one","two").map { name -> AudioItem().apply { key=name; url=file.toURI().toString(); size=file.length(); ext="mp3" } }
    var controller: androidx.media3.session.MediaController?=null
    try {
      audio.load(items,0.0)
      instrumentation.runOnMainSync {
        audio.metadata(AudioMetadata().apply {
          key="one"; title="Test tone"; duration=1.0
          queue=listOf("one","two").map { name -> AudioQueueItem().apply { key=name; title=name; duration=1.0 } }
        })
        audio.resume()
      }
      assertTrue(first.await(15,TimeUnit.SECONDS))
      val notifications=context.getSystemService(android.app.NotificationManager::class.java)
      val deadline=System.nanoTime()+TimeUnit.SECONDS.toNanos(5)
      while(System.nanoTime()<deadline && notifications.activeNotifications.none {
        it.id==740 && it.notification.extras.containsKey(android.app.Notification.EXTRA_MEDIA_SESSION)
      }) Thread.sleep(50)
      assertTrue("startup notification is replaced by media controls",notifications.activeNotifications.any {
        it.id==740 && it.notification.extras.containsKey(android.app.Notification.EXTRA_MEDIA_SESSION)
      })
      instrumentation.runOnMainSync { audio.resume() }
      Thread.sleep(150)
      assertTrue("resuming an active service preserves media controls",notifications.activeNotifications.any {
        it.id==740 && it.notification.extras.containsKey(android.app.Notification.EXTRA_MEDIA_SESSION)
      })
      lateinit var future: com.google.common.util.concurrent.ListenableFuture<androidx.media3.session.MediaController>
      instrumentation.runOnMainSync {
        val token=androidx.media3.session.SessionToken(context,android.content.ComponentName(context,AudioService::class.java))
        future=androidx.media3.session.MediaController.Builder(context,token).buildAsync()
      }
      controller=future.get(15,TimeUnit.SECONDS)
      instrumentation.runOnMainSync {
        assertTrue(controller!!.isCommandAvailable(androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM))
        controller!!.seekToNextMediaItem()
      }
      assertTrue("next reaches JS",next.await(5,TimeUnit.SECONDS))
      instrumentation.runOnMainSync { controller!!.seekTo(500) }
      assertTrue("scrub reaches JS",seek.await(5,TimeUnit.SECONDS))
    } finally { instrumentation.runOnMainSync { controller?.release(); audio.stop() } }
  }

  @Test fun transientFocusLossResumes() {
    val file=fixture("mp3")
    val audio=AudioService.engine(context)
    val first=CountDownLatch(1)
    val lost=CountDownLatch(1)
    val regained=CountDownLatch(1)
    val started=CountDownLatch(1)
    audio.emit={ event,data ->
      if(event=="track") first.countDown()
      if(event=="command" && data["action"]=="focusPause") lost.countDown()
      if(event=="command" && data["action"]=="play") audio.resume()
      if(event=="position" && data["playing"]==true && lost.count>0L) started.countDown()
      if(event=="position" && data["playing"]==true && lost.count==0L) regained.countDown()
    }
    val manager=context.getSystemService(android.media.AudioManager::class.java)
    val interruption=android.media.AudioFocusRequest.Builder(android.media.AudioManager.AUDIOFOCUS_GAIN_TRANSIENT)
      .setAudioAttributes(android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_MEDIA).build())
      .setOnAudioFocusChangeListener({}).build()
    try {
      audio.load((1..20).map { n -> AudioItem().apply { key="$n"; url=file.toURI().toString(); size=file.length(); ext="mp3" } },0.0)
      instrumentation.runOnMainSync {
        audio.metadata(AudioMetadata().apply { key="1"; title="Focus test"; duration=1.0 })
        audio.resume()
      }
      assertTrue(first.await(15,TimeUnit.SECONDS))
      assertTrue("playback has acquired focus",started.await(15,TimeUnit.SECONDS))
      assertEquals(android.media.AudioManager.AUDIOFOCUS_REQUEST_GRANTED,manager.requestAudioFocus(interruption))
      assertTrue("transient loss reaches JS",lost.await(5,TimeUnit.SECONDS))
      assertFalse(audio.playing)
      manager.abandonAudioFocusRequest(interruption)
      assertTrue("focus regain resumes through JS",regained.await(5,TimeUnit.SECONDS))
    } finally {
      manager.abandonAudioFocusRequest(interruption)
      instrumentation.runOnMainSync { audio.stop() }
    }
  }

}
