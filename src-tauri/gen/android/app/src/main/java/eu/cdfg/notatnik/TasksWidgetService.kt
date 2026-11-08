package eu.cdfg.notatnik

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.RemoteViews
import android.widget.RemoteViewsService

/** The rows of the task widget: one per task of the snapshot, none when it is missing or from another day. */
class TasksWidgetService : RemoteViewsService() {
  override fun onGetViewFactory(intent: Intent): RemoteViewsFactory = Factory(applicationContext)

  private class Factory(private val context: Context) : RemoteViewsFactory {
    private var tasks: List<WidgetData.Task> = emptyList()

    override fun onCreate() {}
    override fun onDataSetChanged() {
      val s = WidgetFiles.snapshot(context)
      tasks = if (WidgetData.stateOf(s, WidgetData.today()) == WidgetData.State.OK) s!!.tasks else emptyList()
    }

    override fun onDestroy() {}
    override fun getCount() = tasks.size
    override fun getViewAt(position: Int): RemoteViews? {
      val t = tasks.getOrNull(position) ?: return null
      val v = RemoteViews(context.packageName, R.layout.widget_task_row)
      v.setTextViewText(R.id.row_text, t.text)
      v.setTextViewText(
        R.id.row_page,
        if (t.overdue) context.getString(R.string.tasks_overdue, t.page) else t.page,
      )
      v.setContentDescription(R.id.row_root, context.getString(R.string.tasks_row_description, t.text, t.page))
      v.setOnClickFillInIntent(R.id.row_root, Intent().setData(Uri.parse(WidgetData.pageLink(t.page))))
      return v
    }

    override fun getLoadingView(): RemoteViews? = null
    override fun getViewTypeCount() = 1
    override fun getItemId(position: Int) = position.toLong()
    override fun hasStableIds() = false
  }
}
