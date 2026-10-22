// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.notify

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import expo.modules.kotlin.exception.Exceptions
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.records.Field
import expo.modules.kotlin.records.Record

private const val CHANNEL = "notifications"

class Notice : Record {
  @Field val id: Int = 0
  @Field val title: String = ""
  @Field val body: String? = null
  @Field val link: String? = null
}

/**
 * Local notifications only: posted by the app itself, never pushed, so
 * there's nothing from Google in here. Each server's are grouped together,
 * and tapping one opens `tinystream://open` with the server and the link.
 */
class NotifyModule : Module() {
  private val context: Context
    get() = appContext.reactContext ?: throw Exceptions.ReactContextLost()

  private fun channel() {
    val manager = context.getSystemService(NotificationManager::class.java)
    if (manager.getNotificationChannel(CHANNEL) != null) return
    manager.createNotificationChannel(
      NotificationChannel(CHANNEL, "From your servers", NotificationManager.IMPORTANCE_DEFAULT).apply {
        description = "New episodes, requests and invitations"
      },
    )
  }

  private fun open(server: String, link: String?): PendingIntent {
    val uri = Uri.Builder().scheme("tinystream").authority("open").appendQueryParameter("server", server)
    if (link != null) uri.appendQueryParameter("link", link)
    val intent = Intent(Intent.ACTION_VIEW, uri.build()).setPackage(context.packageName).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    return PendingIntent.getActivity(context, uri.build().hashCode(), intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
  }

  private fun post(server: String, serverName: String, notices: List<Notice>) {
    val manager = NotificationManagerCompat.from(context)
    if (!manager.areNotificationsEnabled() || notices.isEmpty()) return
    channel()
    for (n in notices) {
      val notification = NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(R.drawable.ic_stat_tinystream)
        .setContentTitle(n.title)
        .setContentText(n.body)
        .setStyle(NotificationCompat.BigTextStyle().bigText(n.body))
        .setSubText(serverName)
        .setGroup(server)
        .setAutoCancel(true)
        .setContentIntent(open(server, n.link))
        .build()
      manager.notify("$server:${n.id}", 0, notification)
    }
    // The group's summary, which is what shows once there are several.
    val summary = NotificationCompat.Builder(context, CHANNEL)
      .setSmallIcon(R.drawable.ic_stat_tinystream)
      .setSubText(serverName)
      .setGroup(server)
      .setGroupSummary(true)
      .setAutoCancel(true)
      .setContentIntent(open(server, null))
      .build()
    manager.notify(server, 0, summary)
  }

  override fun definition() = ModuleDefinition {
    Name("TinystreamNotify")

    Function("enabled") { NotificationManagerCompat.from(context).areNotificationsEnabled() }

    Function("post") { server: String, serverName: String, notices: List<Notice> ->
      try {
        post(server, serverName, notices)
      } catch (e: SecurityException) {
        // Not allowed to post (anymore): nothing to do but not post.
      }
    }

    Function("openSettings") {
      val intent = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
        .putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      context.startActivity(intent)
    }
  }
}
