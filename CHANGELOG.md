<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Changelog

## 0.1.0

The first release from its own repository. The crate moved out of `daybrite/day`
(`parts/day-part-speech`) with its history, and [docs/speech.md](docs/speech.md) came with it.

- Tested against day 0.4 at the revision in `demo/Cargo.lock`.
- New: the `demo/` app and its `dayscript/speech.yaml`, run by CI on macOS, the iOS Simulator,
  the Android emulator, headless WebKit, and the HarmonyOS emulator.
- Unchanged: `speak`, `stop`, `available`, and the seven arms.
