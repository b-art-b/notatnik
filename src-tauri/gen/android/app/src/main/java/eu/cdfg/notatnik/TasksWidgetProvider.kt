package eu.cdfg.notatnik

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.RemoteViews

/** Today's tasks (AN-7): a list read from the snapshot the app writes; a row opens its page in the app. */
class TasksWidgetProvider : AppWidgetProvider() {
  override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
    for (id in ids) manager.updateAppWidget(id, views(context, id))
    manager.notifyAppWidgetViewDataChanged(ids, R.id.widget_tasks_list)
  }

  // the date changed while the app was not running: yesterday's list must not stay
  override fun onReceive(context: Context, intent: Intent) {
    super.onReceive(context, intent)
    if (intent.action == Intent.ACTION_DATE_CHANGED || intent.action == Intent.ACTION_TIMEZONE_CHANGED) {
      refreshAll(context)
    }
  }

  companion object {
    /** Draws all placed task widgets again from the snapshot. */
    fun refreshAll(context: Context) {
      val manager = AppWidgetManager.getInstance(context)
      val ids = manager.getAppWidgetIds(ComponentName(context, TasksWidgetProvider::class.java))
      if (ids.isEmpty()) return
      for (id in ids) manager.updateAppWidget(id, views(context, id))
      manager.notifyAppWidgetViewDataChanged(ids, R.id.widget_tasks_list)
    }

    private fun link(context: Context, uri: String, code: Int): PendingIntent = PendingIntent.getActivity(
      context,
      code,
      Intent(context, MainActivity::class.java).setData(Uri.parse(uri)),
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    private fun views(context: Context, id: Int): RemoteViews {
      val v = RemoteViews(context.packageName, R.layout.widget_tasks)
      val state = WidgetData.stateOf(WidgetFiles.snapshot(context), WidgetData.today())
      v.setTextViewText(
        R.id.widget_tasks_empty,
        context.getString(
          when (state) {
            WidgetData.State.MISSING -> R.string.tasks_set_up
            WidgetData.State.STALE -> R.string.tasks_refresh
            else -> R.string.tasks_none
          }
        ),
      )
      val service = Intent(context, TasksWidgetService::class.java)
        .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id)
        .setData(Uri.parse("notatnik-widget://tasks/$id"))
      v.setRemoteAdapter(R.id.widget_tasks_list, service)
      v.setEmptyView(R.id.widget_tasks_list, R.id.widget_tasks_empty)
      // the header opens the agenda; a row opens its page (the fill-in intent of the row carries the link)
      v.setOnClickPendingIntent(R.id.widget_tasks_header, link(context, "notatnik://agenda", 1))
      v.setOnClickPendingIntent(R.id.widget_tasks_empty, link(context, "notatnik://agenda", 2))
      v.setPendingIntentTemplate(
        R.id.widget_tasks_list,
        PendingIntent.getActivity(
          context,
          3,
          Intent(context, MainActivity::class.java),
          PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        ),
      )
      return v
    }
  }
}
