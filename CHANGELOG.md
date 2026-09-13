<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Changelog

## Unreleased

- New: `speak_until_done` and `speak_future` report when an utterance ends, through the bridge's
  callback tier (day's `docs/bridge.md` "Callbacks"). `speak_native` now carries a
  `day_bridge::Done<i32>`; every arm completes it from its engine's own end-of-utterance event
  (`AVSpeechSynthesizerDelegate`, `UtteranceProgressListener`, SAPI's end-of-stream event,
  `SpeechSynthesisUtterance.onend`), and the Linux arm reports `SpeechEnd::Unobserved`.
- Changed: `stop` answers a pending `speak_future` with `SpeechEnd::Stopped`, on Apple even when
  the utterance had not started yet.
- Changed: the HarmonyOS arm is reached for the first time — the bridge gained its ArkTS half —
  and completes through Core Speech Kit's `SpeakListener`.
- Fixed: the Windows arm compiles again. It called `SpClearEvent`, an inline from `sphelper.h`,
  without including that header; it now releases each SAPI event's `lParam` itself, the same way.
- Needs day 0.4.3 or newer: `day-bridge` gained the callback tier and `day-async`.

## 0.1.0

The first release from its own repository. The crate moved out of `daybrite/day`
(`parts/day-part-speech`) with its history, and [docs/speech.md](docs/speech.md) came with it.

- Tested against day 0.4 at the revision in `demo/Cargo.lock`.
- New: the `demo/` app and its `dayscript/speech.yaml`, run by CI on macOS, the iOS Simulator,
  the Android emulator, headless WebKit, and the HarmonyOS emulator.
- Unchanged: `speak`, `stop`, `available`, and the seven arms.
