package app.clipcat.editor.spike

import android.content.Context
import android.view.Surface

/**
 * The two JNI entry points UniFFI cannot carry (src/crates/clipcat-mobile/src/jni.rs).
 * Loading the library here, through System.loadLibrary, is also what runs it
 * once before JNA opens the same .so for the UniFFI calls.
 */
object ClipCatNative {
    init {
        System.loadLibrary("clipcat_mobile")
    }

    /** Logcat, and the JavaVM + Context that cpal's AAudio host reads. */
    @JvmStatic external fun init(context: Context)

    /** The Surface's ANativeWindow with one reference acquired; 0 for none. */
    @JvmStatic external fun windowFromSurface(surface: Surface): Long
}
