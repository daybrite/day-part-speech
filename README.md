<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# day-part-speech

[![ci](https://github.com/daybrite/day-part-speech/actions/workflows/ci.yml/badge.svg)](https://github.com/daybrite/day-part-speech/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MPL--2.0-green.svg)](LICENSE)

Text to speech in [Day](https://daybrite.dev) apps, through each platform's own engine: one Rust
API, and the platform's voice underneath.

The part is headless. It draws nothing, and its whole implementation is one file: a declaration
of three functions, and an arm for every platform written in that platform's language, which is
what makes it [daybridge](https://daybrite.dev/docs/internal/bridge)'s reference sample.
`AVSpeechSynthesizer` in Swift on macOS and iOS, `TextToSpeech` in Java on Android, Core Speech
Kit in ArkTS on HarmonyOS, `speechSynthesis` in JavaScript on the web, SAPI in C++ on Windows,
speech-dispatcher in C on Linux. `day build` generates the glue on both sides and compiles each
arm into the app that uses it.

## Use it

Add the dependency and call it. There is nothing to configure per platform.

```toml
[dependencies]
day-part-speech = { git = "https://github.com/daybrite/day-part-speech.git" }
```

```rust
if day_part_speech::available() != day_bridge::Support::Unsupported {
    day_part_speech::speak("A clear day, with a chance of rain later.")?;
}
day_part_speech::stop();
```

| Function | Meaning |
|---|---|
| `speak(&str) -> Result<(), Error>` | say it, interrupting anything already speaking |
| `stop()` | stop immediately; a silent no-op where speech is unsupported |
| `available() -> Support` | `Native`, `Emulated`, or `Unsupported`, for this host rather than just this target |

`Ok` from `speak` means the platform accepted the utterance, not that it finished; nothing reports
completion. `available()` asks the arm at run time as well as the compiler at build time, so a
Linux desktop without speech-dispatcher reports `Unsupported` instead of `Native` followed by
silence, and the app starts either way, because that arm loads the library on first use rather
than linking it.

| Platform | Engine | Arm | Support |
|---|---|---|---|
| macOS, iOS | `AVSpeechSynthesizer` | Swift | Native |
| Android | `TextToSpeech` | Java | Native |
| HarmonyOS | Core Speech Kit | ArkTS | Emulated (zh-CN voices) |
| Web | `speechSynthesis` | JavaScript | Native, where the browser has it |
| Windows | SAPI 5 | C++ | Native |
| Linux | speech-dispatcher | C | Native when installed, else Unsupported |
| everywhere else | | Rust | Unsupported |

[docs/speech.md](docs/speech.md) walks through the arms, and is the page to read before writing a
bridge of your own.

## Compatibility

| This crate | Tested against day | Targets |
|---|---|---|
| 0.1 | 0.4 (`main` at the revision in `demo/Cargo.lock`) | every target with an arm above |

Every day dependency names the bare canonical URL with no branch or tag, and your app's
`Cargo.lock` picks one day revision for the whole graph. Cargo unifies a git dependency only when
URL and ref match, so a crate that pinned a tag would double every day crate in an app on `main`.
`[package.metadata.day] compat = "0.4"` records the minor this release was tested against, and
`day build` notes a mismatch before compiling.

To build against a fork of day, patch the canonical URL once in your app and this crate follows:

```sh
day patch --git https://github.com/acme/day.git@acme
```

## Develop it

```sh
cargo test                                            # the Rust side, with this host's arm
cd demo && day launch -p macos-appkit --script dayscript/speech.yaml
cd demo && day launch -p ios-uikit --script dayscript/speech.yaml
cd demo && day launch -p android-mdc --script dayscript/speech.yaml
```

The [demo app](demo/) depends on this crate by path, and its walkthrough drives both bridged calls
on device; CI runs it on macOS, the iOS Simulator, the Android emulator, headless WebKit, and
the HarmonyOS emulator on every push, and daily against day's newest `main`. Hearing the voice is the acceptance test, and it needs a
person. To work against a local day checkout, `day patch --local ../day` in either directory
writes a gitignored patch table.

Extending Day is documented at [daybrite.dev/docs/extending](https://daybrite.dev/docs/extending);
parts in particular at [daybrite.dev/docs/parts](https://daybrite.dev/docs/parts).

## Part of Day

This crate is one part of [Day](https://daybrite.dev), a Rust framework for building apps out of
each platform's own widgets — AppKit, UIKit, Android's Material widgets, GTK 4, Qt 6, XAML, and
ArkUI — from one codebase. When you write `button("Save")`, macOS shows an `NSButton` and Android
shows a Material button.

New to Day? Start at [daybrite.dev](https://daybrite.dev), or browse the
[source repository](https://github.com/daybrite/day).
