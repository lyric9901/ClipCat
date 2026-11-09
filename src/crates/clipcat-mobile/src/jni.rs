// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah

//! The two things UniFFI cannot carry: the app's `Context` (which cpal's
//! AAudio host reads through ndk-context to size its buffers) and a
//! `Surface` (whose ANativeWindow the render thread draws on). Both are
//! plain JNI functions on `app.clipcat.editor.spike.ClipCatNative`, which the
//! Kotlin side reaches after `System.loadLibrary("clipcat_mobile")`.

use jni_sys::{JNIEnv, jclass, jlong, jobject};

/// `ClipCatNative.init(context)`: logcat, and the JavaVM and Context for
/// ndk-context. Once per process; later calls do nothing.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_app_clipcat_editor_spike_ClipCatNative_init(
    env: *mut JNIEnv,
    class: jclass,
    context: jobject,
) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("clipcat-spike"),
        );
        // SAFETY: env is the calling thread's JNIEnv, valid for this call.
        unsafe {
            let functions = &**env;
            let mut vm = std::ptr::null_mut();
            let get_vm = functions.GetJavaVM.expect("GetJavaVM");
            if get_vm(env, &mut vm) != 0 || vm.is_null() {
                log::error!("no JavaVM");
                return;
            }
            let global = functions.NewGlobalRef.expect("NewGlobalRef")(env, context);
            ndk_context::initialize_android_context(vm.cast(), global.cast());
        }
        log::info!("clipcat-spike {} native side up", env!("CARGO_PKG_VERSION"));
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_app_concat_editor_spike_ConcatNative_init(
    env: *mut JNIEnv,
    class: jclass,
    context: jobject,
) {
    Java_app_clipcat_editor_spike_ClipCatNative_init(env, class, context);
}

/// `ClipCatNative.windowFromSurface(surface)`: the Surface's ANativeWindow
/// with one reference acquired, as a long, or 0. `Engine.attachSurface`
/// takes that reference over.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_app_clipcat_editor_spike_ClipCatNative_windowFromSurface(
    env: *mut JNIEnv,
    _class: jclass,
    surface: jobject,
) -> jlong {
    // SAFETY: env and surface are this call's own arguments.
    let window = unsafe { ndk_sys::ANativeWindow_fromSurface(env.cast(), surface.cast()) };
    window as jlong
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_app_concat_editor_spike_ConcatNative_windowFromSurface(
    env: *mut JNIEnv,
    class: jclass,
    surface: jobject,
) -> jlong {
    Java_app_clipcat_editor_spike_ClipCatNative_windowFromSurface(env, class, surface)
}
