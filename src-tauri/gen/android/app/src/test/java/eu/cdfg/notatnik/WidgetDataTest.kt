package eu.cdfg.notatnik

import java.util.Calendar
import java.util.Date
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WidgetDataTest {
  private val json = """{"v":1,"graph":"g1","date":"2026-11-08","written_at":"2026-11-08T08:00:00",
    "tasks":[{"page":"Plan","text":"call Anna","due":"2026-11-08","overdue":false,"marker":"TODO"},
             {"page":"Home","text":"pay \"rent\"","due":null,"overdue":true,"marker":"DOING"}],"more":3}"""

  @Test
  fun aSnapshotIsRead() {
    val s = WidgetData.parseSnapshot(json)!!
    assertEquals("g1", s.graph)
    assertEquals(2, s.tasks.size)
    assertEquals("call Anna", s.tasks[0].text)
    assertTrue(s.tasks[1].overdue)
    assertNull(s.tasks[1].due)
    assertEquals(3, s.more)
  }

  @Test
  fun somethingThatIsNotASnapshotIsNull() {
    assertNull(WidgetData.parseSnapshot(null))
    assertNull(WidgetData.parseSnapshot(""))
    assertNull(WidgetData.parseSnapshot("not json"))
    assertNull(WidgetData.parseSnapshot("""{"v":2,"tasks":[]}"""))
  }

  @Test
  fun yesterdaysTasksAreStaleNotToday() {
    val s = WidgetData.parseSnapshot(json)
    assertEquals(WidgetData.State.OK, WidgetData.stateOf(s, "2026-11-08"))
    assertEquals(WidgetData.State.STALE, WidgetData.stateOf(s, "2026-11-09"))
    assertEquals(WidgetData.State.MISSING, WidgetData.stateOf(null, "2026-11-08"))
    val empty = WidgetData.parseSnapshot("""{"v":1,"graph":"g","date":"2026-11-08","tasks":[]}""")
    assertEquals(WidgetData.State.EMPTY, WidgetData.stateOf(empty, "2026-11-08"))
  }

  @Test
  fun aQueueLineIsOneJsonObjectEndingInANewline() {
    val tricky = "say \"hi\"\nsecond line 😀 " + "x".repeat(5000)
    val at = Calendar.getInstance().apply { set(2026, Calendar.NOVEMBER, 8, 23, 58, 5) }.time
    val line = WidgetData.queueLine(tricky, "g1", at, "id-1")
    assertTrue(line.endsWith("\n"))
    assertEquals(1, line.count { it == '\n' }) // the newline of the text is escaped, so the line stays one line
    val o = JSONObject(line)
    assertEquals(tricky, o.getString("text"))
    assertEquals("g1", o.getString("graph"))
    assertEquals("id-1", o.getString("id"))
    assertEquals("2026-11-08T23:58:05", o.getString("at"))
  }

  @Test
  fun aPageLinkEncodesTheName() {
    assertEquals("notatnik://page/Projects%2FHome%20page", WidgetData.pageLink("Projects/Home page"))
    assertFalse(WidgetData.pageLink("a b").contains(" "))
    assertEquals(10, WidgetData.today(Date()).length)
  }
}
