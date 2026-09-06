// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Speech Demo — the demo and on-device test app for `day-part-speech`.
//!
//! One page: what the part reports for this host, a line to say, Speak, and Stop. Every element
//! carries a stable id, so `dayscript/speech.yaml` can drive both bridged calls on macOS, the iOS
//! Simulator, and the Android emulator — which is how the crate's CI proves that each platform's
//! arm links into a real app and answers. Hearing the voice is the part a script cannot check.

use day::prelude::*;

// The mobile entry point; a plain cargo desktop build enters through src/main.rs.
day::day_start!(options: window(), root);

// Typed constants for everything under `resource/` (https://daybrite.dev/docs/resources).
day::resources!();

/// The window every entry point opens.
pub fn window() -> day::WindowOptions {
    day::WindowOptions {
        locales: Some((res::locales::DEFAULT, res::locales::CATALOG)),
        title_fn: Some(|| res::str::app_title().format()),
        size: day::prelude::Size::new(480.0, 520.0),
        ..Default::default()
    }
}

/// The whole app: title, the part's answer for this host, the phrase, and the two calls.
pub fn root() -> impl Piece {
    info!("Speech Demo starting");

    // `available()` asks the compiler which arm this target has AND asks that arm at run time
    // whether it can reach an engine, so the label is the answer for this machine: Unsupported on
    // a Linux desktop without speech-dispatcher, Emulated on HarmonyOS, Native elsewhere.
    let support = day_part_speech::available();
    let support_text = match support {
        Support::Native => res::str::support_native(),
        Support::Emulated => res::str::support_emulated(),
        Support::Unsupported => res::str::support_unsupported(),
    };

    // Empty means "say the sample", which is exactly what the placeholder shows.
    let phrase = Signal::new(String::new());

    // A scroll view rather than a bare column: on macOS the window's content runs under the
    // unified title bar, and AppKit insets a scroll view below it; on a phone it is what a page
    // wants anyway.
    scroll(
        column((
            label(res::str::app_title())
                .font(Font::Title)
                .id("speech-title"),
            label(res::str::caption()).font(Font::Footnote),
            section((
                labeled(
                    res::str::support_label(),
                    label(support_text).id("speech-support"),
                ),
                text_field(phrase)
                    .placeholder(res::str::phrase())
                    .id("speech-text"),
                row((
                    button(res::str::speak())
                        .action(move || {
                            let typed = phrase.with(|t| t.trim().to_string());
                            let text = if typed.is_empty() {
                                res::str::phrase().format()
                            } else {
                                typed
                            };
                            // An `Err` here is the fallback arm answering, which the support label
                            // above already said. A v1 bridge call is one-shot: nothing reports when
                            // the voice finishes, so there is no progress readout to keep honest.
                            let _ = day_part_speech::speak(&text);
                        })
                        .id("speech-speak"),
                    button(res::str::stop())
                        .bordered()
                        .action(day_part_speech::stop)
                        .id("speech-stop"),
                ))
                .spacing(8.0),
            ))
            .title(res::str::section()),
        ))
        .spacing(12.0)
        .padding(16.0),
    )
}
