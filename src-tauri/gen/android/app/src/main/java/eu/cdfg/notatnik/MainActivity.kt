package eu.cdfg.notatnik

import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Edge-to-edge puts the page under the status bar, the navigation bar and a camera cut-out. The web view does not
    // always report those to CSS (`env(safe-area-inset-*)`), so the content view is padded by them here instead and the
    // insets are consumed (PL-3, PL-5).
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      // The on-screen keyboard (IME) is an inset as well: with it in the bottom padding the page gets shorter while
      // the keyboard is up and the web view scrolls the field that has the focus into view. Without it the keyboard
      // covers the lower part of the page and the text typed into a field there cannot be seen.
      val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, keyboard.bottom))
      WindowInsetsCompat.CONSUMED
    }
  }
}
