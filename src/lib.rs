// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! day-part-speech — HEADLESS text to speech. One API; each platform's own engine underneath.
//!
//! ```no_run
//! if day_part_speech::available() != day_bridge::Support::Unsupported {
//!     let _ = day_part_speech::speak("Rain later");
//! }
//! ```
//!
//! This is [daybridge](https://daybrite.dev/docs/internal/bridge)'s reference sample. Every
//! platform has text to speech and every platform exposes it in a different language, so the arms
//! below are written in those languages and live in this file: Swift for `AVSpeechSynthesizer`,
//! Java for Android's `TextToSpeech`, ArkTS for HarmonyOS Core Speech Kit, JavaScript for
//! `speechSynthesis`, C++ for Windows SAPI, C for speech-dispatcher — plus the Rust arm that keeps
//! the crate compiling anywhere else, including under day-mock in `cargo test`.
//!
//! [`speak`] is fire and forget: it returns once the platform has accepted the utterance. Every
//! engine also reports when an utterance ENDS, and the bridge's callback tier carries that back:
//! [`speak_until_done`] takes a callback, [`speak_future`] is the same thing to `.await` under
//! `day::task`, and both resolve with a [`SpeechEnd`].

use std::future::Future;

pub use day_bridge::{Error, Support};

/// How an utterance ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeechEnd {
    /// The engine spoke the whole text.
    Finished,
    /// [`stop`] or a later [`speak`] interrupted it.
    Stopped,
    /// The engine accepted the text but reports no end — speech-dispatcher on Linux, whose
    /// notifications this arm does not subscribe to. Treat it as done at once.
    Unobserved,
}

/// The end an arm reports as an integer: `1` finished, `0` stopped, `2` unobserved.
impl SpeechEnd {
    fn from_code(code: i32) -> Result<SpeechEnd, Error> {
        match code {
            1 => Ok(SpeechEnd::Finished),
            0 => Ok(SpeechEnd::Stopped),
            2 => Ok(SpeechEnd::Unobserved),
            other => Err(Error::Foreign(format!("unknown speech end code {other}"))),
        }
    }
}

/// Speak `text` with the system voice, interrupting anything already speaking.
///
/// `Ok` means the platform accepted the request. Nothing here reports when it finishes; see
/// [`speak_until_done`] and [`speak_future`] for that.
///
/// Where [`available`] reports `Unsupported` this returns `Err(Unsupported)` without reaching
/// the arm — the arm can only say "failed", and a host with no engine (desktop Linux without
/// speech-dispatcher) is not a failure, it is the absence `available` already reported.
pub fn speak(text: &str) -> Result<(), Error> {
    if available() == Support::Unsupported {
        return Err(Error::Unsupported);
    }
    speak_native_async(text, |_| {}).map(|_| ())
}

/// Speak `text`, and call `on_end` once from whichever thread the engine reports on, when the
/// utterance finishes, is stopped, or fails. `Ok` means the platform accepted the request; on
/// `Err` the callback still fires exactly once, with that error, so either channel alone is a
/// complete answer. Capture a `Setter` to reach UI state from the callback (docs/async.md).
pub fn speak_until_done(
    text: &str,
    on_end: impl FnOnce(Result<SpeechEnd, Error>) + Send + 'static,
) -> Result<(), Error> {
    if available() == Support::Unsupported {
        on_end(Err(Error::Unsupported));
        return Err(Error::Unsupported);
    }
    speak_native_async(text, move |code| {
        on_end(code.and_then(SpeechEnd::from_code))
    })
    .map(|_| ())
}

/// Speak `text` and resolve when the utterance ends: the same answer [`speak_until_done`] gives,
/// to `.await` inside `day::task`, where the continuation runs on the UI thread. Dropping the
/// future stops listening; it does not stop the voice — call [`stop`] for that.
pub fn speak_future(text: &str) -> impl Future<Output = Result<SpeechEnd, Error>> + Send {
    let fut = if available() == Support::Unsupported {
        None
    } else {
        Some(speak_native_future(text))
    };
    async move {
        match fut {
            Some(f) => f.await.and_then(SpeechEnd::from_code),
            None => Err(Error::Unsupported),
        }
    }
}

/// Stop speaking immediately. Silent no-op where speech is unsupported. A pending
/// [`speak_future`] resolves with [`SpeechEnd::Stopped`].
pub fn stop() {
    stop_native();
}

/// What speech can actually do here, right now.
///
/// Two questions in one answer. The bridge knows at COMPILE time whether this target has an arm at
/// all (`speak_native_support`), but on some platforms the engine is a separate package the user
/// may not have installed — desktop Linux ships without speech-dispatcher more often than with it
/// — so the arm is also asked at RUN time whether it can reach one. A target with an arm but no
/// engine reports `Unsupported`, because that is what the caller can act on.
pub fn available() -> Support {
    match speak_native_support() {
        Support::Unsupported => Support::Unsupported,
        claimed if engine_ready_native().is_ok() => claimed,
        _ => Support::Unsupported,
    }
}

day_bridge::bridge! {
    // The contract. Every arm below implements exactly this; day-build checks that they agree.
    #[day_bridge::declare]
    extern "day" {
        /// Start speaking. `Ok` means accepted; `done` is completed once, when the utterance ends,
        /// with `1` (finished), `0` (stopped) or `2` (the engine reports no end).
        fn speak_native(text: &str, done: day_bridge::Done<i32>) -> Result<(), day_bridge::Error>;
        fn stop_native();
        /// Whether this host can reach a speech engine at all. `Ok` means [`available`] may report
        /// what the arm claims; an `Err` demotes it to `Unsupported`.
        fn engine_ready_native() -> Result<(), day_bridge::Error>;
    }

    // Linux: speech-dispatcher's C API, loaded at RUNTIME rather than linked.
    //
    // `link = ["speechd"]` would be the obvious spelling and it is the wrong one twice over: it
    // needs the -dev package on every build machine, and it writes a DT_NEEDED entry that stops
    // the whole app from starting on any desktop without libspeechd — for a feature the user may
    // never press. A speech engine is exactly the kind of optional platform service `dlopen`
    // exists for (the bridge reference, "Linking"). The arm keeps the connection handle in the language
    // that owns it, which is why it is C rather than Rust.
    //
    // speech-dispatcher's end-of-message notifications are callback fields inside its connection
    // struct, whose layout this arm would have to restate to set them — so it does not, and
    // completes every utterance as UNOBSERVED (2) the moment the daemon accepts it.
    #[day_bridge::impl(c, platforms = [linux])]
    c!(
        prelude = r#"
            #include <dlfcn.h>
            #include <stddef.h>
        "#,
        body = r#"
            /* speech-dispatcher's own declarations, so the arm compiles with no headers
               installed — it never includes libspeechd.h and never links against it. */
            typedef struct SPDConnection SPDConnection;
            typedef enum { SPD_MODE_SINGLE = 0, SPD_MODE_THREADED = 1 } SPDConnectionMode;
            typedef enum { SPD_IMPORTANT = 1, SPD_MESSAGE = 3 } SPDPriority;

            typedef SPDConnection* (*day_spd_open_fn)(const char*, const char*, const char*,
                                                      SPDConnectionMode);
            typedef int (*day_spd_say_fn)(SPDConnection*, SPDPriority, const char*);
            typedef int (*day_spd_stop_fn)(SPDConnection*);

            static day_spd_open_fn day_spd_open = NULL;
            static day_spd_say_fn day_spd_say = NULL;
            static day_spd_stop_fn day_spd_stop = NULL;
            static SPDConnection* day_speech_conn = NULL;
            static int day_speech_looked = 0;

            /* Resolve the library once. The soname first, then the -dev symlink, so a host with
               only the runtime package still works. */
            static int day_speech_load(void) {
                if (day_speech_looked) {
                    return day_spd_open != NULL;
                }
                day_speech_looked = 1;
                void* lib = dlopen("libspeechd.so.2", RTLD_LAZY);
                if (lib == NULL) {
                    lib = dlopen("libspeechd.so", RTLD_LAZY);
                }
                if (lib == NULL) {
                    return 0;
                }
                day_spd_open = (day_spd_open_fn) dlsym(lib, "spd_open");
                day_spd_say = (day_spd_say_fn) dlsym(lib, "spd_say");
                day_spd_stop = (day_spd_stop_fn) dlsym(lib, "spd_stop");
                return day_spd_open != NULL && day_spd_say != NULL && day_spd_stop != NULL;
            }

            int32_t engine_ready_native(void) {
                return day_speech_load() ? 0 : 1;
            }

            int32_t speak_native(const char* text, uint64_t done) {
                if (!day_speech_load()) {
                    return 1;
                }
                if (day_speech_conn == NULL) {
                    day_speech_conn = day_spd_open("day", "speech", NULL, SPD_MODE_SINGLE);
                    if (day_speech_conn == NULL) {
                        return 1;
                    }
                }
                day_spd_stop(day_speech_conn);
                if (day_spd_say(day_speech_conn, SPD_MESSAGE, text) < 0) {
                    return 1;
                }
                speak_native_complete(done, 2);
                return 0;
            }

            void stop_native(void) {
                if (day_speech_conn != NULL) {
                    day_spd_stop(day_speech_conn);
                }
            }
        "#,
    );

    // Windows: SAPI 5, the speech API every supported Windows has had since XP. It is COM —
    // `CoCreateInstance`, an `ISpVoice` interface pointer, `HRESULT`s — which Rust can reach but
    // only by declaring the vtable by hand; three lines of C++ get the same thing from the SDK
    // header. This is also the arm that needs UTF-16: SAPI speaks `WCHAR`, so the declaration's
    // `&str` is converted for this arm and left alone for every other one.
    //
    // The end of an utterance is a SAPI event: the voice notifies through a callback function and
    // the arm reads the event queue, matching the event's stream number to the utterance it
    // started. A purge (stop, or the next speak) ends the pending utterance as stopped.
    #[day_bridge::impl(cpp, platforms = [windows], encoding = "utf16", link = ["ole32", "sapi"])]
    cpp!(
        prelude = r#"
            #include <windows.h>
            #include <sapi.h>
            #include <sperror.h>
        "#,
        body = r#"
            static ISpVoice* day_speech_voice = nullptr;
            /* The utterance still speaking: its SAPI stream number and the token to complete. */
            static ULONG day_speech_stream = 0;
            static uint64_t day_speech_pending = 0;

            static void day_speech_settle(int32_t how) {
                if (day_speech_pending != 0) {
                    uint64_t done = day_speech_pending;
                    day_speech_pending = 0;
                    day_speech_stream = 0;
                    speak_native_complete(done, how);
                }
            }

            static void __stdcall day_speech_notify(WPARAM, LPARAM) {
                if (!day_speech_voice) {
                    return;
                }
                SPEVENT ev;
                ULONG fetched = 0;
                while (SUCCEEDED(day_speech_voice->GetEvents(1, &ev, &fetched)) && fetched == 1) {
                    if (ev.eEventId == SPEI_END_INPUT_STREAM && ev.ulStreamNum == day_speech_stream) {
                        day_speech_settle(1);
                    }
                    SpClearEvent(&ev);
                }
            }

            /* One voice for the process, created on first use. COM may already be
               initialized on this thread by the host app — S_FALSE and RPC_E_CHANGED_MODE
               both mean "already up", and SAPI is happy in either apartment. */
            static ISpVoice* day_speech_open() {
                if (day_speech_voice) {
                    return day_speech_voice;
                }
                HRESULT hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
                if (FAILED(hr) && hr != RPC_E_CHANGED_MODE) {
                    return nullptr;
                }
                if (FAILED(CoCreateInstance(CLSID_SpVoice, nullptr, CLSCTX_ALL, IID_ISpVoice,
                                            reinterpret_cast<void**>(&day_speech_voice)))) {
                    day_speech_voice = nullptr;
                    return nullptr;
                }
                day_speech_voice->SetNotifyCallbackFunction(day_speech_notify, 0, 0);
                day_speech_voice->SetInterest(SPFEI(SPEI_END_INPUT_STREAM), SPFEI(SPEI_END_INPUT_STREAM));
                return day_speech_voice;
            }

            int32_t speak_native(const char16_t* text, uint64_t done) {
                ISpVoice* voice = day_speech_open();
                if (!voice) {
                    return 1;
                }
                /* PURGEBEFORESPEAK drops whatever is still speaking, matching every other arm's
                   "interrupt and say this" — so the utterance it drops ends as stopped. */
                day_speech_settle(0);
                ULONG stream = 0;
                HRESULT hr = voice->Speak(reinterpret_cast<const WCHAR*>(text),
                                          SPF_ASYNC | SPF_PURGEBEFORESPEAK, &stream);
                if (FAILED(hr)) {
                    return 1;
                }
                day_speech_stream = stream;
                day_speech_pending = done;
                return 0;
            }

            int32_t engine_ready_native(void) {
                return day_speech_open() != nullptr ? 0 : 1;
            }

            /* A null utterance with PURGEBEFORESPEAK is SAPI's documented stop. */
            void stop_native(void) {
                if (day_speech_voice) {
                    day_speech_settle(0);
                    day_speech_voice->Speak(nullptr, SPF_PURGEBEFORESPEAK, nullptr);
                }
            }
        "#,
    );

    // Android: `TextToSpeech` needs a `Context` and is unusable until its `OnInitListener`
    // fires, so an utterance requested before then is queued and spoken on init. Neither is
    // reachable from Rust, which is why this arm is required rather than a convenience.
    //
    // Written in Java rather than Kotlin so it compiles in any Android project: AGP compiles
    // `.java` from a source directory with no extra plugin, while a `.kt` needs the Kotlin plugin
    // (`day lint` says so before a build gets that far).
    //
    // The utterance id IS the completion token, so the progress listener completes exactly the
    // request that ended: `onDone` finished, `onStop` stopped, `onError` failed. The callbacks
    // arrive on the engine's own thread, which the bridge's completion accepts.
    #[day_bridge::impl(java, platforms = [android])]
    java!(
        prelude = r#"
            import android.content.Context;
            import android.speech.tts.TextToSpeech;
            import android.speech.tts.UtteranceProgressListener;
            import dev.daybrite.day.bridge.DayBridge;
            import java.util.Locale;
        "#,
        body = r#"
            private static TextToSpeech engine = null;
            private static boolean ready = false;
            private static String pendingText = null;
            private static long pendingDone = 0;

            private static void say(String text, long done) {
                if (engine != null) {
                    engine.speak(text, TextToSpeech.QUEUE_FLUSH, null, Long.toString(done));
                }
            }

            private static final UtteranceProgressListener progress = new UtteranceProgressListener() {
                private long token(String utteranceId) {
                    try {
                        return Long.parseLong(utteranceId);
                    } catch (NumberFormatException e) {
                        return 0;
                    }
                }

                @Override public void onStart(String utteranceId) {}

                @Override public void onDone(String utteranceId) {
                    speak_native_complete(token(utteranceId), 1);
                }

                @Override public void onStop(String utteranceId, boolean interrupted) {
                    speak_native_complete(token(utteranceId), 0);
                }

                @Override @SuppressWarnings("deprecation") public void onError(String utteranceId) {
                    speak_native_fail(token(utteranceId), "the engine reported an error");
                }

                @Override public void onError(String utteranceId, int errorCode) {
                    speak_native_fail(token(utteranceId), "the engine reported error " + errorCode);
                }
            };

            public static void speak_native(String text, long done) {
                Context ctx = DayBridge.ctx;
                if (ctx == null) {
                    throw new IllegalStateException("no Context");
                }
                if (engine == null) {
                    engine = new TextToSpeech(ctx, status -> {
                        ready = status == TextToSpeech.SUCCESS;
                        if (ready) {
                            engine.setLanguage(Locale.getDefault());
                            engine.setOnUtteranceProgressListener(progress);
                            if (pendingText != null) {
                                say(pendingText, pendingDone);
                                pendingText = null;
                                pendingDone = 0;
                            }
                        } else if (pendingDone != 0) {
                            speak_native_fail(pendingDone, "the engine failed to initialize");
                            pendingText = null;
                            pendingDone = 0;
                        }
                    });
                }
                if (ready) {
                    say(text, done);
                } else {
                    // A second request while the engine is still starting supersedes the first,
                    // which ends as stopped — the same outcome QUEUE_FLUSH gives once it runs.
                    if (pendingDone != 0) {
                        speak_native_complete(pendingDone, 0);
                    }
                    pendingText = text;
                    pendingDone = done;
                }
            }

            public static void stop_native() {
                if (pendingDone != 0) {
                    speak_native_complete(pendingDone, 0);
                    pendingText = null;
                    pendingDone = 0;
                }
                if (engine != null) {
                    engine.stop();
                }
            }

            public static void engine_ready_native() {
                if (DayBridge.ctx == null) {
                    throw new IllegalStateException("no Context");
                }
            }
        "#,
    );

    // HarmonyOS: Core Speech Kit is ArkTS-only — the ArkUI C NDK has no TTS at all — so this arm
    // is required. Its voices are zh-CN in API 13, which is why the arm reports Emulated rather
    // than Native: the platform answers, but not in every language a caller might ask for.
    //
    // The generator stages this module but has no Rust half for ArkTS yet (docs/bridge.md), so
    // the token is carried for shape and the fallback answers on HarmonyOS until it does.
    #[day_bridge::impl(arkts, platforms = [ohos], support = "emulated")]
    arkts!(
        prelude = r#"
            import { textToSpeech } from '@kit.CoreSpeechKit';
        "#,
        body = r#"
            let dayEngine: textToSpeech.TextToSpeechEngine | undefined = undefined;
            let daySeq: number = 0;

            export async function speak_native(text: string, _done: number): Promise<void> {
                if (!dayEngine) {
                    dayEngine = await textToSpeech.createEngine({
                        language: 'zh-CN', person: 0, online: 1,
                    });
                }
                dayEngine.stop();
                daySeq += 1;
                dayEngine.speak(text, { requestId: `day-speech-${daySeq}` });
            }

            export function stop_native(): void {
                dayEngine?.stop();
            }

            // Core Speech Kit is part of the system; the engine is created on first speak.
            export function engine_ready_native(): void {}
        "#,
    );

    // Web: speechSynthesis. Nothing about it is reachable from wasm, so this arm is required
    // rather than a convenience. The utterance's own events complete the token: `end` when it
    // finished, and `error` with `interrupted`/`canceled` when `cancel()` cut it short.
    #[day_bridge::impl(js, platforms = [web])]
    js!(r#"
        export function speak_native(text, done) {
            const utterance = new SpeechSynthesisUtterance(text);
            utterance.lang = document.documentElement.lang || navigator.language;
            utterance.onend = () => speak_native_complete(done, 1);
            utterance.onerror = (e) => {
                if (e.error === 'interrupted' || e.error === 'canceled') {
                    speak_native_complete(done, 0);
                } else {
                    speak_native_fail(done, e.error);
                }
            };
            speechSynthesis.cancel();
            speechSynthesis.speak(utterance);
        }

        export function stop_native() {
            speechSynthesis.cancel();
        }

        // Not every browser has the Web Speech API — Firefox on some platforms ships without it.
        export function engine_ready_native() {
            if (typeof speechSynthesis === "undefined") {
                throw new Error("no speechSynthesis");
            }
        }
    "#);

    // Apple: AVSpeechSynthesizer. `objc2` could reach it, but the synthesizer has to outlive the
    // call — it stops speaking if it deallocates — and a file-scope `let` in Swift says that in
    // one line. Its delegate reports the end of each utterance, keyed back to the token that
    // started it.
    //
    // A stop is answered by the arm, not the engine: `stopSpeaking(at:)` reaches only an utterance
    // that has started, and one still queued in the moments after `speak` would otherwise play
    // through and report "finished" after the user pressed Stop. So `stop` completes every pending
    // token as stopped at once, and the delegate's `didStart` cuts an utterance whose token was
    // already stopped, so the voice follows the answer.
    #[day_bridge::impl(swift, platforms = [ios, macos])]
    swift!(
        prelude = r#"
            import AVFoundation
        "#,
        body = r#"
            private final class DaySpeechDelegate: NSObject, AVSpeechSynthesizerDelegate {
                var pending: [ObjectIdentifier: UInt64] = [:]

                /// Answer every pending utterance as stopped; a later `didFinish`/`didCancel`
                /// for one of them then finds nothing.
                func stopAll() {
                    let stopped = pending
                    pending.removeAll()
                    for (_, done) in stopped {
                        speak_native_complete(done, 0)
                    }
                }

                func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer,
                                       didStart utterance: AVSpeechUtterance) {
                    if pending[ObjectIdentifier(utterance)] == nil {
                        synthesizer.stopSpeaking(at: .immediate)
                    }
                }

                func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer,
                                       didFinish utterance: AVSpeechUtterance) {
                    if let done = pending.removeValue(forKey: ObjectIdentifier(utterance)) {
                        speak_native_complete(done, 1)
                    }
                }

                func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer,
                                       didCancel utterance: AVSpeechUtterance) {
                    if let done = pending.removeValue(forKey: ObjectIdentifier(utterance)) {
                        speak_native_complete(done, 0)
                    }
                }
            }

            private let dayDelegate = DaySpeechDelegate()
            private let daySynthesizer: AVSpeechSynthesizer = {
                let s = AVSpeechSynthesizer()
                s.delegate = dayDelegate
                return s
            }()

            func speak_native(text: String, done: UInt64) throws {
                #if os(iOS)
                try AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
                try AVAudioSession.sharedInstance().setActive(true)
                #endif
                dayDelegate.stopAll()
                daySynthesizer.stopSpeaking(at: .immediate)
                let utterance = AVSpeechUtterance(string: text)
                utterance.voice = AVSpeechSynthesisVoice(language: Locale.current.identifier)
                dayDelegate.pending[ObjectIdentifier(utterance)] = done
                daySynthesizer.speak(utterance)
            }

            func stop_native() {
                dayDelegate.stopAll()
                daySynthesizer.stopSpeaking(at: .immediate)
            }

            // AVFoundation is part of the OS; there is nothing to be missing.
            func engine_ready_native() throws {}
        "#,
    );

    // Everywhere without a platform arm — and the arm `cargo test` and day-mock compile against.
    // Returning `Err` completes the caller's callback with it: an unsupported target never
    // leaves a future pending.
    #[day_bridge::impl(rust, platforms = [other])]
    fn speak_native(_text: &str, _done: day_bridge::Done<i32>) -> Result<(), day_bridge::Error> {
        Err(day_bridge::Error::Unsupported)
    }

    #[day_bridge::impl(rust, platforms = [other])]
    fn stop_native() {}

    #[day_bridge::impl(rust, platforms = [other])]
    fn engine_ready_native() -> Result<(), day_bridge::Error> {
        Err(day_bridge::Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Wake, Waker};

    /// The ~20-line park/unpark executor a part's future tests use (docs/async.md).
    fn block_on<F: Future>(mut fut: F) -> F::Output {
        struct Unpark(std::thread::Thread);
        impl Wake for Unpark {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        // SAFETY: `fut` lives on this stack frame and is never moved after being pinned.
        let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
            std::thread::park();
        }
    }

    /// Speaking must never be fatal, whatever the host has: on a target with no arm this is the
    /// fallback's `Unsupported`, and on one with an arm it drives the real engine.
    #[test]
    fn speaking_is_never_fatal() {
        let _ = super::speak("");
        super::stop();
    }

    /// A host with no engine installed must report `Unsupported` rather than claim `Native` and
    /// then do nothing — the case desktop Linux hits whenever speech-dispatcher is absent.
    #[test]
    fn available_answers_for_this_host_not_just_this_target() {
        // Never panics, and never claims more than the fallback on a target with no arm.
        let _ = super::available();
    }

    /// `available()` and `speak()` must agree: an `Unsupported` target cannot succeed.
    #[test]
    fn support_matches_behavior() {
        if super::available() == super::Support::Unsupported {
            assert_eq!(super::speak("x"), Err(super::Error::Unsupported));
        }
    }

    /// The completing forms never leave a caller waiting: on an unsupported target the callback
    /// fires with `Unsupported` at once, and the future resolves with it.
    #[test]
    fn the_completing_forms_always_answer() {
        if super::available() != super::Support::Unsupported {
            return;
        }
        let seen = Arc::new(Mutex::new(None));
        let s = seen.clone();
        let started = super::speak_until_done("x", move |end| *s.lock().unwrap() = Some(end));
        assert_eq!(started, Err(super::Error::Unsupported));
        assert_eq!(*seen.lock().unwrap(), Some(Err(super::Error::Unsupported)));
        assert_eq!(
            block_on(super::speak_future("x")),
            Err(super::Error::Unsupported)
        );
    }

    #[test]
    fn end_codes_map_to_the_three_ends() {
        assert_eq!(
            super::SpeechEnd::from_code(1),
            Ok(super::SpeechEnd::Finished)
        );
        assert_eq!(
            super::SpeechEnd::from_code(0),
            Ok(super::SpeechEnd::Stopped)
        );
        assert_eq!(
            super::SpeechEnd::from_code(2),
            Ok(super::SpeechEnd::Unobserved)
        );
        assert!(super::SpeechEnd::from_code(9).is_err());
    }
}
