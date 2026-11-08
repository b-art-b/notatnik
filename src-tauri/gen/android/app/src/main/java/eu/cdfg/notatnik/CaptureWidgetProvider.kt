package eu.cdfg.notatnik

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews

/** The capture bar (AN-6): a tap opens [CaptureActivity]. */
class CaptureWidgetProvider : AppWidgetProvider() {
  override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
    val open = PendingIntent.getActivity(
      context,
      0,
      Intent(context, CaptureActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )
    for (id in ids) {
      val views = RemoteViews(context.packageName, R.layout.widget_capture)
      views.setOnClickPendingIntent(R.id.widget_capture_root, open)
      manager.updateAppWidget(id, views)
    }
  }
}
