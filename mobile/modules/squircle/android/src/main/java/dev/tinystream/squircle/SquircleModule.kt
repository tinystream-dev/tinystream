// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.squircle

import android.graphics.Color
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class SquircleModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("TinystreamSquircle")

    View(SquircleView::class) {
      Prop("radius") { view: SquircleView, radius: Float -> view.setRadius(radius) }
      Prop("smoothing") { view: SquircleView, smoothing: Float -> view.setSmoothing(smoothing) }
      Prop("edge") { view: SquircleView, colors: List<Color>? -> view.setEdge(colors) }
      Prop("dashed") { view: SquircleView, color: Color? -> view.setDashed(color) }
    }
  }
}
