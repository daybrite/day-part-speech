<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# day-part-speech

[![ci](https://github.com/daybrite/day-part-speech/actions/workflows/ci.yml/badge.svg)](https://github.com/daybrite/day-part-speech/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MPL--2.0-green.svg)](LICENSE)

## Overview and capabilities

`day-part-speech` reads text aloud using the device's speech engine through one Rust API.

[Day](https://github.com/daybrite/day) is a Rust framework for building applications
from a shared codebase using each platform's native UI toolkit. A **piece** is a UI
component you place in a layout; a **part** provides a capability without drawing UI.
Day's `day` command builds and packages the Rust code, resources, and native platform
code together. `Cargo.toml` declares Rust dependencies; `Day.toml` configures the app
and its target platforms.

This is a headless part: call it from a button action or another app event; there is
no speech widget to add to the layout. It provides five operations:

| API | Meaning |
|---|---|
| `speak(&str) -> Result<(), Error>` | Request speech, replacing the current utterance. |
| `speak_until_done(&str, on_end)` | The same request, with a callback that fires once when the utterance ends. |
| `speak_future(&str) -> impl Future<Output = Result<SpeechEnd, Error>>` | The same request, to `.await` under `day::task`. |
| `stop()` | Ask the engine to stop; a pending `speak_future` resolves with `SpeechEnd::Stopped`. |
| `available() -> Support` | Report `Native`, `Emulated`, or `Unsupported` using the compiled platform implementation and its runtime probe. |

Speech is asynchronous. `Ok(())` from `speak` means the request was dispatched/accepted, not
that sound was produced or speech completed; the completing forms report the end through the
bridge's callback tier as a `SpeechEnd` (`Finished`, `Stopped`, or `Unobserved` where the engine
reports no end, which is speech-dispatcher on Linux). The API has no voice picker, rate/pitch
controls, audio-file output, or speech recognition.

## Platform support and limitations

| Platform | Engine | Support and constraints |
|---|---|---|
| macOS, iOS | [AVSpeechSynthesizer](https://developer.apple.com/documentation/avfaudio/avspeechsynthesizer) | `Native`; chooses a voice using the current locale. On iOS, speaking configures and activates a playback/spoken-audio session. |
| Android | [Android TextToSpeech](https://developer.android.com/reference/android/speech/tts/TextToSpeech) | `Native` when a Day Android context is present. Engine initialization is asynchronous; the most recent pending utterance is retained until ready. Installed voices/languages determine output. |
| HarmonyOS | [Huawei Core Speech Kit](https://developer.huawei.com/consumer/cn/sdk/core-speech-kit) (Chinese documentation) | `Emulated`; this implementation requests `zh-CN`, not the app's locale. Engine creation is deferred until speaking. |
| Web | [Web Speech API: speech synthesis](https://webaudio.github.io/web-speech-api/#tts-section) | `Native` when the API exists. Voice availability and browser playback policy still determine whether speech is heard. |
| Windows | [Microsoft SAPI / ISpVoice](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee413476%28v%3Dvs.85%29) | `Native` when a COM speech voice can be created. |
| Linux | [Speech Dispatcher](https://github.com/brailcom/speechd) | `Native` when its shared library can be loaded; a working service and voice are also needed to speak. |
| Other targets / fallback | Rust fallback | `Unsupported`; `speak` returns `Error::Unsupported`. |

`available()` is a useful capability check, not an audio self-test. In particular,
Linux probes library loading rather than the service connection, and Android probes
the application context rather than successful voice initialization. Handle errors
from `speak` and test actual output on your target devices. Linux apps can start
without speech-dispatcher installed because it is loaded on demand.

`stop()` does not report completion. On Android it stops the current engine but does
not clear an utterance waiting for initialization. Web callers should guard calls
with `available()` when the browser lacks the speech API.

## Add it to a Day project

Add the crate to your existing app's `Cargo.toml`:

```toml
[dependencies]
day-part-speech = { git = "https://github.com/daybrite/day-part-speech.git" }
```

Call it from an app action. `Error` and `Support` are re-exported, so you do not need
a direct `day-bridge` dependency just to use these types:

```rust
use day_part_speech::{self as speech, Error, Support};

fn read_forecast() -> Result<(), Error> {
    if speech::available() == Support::Unsupported {
        return Err(Error::Unsupported);
    }
    speech::speak("A clear day, with a chance of rain later.")
}

fn stop_reading() {
    if speech::available() != Support::Unsupported {
        speech::stop();
    }
}
```

Surface a returned error in your UI or provide a text-only fallback. Keep the text
visible when speech is unavailable. No speech-specific backend feature, permission
reason, or native dependency entry needs to be added to `Day.toml` by the app.

Build through the Day CLI so the foreign-language implementations are generated and
packaged, for example `day build -p android-mdc` or `day launch -p macos-appkit`, with
that target configured in your app and its SDK installed. The [demo](demo/) contains
a complete app with text entry, Speak/Stop controls, and a support readout. The crate
is designed to run inside Day's platform host, particularly on Android where the
bridge needs Day's application context.

## Architecture and dependencies

Dependency links below lead to upstream source repositories or official API
documentation. Version requirements describe this checkout's [Cargo.toml](Cargo.toml),
not necessarily the newest upstream releases.

[src/lib.rs](src/lib.rs) contains both the public wrapper functions and a
`day_bridge::bridge!` declaration with platform implementations in Swift, Java,
ArkTS, JavaScript, C++, and C. Each implements the same internal speak, stop, and
readiness functions. A Rust fallback keeps unsupported targets buildable.

[build.rs](build.rs) calls `day_build::bridge::generate()`. This emits the Rust glue
and a manifest into Cargo's build output. `day build` uses that manifest to generate
foreign adapters and compile/stage the appropriate platform code. The bridge owns
language conversions, including UTF-16 for Windows. Native engine handles stay in
the language that owns them and are reused across calls.

| Dependency | Role and cost |
|---|---|
| [day-bridge](https://github.com/daybrite/day/tree/main/crates/day-bridge) | The sole unconditional runtime dependency; provides bridge macros, error/support types, and cross-language plumbing. |
| [day-android](https://github.com/daybrite/day/tree/main/toolkits/day-android) | Android-only Rust dependency providing the cached JVM and application context for JNI calls. |
| [day-build](https://github.com/daybrite/day/tree/main/crates/day-build) | Build-time dependency for bridge generation; brings Day's build/code-generation dependencies, not a speech engine bundled in the app. |
| Apple system framework | Swift calls AVFoundation; no third-party SwiftPM speech library. |
| Android, HarmonyOS, web APIs | Platform-provided speech engines; no third-party Gradle speech package or JavaScript speech library declared here. |
| Windows system libraries | C++ uses SDK headers and links `ole32` and `sapi`. |
| Linux runtime library | C uses `dlopen`/`dlsym` for `libspeechd.so.2` (falling back to `libspeechd.so`); no speech-dispatcher headers or link-time `speechd` dependency required. |

The Linux implementation resolves symbols once and lazily opens a speech-dispatcher
connection. Android holds a pending string during initialization. Windows retains an
`ISpVoice`; Apple retains a synthesizer so it outlives the initial function call.
These lifecycle differences explain why identical Rust calls can have different
startup behavior. See [the bridge walkthrough](docs/speech.md) and
[Cargo.toml](Cargo.toml) for implementation and dependency declarations.

## Compatibility and development

This checkout requires Rust 1.89 or newer and declares compatibility with Day 0.4 in
[Cargo.toml](Cargo.toml). The crate is consumed from Git, not crates.io. Its Day
dependencies use `https://github.com/daybrite/day.git` without a branch, tag, or
revision. Use the same source in your app and keep its `Cargo.lock` to record the
resolved revisions. Mixing Day source URLs or refs can introduce duplicate framework
crates and incompatible types.

For a local framework checkout, run `day patch --local ../day` from this repository
(adjust the path when running from `demo/`). The [demo](demo/) depends on this crate
by path and is a complete integration example.

Run `cargo test` for the Rust API checks. From `demo/`, run
`day launch -p macos-appkit --script dayscript/speech.yaml` or choose another target
listed in its `Day.toml`. Automated checks can exercise the calls and UI, but a
listening test is needed to verify pronunciation, voice availability, and output.
