<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Speech Demo

The demo and on-device test app for [`day-part-speech`](..): one page with the support the part
reports for this host, a line to say, Speak, and Stop. It depends on the part by path
(`day-part-speech = { path = ".." }`), so a change to the part and a change here land in one pull
request and one CI run.

## Run it

```sh
day doctor                                            # the toolchains for the targets below
day launch -p macos-appkit --script dayscript/speech.yaml
day launch -p ios-uikit --script dayscript/speech.yaml
day launch -p android-mdc --script dayscript/speech.yaml
```

The script is the test: it asserts the support label, drives both bridged calls, and captures two
screenshots under `build/day/screenshots/<target>/`. CI runs exactly this on macOS, the iOS
Simulator, the Android emulator, headless WebKit, and the HarmonyOS emulator ([../.github/workflows/ci.yml](../.github/workflows/ci.yml)).
The voice itself is the one thing a script cannot hear.

## Build against a local day

No `Cargo.lock` is committed: the first build resolves day at the tip of `main`, and `cargo
update` moves it there again. To build against a checkout of day (or of the part) instead:

```sh
day patch --local ../../day             # writes .cargo/config.toml, gitignored
day patch --check                       # every day crate now resolves from the checkout
```

Delete `.cargo/config.toml` to go back to the git dependency. The lock is gitignored, so a
patched build cannot leave the checkout's paths behind for anyone else.
