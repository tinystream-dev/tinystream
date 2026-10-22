// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.about

import expo.modules.kotlin.exception.Exceptions
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import uniffi.tinystream_about.version

class AboutModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("TinystreamAbout")

    Function("version") { version() }

    // Written into the APK's assets by scripts/licenses.ts, for release builds only.
    AsyncFunction("licenses") {
      val context = appContext.reactContext ?: throw Exceptions.ReactContextLost()
      try {
        context.assets.open("THIRD_PARTY_LICENSES").bufferedReader().use { it.readText() }
      } catch (e: java.io.IOException) {
        null
      }
    }
  }
}
