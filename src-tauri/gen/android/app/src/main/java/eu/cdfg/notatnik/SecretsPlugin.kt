package eu.cdfg.notatnik

import android.app.Activity
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The passwords of server logins and the key of an encrypted graph (SY-7, EN-10, AS1). The Rust side (`keystore.rs`)
 * implements its `Secrets` trait with the three commands below.
 *
 * One AES-256-GCM key lives in the Android Keystore: it cannot be read out of the device, and on most phones it sits in
 * secure hardware. The values themselves are kept encrypted with it in a private preferences file of the app. The name
 * of a value is bound into the encryption (as associated data), so a value cannot be moved under another name.
 *
 * A value that cannot be decrypted (the key is gone, e.g. after a restore from a backup onto another phone) reads as
 * "not there": the app then asks for the password again. It never crashes and never returns garbage.
 */
@TauriPlugin
class SecretsPlugin(private val activity: Activity) : Plugin(activity) {
  private val prefs by lazy { activity.getSharedPreferences(FILE, Context.MODE_PRIVATE) }

  @Command
  fun get(invoke: Invoke) {
    val key = invoke.getArgs().getString("key")
    val out = JSObject()
    out.put("value", if (key == null) null else read(key))
    invoke.resolve(out)
  }

  @Command
  fun set(invoke: Invoke) {
    val args = invoke.getArgs()
    val key = args.getString("key")
    val value = args.getString("value")
    if (key == null || value == null) {
      invoke.reject("key and value are needed")
      return
    }
    try {
      prefs.edit().putString(key, seal(key, value)).apply()
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject("the Android Keystore did not take the value: ${e.message}")
    }
  }

  @Command
  fun delete(invoke: Invoke) {
    invoke.getArgs().getString("key")?.let { prefs.edit().remove(it).apply() }
    invoke.resolve()
  }

  private fun read(key: String): String? {
    val stored = prefs.getString(key, null) ?: return null
    return try {
      open(key, stored)
    } catch (e: Exception) {
      // a key that does not fit: forget the value, the app asks again
      prefs.edit().remove(key).apply()
      null
    }
  }

  private fun seal(name: String, value: String): String {
    val cipher = Cipher.getInstance(MODE)
    cipher.init(Cipher.ENCRYPT_MODE, masterKey())
    cipher.updateAAD(name.toByteArray(Charsets.UTF_8))
    val iv = cipher.iv
    val data = cipher.doFinal(value.toByteArray(Charsets.UTF_8))
    return Base64.encodeToString(iv + data, Base64.NO_WRAP)
  }

  private fun open(name: String, stored: String): String {
    val raw = Base64.decode(stored, Base64.NO_WRAP)
    val cipher = Cipher.getInstance(MODE)
    cipher.init(Cipher.DECRYPT_MODE, masterKey(), GCMParameterSpec(TAG_BITS, raw, 0, IV_BYTES))
    cipher.updateAAD(name.toByteArray(Charsets.UTF_8))
    return String(cipher.doFinal(raw, IV_BYTES, raw.size - IV_BYTES), Charsets.UTF_8)
  }

  private fun masterKey(): SecretKey {
    val store = KeyStore.getInstance(PROVIDER).apply { load(null) }
    (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
    val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
    gen.init(
      KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .build()
    )
    return gen.generateKey()
  }

  private companion object {
    const val PROVIDER = "AndroidKeyStore"
    const val ALIAS = "notatnik-secrets"
    const val FILE = "notatnik_secrets"
    const val MODE = "AES/GCM/NoPadding"
    const val IV_BYTES = 12
    const val TAG_BITS = 128
  }
}
