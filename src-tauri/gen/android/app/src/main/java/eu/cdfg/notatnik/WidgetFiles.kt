package eu.cdfg.notatnik

import android.content.Context
import java.io.File
import java.io.FileOutputStream

/**
 * Where the widget files are. The app tells the directory on every start (`WidgetsPlugin.directory`) and it is kept in
 * the preferences, so a widget finds the files without the app running. Only this file touches them.
 */
object WidgetFiles {
  private const val PREFS = "widgets"
  private const val KEY = "dir"

  fun setDirectory(context: Context, dir: String) {
    context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY, dir).apply()
  }

  private fun dir(context: Context): File? =
    context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, null)?.let(::File)

  fun snapshot(context: Context): WidgetData.Snapshot? {
    val d = dir(context) ?: return null
    return try {
      WidgetData.parseSnapshot(File(d, "today.json").readText())
    } catch (e: Exception) {
      null
    }
  }

  /** The graph the widget shows, for the line it queues; empty when the app has not written a snapshot yet. */
  fun graph(context: Context): String = snapshot(context)?.graph ?: ""

  /** Appends one line to the queue in a single write; false when the app has never told where the files are. */
  fun enqueue(context: Context, text: String): Boolean {
    val d = dir(context) ?: return false
    return try {
      d.mkdirs()
      // O_APPEND: the whole line is written at once, so the app, reading at the same moment, never sees half of it
      FileOutputStream(File(d, "capture-queue.jsonl"), true).use {
        it.write(WidgetData.queueLine(text, graph(context)).toByteArray(Charsets.UTF_8))
        it.fd.sync()
      }
      true
    } catch (e: Exception) {
      false
    }
  }
}
