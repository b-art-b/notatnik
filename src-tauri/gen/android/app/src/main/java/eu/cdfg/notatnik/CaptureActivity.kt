package eu.cdfg.notatnik

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

/**
 * The dialog the capture widget opens (AN-6): one line, "Add" puts it in the queue the app drains into today's journal
 * the next time it runs. It never starts the app; "Open in app" does. Nothing here writes the graph.
 */
class CaptureActivity : AppCompatActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    setContentView(R.layout.activity_capture)
    window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE)
    val input = findViewById<EditText>(R.id.capture_input)
    findViewById<android.view.View>(R.id.capture_add).setOnClickListener { add(input) }
    findViewById<android.view.View>(R.id.capture_cancel).setOnClickListener { finish() }
    findViewById<android.view.View>(R.id.capture_open).setOnClickListener {
      startActivity(
        Intent(this, MainActivity::class.java)
          .setData(Uri.parse("notatnik://capture"))
          .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
      )
      finish()
    }
    input.setOnEditorActionListener { _, action, _ ->
      if (action == EditorInfo.IME_ACTION_DONE) add(input)
      action == EditorInfo.IME_ACTION_DONE
    }
    input.requestFocus()
  }

  private fun add(input: EditText) {
    val text = input.text.toString().trim()
    if (text.isEmpty()) {
      input.error = getString(R.string.capture_empty)
      return
    }
    if (WidgetFiles.enqueue(this, text)) {
      Toast.makeText(this, R.string.capture_saved, Toast.LENGTH_SHORT).show()
      finish()
    } else {
      // the app has never run: it does not know where to look yet
      Toast.makeText(this, R.string.capture_set_up, Toast.LENGTH_LONG).show()
    }
  }
}
