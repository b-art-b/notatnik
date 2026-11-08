package eu.cdfg.notatnik

import android.app.Activity
import android.content.Intent
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * What the app does for the home-screen widgets (AN-5..AN-7). The Rust side (`src-tauri/src/widgets.rs`) implements its
 * `Widgets` trait with the three commands below: where the widget files are, redraw the widgets, and the link the app
 * was opened with (`notatnik://capture`, `notatnik://page/<name>`, `notatnik://agenda`), handed over once.
 */
@TauriPlugin
class WidgetsPlugin(private val activity: Activity) : Plugin(activity) {
  @Volatile private var link: String? = fromIntent(activity.intent)

  private fun fromIntent(intent: Intent?): String? =
    intent?.data?.takeIf { it.scheme == "notatnik" }?.toString()

  // the app was running and a widget (or `am start`) opened it again
  override fun onNewIntent(intent: Intent) {
    fromIntent(intent)?.let { link = it }
  }

  @Command
  fun directory(invoke: Invoke) {
    val dir = invoke.getArgs().getString("dir")
    if (dir == null) {
      invoke.reject("dir is needed")
      return
    }
    WidgetFiles.setDirectory(activity, dir)
    invoke.resolve()
  }

  @Command
  fun refresh(invoke: Invoke) {
    TasksWidgetProvider.refreshAll(activity)
    invoke.resolve()
  }

  @Command
  fun takeLink(invoke: Invoke) {
    val out = JSObject()
    out.put("value", link)
    link = null
    // the launch intent keeps its data after a configuration change; it must not be replayed
    activity.intent?.data = null
    invoke.resolve(out)
  }
}
