// SPDX-License-Identifier: AGPL-3.0-or-later

package dev.tinystream.haptics

import android.content.Context
import android.os.SystemClock
import android.os.VibrationAttributes
import android.os.VibrationEffect
import android.os.VibrationEffect.Composition
import android.os.Vibrator
import android.os.VibratorManager
import android.view.HapticFeedbackConstants
import expo.modules.kotlin.exception.Exceptions
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/** A primitive at a scale (0–1), after a delay in milliseconds. */
private data class Step(val primitive: Int, val scale: Float, val delay: Int = 0)

/**
 * What an event feels like: primitives where the vibrator has them, else one
 * of the system's own feedback constants (or nothing).
 */
private class Feel(val steps: List<Step>, val fallback: Int?) {
  constructor(vararg steps: Step, fallback: Int?) : this(steps.toList(), fallback)
}

private val FEELS = mapOf(
  "tick" to Feel(Step(Composition.PRIMITIVE_TICK, 0.5f), fallback = HapticFeedbackConstants.SEGMENT_TICK),
  "edge" to Feel(Step(Composition.PRIMITIVE_THUD, 0.7f), fallback = HapticFeedbackConstants.GESTURE_THRESHOLD_ACTIVATE),
  "press" to Feel(Step(Composition.PRIMITIVE_CLICK, 0.6f), fallback = HapticFeedbackConstants.VIRTUAL_KEY),
  "longPressStart" to Feel(Step(Composition.PRIMITIVE_SLOW_RISE, 0.35f), fallback = HapticFeedbackConstants.GESTURE_START),
  "longPressOpen" to Feel(
    Step(Composition.PRIMITIVE_QUICK_RISE, 0.5f),
    Step(Composition.PRIMITIVE_CLICK, 1f, 30),
    fallback = HapticFeedbackConstants.LONG_PRESS,
  ),
  "toggleOn" to Feel(
    Step(Composition.PRIMITIVE_QUICK_RISE, 0.35f),
    Step(Composition.PRIMITIVE_CLICK, 0.8f, 20),
    fallback = HapticFeedbackConstants.TOGGLE_ON,
  ),
  "toggleOff" to Feel(
    Step(Composition.PRIMITIVE_QUICK_FALL, 0.35f),
    Step(Composition.PRIMITIVE_LOW_TICK, 0.8f, 20),
    fallback = HapticFeedbackConstants.TOGGLE_OFF,
  ),
  "pullProgress" to Feel(Step(Composition.PRIMITIVE_LOW_TICK, 0.15f), fallback = HapticFeedbackConstants.SEGMENT_FREQUENT_TICK),
  "pullTrigger" to Feel(
    Step(Composition.PRIMITIVE_THUD, 0.6f),
    Step(Composition.PRIMITIVE_CLICK, 0.9f, 40),
    fallback = HapticFeedbackConstants.CONFIRM,
  ),
  "dismissThreshold" to Feel(
    Step(Composition.PRIMITIVE_SPIN, 0.4f),
    Step(Composition.PRIMITIVE_CLICK, 0.7f, 10),
    fallback = HapticFeedbackConstants.GESTURE_THRESHOLD_ACTIVATE,
  ),
  "tab" to Feel(Step(Composition.PRIMITIVE_CLICK, 0.45f), fallback = HapticFeedbackConstants.CLOCK_TICK),
  "reveal" to Feel(Step(Composition.PRIMITIVE_SLOW_RISE, 0.25f), fallback = null),
  "texture" to Feel(Step(Composition.PRIMITIVE_LOW_TICK, 0.08f), fallback = null),
  "success" to Feel(
    Step(Composition.PRIMITIVE_CLICK, 0.6f),
    Step(Composition.PRIMITIVE_CLICK, 1f, 80),
    fallback = HapticFeedbackConstants.CONFIRM,
  ),
  "error" to Feel(
    Step(Composition.PRIMITIVE_THUD, 0.8f),
    Step(Composition.PRIMITIVE_THUD, 0.6f, 90),
    fallback = HapticFeedbackConstants.REJECT,
  ),
)

private val INTENSITIES = listOf("off", "subtle", "full")

/** The finger moves far more often than anyone could feel the difference. */
private const val TEXTURE_GAP_MS = 45L

class HapticsModule : Module() {
  private val context: Context
    get() = appContext.reactContext ?: throw Exceptions.ReactContextLost()

  private val vibrator: Vibrator by lazy {
    context.getSystemService(VibratorManager::class.java).defaultVibrator
  }

  private val prefs by lazy { context.getSharedPreferences("tinystream.haptics", Context.MODE_PRIVATE) }

  private var intensity: String? = null
  private val supported = mutableMapOf<String, Boolean>()
  private var lastTexture = 0L

  private val touch = VibrationAttributes.createForUsage(VibrationAttributes.USAGE_TOUCH)

  private fun intensity() = intensity ?: (prefs.getString("intensity", null) ?: "full").also { intensity = it }

  private fun play(event: String, amount: Double?) {
    val feel = FEELS[event] ?: throw IllegalArgumentException("no haptic event called $event")
    val level = intensity()
    if (level == "off" || (level == "subtle" && event == "texture")) return
    if (event == "texture") {
      val now = SystemClock.uptimeMillis()
      if (now - lastTexture < TEXTURE_GAP_MS) return
      lastTexture = now
    }
    val composable = supported.getOrPut(event) {
      vibrator.areAllPrimitivesSupported(*feel.steps.map { it.primitive }.toIntArray())
    }
    if (composable) {
      // The amount (0–1, e.g. how far a pull has come) adds to the first step.
      val boost = amount?.coerceIn(0.0, 1.0)?.toFloat()?.times(0.5f) ?: 0f
      val factor = if (level == "subtle") 0.5f else 1f
      val composition = VibrationEffect.startComposition()
      feel.steps.forEachIndexed { i, step ->
        val scale = ((step.scale + if (i == 0) boost else 0f) * factor).coerceIn(0f, 1f)
        composition.addPrimitive(step.primitive, scale, step.delay)
      }
      vibrator.vibrate(composition.compose(), touch)
      return
    }
    val constant = feel.fallback ?: return
    val view = appContext.currentActivity?.window?.decorView ?: return
    view.post { view.performHapticFeedback(constant) }
  }

  override fun definition() = ModuleDefinition {
    Name("TinystreamHaptics")

    Function("play") { event: String, amount: Double? -> play(event, amount) }

    Function("intensity") { intensity() }

    Function("setIntensity") { level: String ->
      require(level in INTENSITIES) { "haptic intensity is one of ${INTENSITIES.joinToString()}" }
      intensity = level
      prefs.edit().putString("intensity", level).apply()
    }
  }
}
