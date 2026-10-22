// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.squircle

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.DashPathEffect
import android.graphics.LinearGradient
import android.graphics.Matrix
import android.graphics.Outline
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Shader
import android.view.View
import android.view.ViewOutlineProvider
import androidx.core.graphics.PathParser
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.views.ExpoView
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.math.tan

/**
 * Clips itself and everything in it to a squircle (@tinystream/shared/squircle's
 * path, built here at its own size so it's there from the first frame), and
 * draws its edge: a hairline brighter at the top, or dashed for placeholders.
 */
class SquircleView(context: Context, appContext: AppContext) : ExpoView(context, appContext) {
  private val density = context.resources.displayMetrics.density
  private var path: Path? = null
  private var radius = 0f
  private var smoothing = 0.6f
  private var edge: IntArray? = null
  private var dashed: Int? = null
  private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
    style = Paint.Style.STROKE
    // Twice the width, half of it clipped away: a crisp 1dp inner edge.
    strokeWidth = 2 * density
  }

  init {
    clipToOutline = true
    outlineProvider = object : ViewOutlineProvider() {
      override fun getOutline(view: View, outline: Outline) {
        val p = path
        if (p != null) outline.setPath(p) else outline.setRoundRect(0, 0, view.width, view.height, radius * density)
      }
    }
  }

  // React Native measures and lays out the children; the LinearLayout underneath mustn't redo it.
  override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) =
    setMeasuredDimension(MeasureSpec.getSize(widthMeasureSpec), MeasureSpec.getSize(heightMeasureSpec))

  override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) = Unit

  override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
    super.onSizeChanged(w, h, oldw, oldh)
    rebuild()
  }

  override fun dispatchDraw(canvas: Canvas) {
    super.dispatchDraw(canvas)
    val p = path ?: return
    if (edge != null || dashed != null) canvas.drawPath(p, paint)
  }

  fun setRadius(value: Float) {
    radius = value
    rebuild()
  }

  fun setSmoothing(value: Float) {
    smoothing = value
    rebuild()
  }

  fun setEdge(colors: List<Color>?) {
    edge = colors?.takeIf { it.size == 3 }?.map { it.toArgb() }?.toIntArray()
    restyle()
  }

  fun setDashed(color: Color?) {
    dashed = color?.toArgb()
    restyle()
  }

  /** The edge's paint: the dashes, or the gradient down the box's height. */
  private fun restyle() {
    val colors = edge
    val dash = dashed
    paint.pathEffect = dash?.let { DashPathEffect(floatArrayOf(4 * density, 4 * density), 0f) }
    paint.shader = if (dash == null && colors != null) LinearGradient(0f, 0f, 0f, height.toFloat(), colors, floatArrayOf(0f, 0.35f, 1f), Shader.TileMode.CLAMP) else null
    paint.color = dash ?: Color.BLACK
    setWillNotDraw(colors == null && dash == null)
    invalidate()
  }

  private fun rebuild() {
    val w = width / density
    val h = height / density
    path = squirclePath(w, h, radius, smoothing)?.let { d ->
      PathParser.createPathFromPathData(d).apply { transform(Matrix().apply { setScale(density, density) }) }
    }
    invalidateOutline()
    restyle()
  }
}

private fun rad(deg: Double) = deg * PI / 180

private fun f(n: Double): String {
  val r = (n * 1000).roundToInt() / 1000.0
  return if (r == r.toLong().toDouble()) r.toLong().toString() else r.toString()
}

/** Rotates a vector 90° clockwise `turns` times (SVG's y axis points down). */
private fun rot(vx: Double, vy: Double, turns: Int): Pair<Double, Double> {
  var x = vx
  var y = vy
  repeat(turns) {
    val t = x
    x = -y
    y = t
  }
  return Pair(x, y)
}

/** @tinystream/shared/squircle's `squirclePath`, in dp; null for an empty box. */
internal fun squirclePath(w: Float, h: Float, radius: Float, smoothing: Float): String? {
  if (w <= 0 || h <= 0) return null
  val maxP = min(w, h) / 2.0
  val r = min(radius.toDouble(), maxP)
  if (r <= 0) return "M0 0H${f(w.toDouble())}V${f(h.toDouble())}H0Z"
  var s = smoothing.toDouble()
  var p = (1 + s) * r
  if (p > maxP) {
    s = max(0.0, min(s, maxP / r - 1))
    p = min(p, maxP)
  }
  val arcMeasure = 90 * (1 - s)
  val arcLen = sin(rad(arcMeasure / 2)) * r * sqrt(2.0)
  val alpha = (90 - arcMeasure) / 2
  val p3p4 = r * tan(rad(alpha / 2))
  val beta = 45 * s
  val c = p3p4 * cos(rad(beta))
  val d = c * tan(rad(beta))
  val b = (p - arcLen - c - d) / 3
  val a = 2 * b

  fun seg(turns: Int): String {
    val c1 = rot(a, 0.0, turns)
    val c2 = rot(a + b, 0.0, turns)
    val e1 = rot(a + b + c, d, turns)
    val arc = rot(arcLen, arcLen, turns)
    val c3 = rot(d, c, turns)
    val c4 = rot(d, b + c, turns)
    val e2 = rot(d, a + b + c, turns)
    return "c${f(c1.first)} ${f(c1.second)} ${f(c2.first)} ${f(c2.second)} ${f(e1.first)} ${f(e1.second)}" +
      "a${f(r)} ${f(r)} 0 0 1 ${f(arc.first)} ${f(arc.second)}" +
      "c${f(c3.first)} ${f(c3.second)} ${f(c4.first)} ${f(c4.second)} ${f(e2.first)} ${f(e2.second)}"
  }
  val W = w.toDouble()
  val H = h.toDouble()
  return "M${f(W - p)} 0" + seg(0) + "L${f(W)} ${f(H - p)}" + seg(1) + "L${f(p)} ${f(H)}" + seg(2) + "L0 ${f(p)}" + seg(3) + "Z"
}
