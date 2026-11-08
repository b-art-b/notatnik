package eu.cdfg.notatnik

import org.json.JSONArray
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID

/**
 * The data of the home-screen widgets (AN-6, AN-7): the snapshot `today.json` the app writes and the line the capture
 * dialog appends to `capture-queue.jsonl`. Pure code, no Android classes but `org.json`, so a plain JVM test runs it.
 */
object WidgetData {
  data class Task(val page: String, val text: String, val due: String?, val overdue: Boolean, val marker: String)

  data class Snapshot(val graph: String, val date: String, val tasks: List<Task>, val more: Int)

  /** What a widget shows for a snapshot. */
  enum class State { MISSING, STALE, EMPTY, OK }

  /** The snapshot in `json`, or null when it is not one (a half written file never happens: the app renames). */
  fun parseSnapshot(json: String?): Snapshot? {
    if (json.isNullOrBlank()) return null
    return try {
      val o = JSONObject(json)
      if (o.optInt("v", 0) != 1) return null
      val list = o.optJSONArray("tasks") ?: JSONArray()
      val tasks = (0 until list.length()).mapNotNull { i ->
        val t = list.optJSONObject(i) ?: return@mapNotNull null
        Task(
          page = t.optString("page"),
          text = t.optString("text"),
          due = if (t.isNull("due")) null else t.optString("due"),
          overdue = t.optBoolean("overdue", false),
          marker = t.optString("marker"),
        )
      }
      Snapshot(o.optString("graph"), o.optString("date"), tasks, o.optInt("more", 0))
    } catch (e: Exception) {
      null
    }
  }

  fun today(now: Date = Date()): String = SimpleDateFormat("yyyy-MM-dd", Locale.ROOT).format(now)

  /** Yesterday's tasks are not shown as today's: a snapshot of another day is stale. */
  fun stateOf(s: Snapshot?, today: String): State = when {
    s == null -> State.MISSING
    s.date != today -> State.STALE
    s.tasks.isEmpty() -> State.EMPTY
    else -> State.OK
  }

  /** One queue line: a single JSON object ending in a newline, so that one append is a whole line. */
  fun queueLine(text: String, graph: String, now: Date = Date(), id: String = UUID.randomUUID().toString()): String {
    val at = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss", Locale.ROOT).format(now)
    val o = JSONObject()
    o.put("id", id)
    o.put("graph", graph)
    o.put("text", text)
    o.put("at", at)
    return o.toString() + "\n"
  }

  /** The link that opens a page of the app. */
  fun pageLink(name: String): String =
    "notatnik://page/" + java.net.URLEncoder.encode(name, "UTF-8").replace("+", "%20")
}
