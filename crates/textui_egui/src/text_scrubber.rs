//! A global last-chance filter for text drawn through [`TextUiEguiExt`](super::TextUiEguiExt).
//!
//! Every label, button, tooltip, markdown/code block and rich text viewer passes its text
//! through the installed scrubber before it is measured or drawn, whatever produced the text.
//! That is how the launcher can promise that, for example, account names never appear in the UI
//! in streamer mode even in strings it did not write itself (game output, error messages).
//!
//! Text typed into input fields is never scrubbed: it is the user's own.

use std::borrow::Cow;
use std::cell::Cell;
use std::sync::{Arc, RwLock};

use textui::RichTextSpan;

/// Returns the replacement for `text`, or `None` to leave it as is.
pub type TextScrubber = dyn Fn(&str) -> Option<String> + Send + Sync;

static SCRUBBER: RwLock<Option<Arc<TextScrubber>>> = RwLock::new(None);

thread_local! {
    static EXEMPT_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Ends one level of exemption when dropped, even if the exempt code panics.
struct ExemptionGuard;

impl Drop for ExemptionGuard {
    fn drop(&mut self) {
        EXEMPT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Runs `draw` with name scrubbing switched off for the text drawn inside it, for UI that is
/// static launcher text and cannot contain an account name (a settings label, a button
/// reading "Cancel"). It saves the scrubber from needlessly rewriting that text, and from
/// mistaking an ordinary word for an account that happens to be named after it.
///
/// This is a promise by the caller. Never wrap anything that can show data the user or a
/// game produced. The scrubber also still resolves account mentions inside the scope; only
/// the guessing of names is skipped. Scopes nest, apply to this thread only, and need no
/// cleanup.
pub fn without_name_scrubbing<R>(draw: impl FnOnce() -> R) -> R {
    EXEMPT_DEPTH.with(|depth| depth.set(depth.get() + 1));
    let _guard = ExemptionGuard;
    draw()
}

/// Whether text is being drawn inside [`without_name_scrubbing`].
pub fn name_scrubbing_exempt() -> bool {
    EXEMPT_DEPTH.with(|depth| depth.get() > 0)
}

/// Installs (or with `None`, removes) the scrubber applied to all drawn text.
pub fn set_text_scrubber(scrubber: Option<Arc<TextScrubber>>) {
    if let Ok(mut slot) = SCRUBBER.write() {
        *slot = scrubber;
    }
}

fn current() -> Option<Arc<TextScrubber>> {
    SCRUBBER.read().ok().and_then(|slot| slot.clone())
}

/// `text` as it will be shown: what the UI draws for it with the current scrubber.
pub fn scrubbed(text: &str) -> Cow<'_, str> {
    scrub(text)
}

/// `text` as it should be shown.
pub(crate) fn scrub(text: &str) -> Cow<'_, str> {
    match current().and_then(|scrubber| scrubber(text)) {
        Some(replacement) => Cow::Owned(replacement),
        None => Cow::Borrowed(text),
    }
}

/// `spans` as they should be shown. Each span is scrubbed on its own; if the text as a whole
/// would still be changed (something to hide straddles two spans), the spans are replaced by one
/// scrubbed span in the first span's style, losing the other colors on that run but never
/// showing the hidden text.
pub(crate) fn scrub_spans(spans: &[RichTextSpan]) -> Cow<'_, [RichTextSpan]> {
    let Some(scrubber) = current() else {
        return Cow::Borrowed(spans);
    };
    let scrubbed: Vec<Option<String>> = spans.iter().map(|span| scrubber(&span.text)).collect();
    let straddling = || {
        let joined: String = spans.iter().map(|span| span.text.as_str()).collect();
        let joined_scrubbed = scrubber(&joined)?;
        let piecewise: String = spans
            .iter()
            .zip(&scrubbed)
            .map(|(span, replacement)| replacement.as_deref().unwrap_or(span.text.as_str()))
            .collect();
        (joined_scrubbed != piecewise).then_some(joined_scrubbed)
    };
    if spans.len() > 1
        && let Some(whole) = straddling()
    {
        return Cow::Owned(vec![RichTextSpan {
            text: whole,
            style: spans[0].style.clone(),
        }]);
    }
    if scrubbed.iter().all(Option::is_none) {
        return Cow::Borrowed(spans);
    }
    Cow::Owned(
        spans
            .iter()
            .zip(scrubbed)
            .map(|(span, replacement)| match replacement {
                Some(text) => RichTextSpan {
                    text,
                    style: span.style.clone(),
                },
                None => span.clone(),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    /// Every method of the UI text API that takes text to draw must scrub it. This reads the
    /// implementation's source, so a method added later without `scrub` fails here.
    #[test]
    fn every_text_drawing_method_scrubs_its_text() {
        let source = include_str!("ext_impl.rs");
        let body = source
            .split_once("impl TextUiEguiExt for TextUi {")
            .expect("impl block")
            .1;
        let mut unscrubbed = Vec::new();
        for method in body.split("\n    fn ").skip(1) {
            let name = method.split(['<', '(']).next().unwrap_or("?");
            let signature = method.split("{\n").next().unwrap_or("");
            let takes_drawn_text = ["text: &str", "code: &str", "markdown: &str"]
                .iter()
                .any(|param| signature.contains(param))
                || signature.contains("spans: &[RichTextSpan]");
            let scrubs = method.contains("scrub(") || method.contains("scrub_spans(");
            if takes_drawn_text && !scrubs {
                unscrubbed.push(name.to_owned());
            }
        }
        assert!(
            unscrubbed.is_empty(),
            "these text methods do not scrub their text: {unscrubbed:?}"
        );
    }

    use super::*;
    use std::sync::Mutex;
    use textui::RichTextStyle;

    // The scrubber is process-global, so tests that install one must not overlap.
    static LOCK: Mutex<()> = Mutex::new(());

    fn install_hiding(word: &'static str) {
        set_text_scrubber(Some(Arc::new(move |text: &str| {
            text.contains(word).then(|| text.replace(word, "###"))
        })));
    }

    fn span(text: &str) -> RichTextSpan {
        RichTextSpan {
            text: text.to_owned(),
            style: RichTextStyle::default(),
        }
    }

    #[test]
    fn exemption_scopes_nest_and_always_end() {
        assert!(!name_scrubbing_exempt());
        without_name_scrubbing(|| {
            assert!(name_scrubbing_exempt());
            without_name_scrubbing(|| assert!(name_scrubbing_exempt()));
            assert!(name_scrubbing_exempt(), "the outer scope is still open");
        });
        assert!(!name_scrubbing_exempt());
    }

    #[test]
    fn a_panic_inside_an_exempt_scope_does_not_leave_it_open() {
        let result = std::panic::catch_unwind(|| {
            without_name_scrubbing(|| panic!("draw failed"));
        });
        assert!(result.is_err());
        assert!(!name_scrubbing_exempt());
    }

    #[test]
    fn exemption_is_per_thread() {
        without_name_scrubbing(|| {
            let other_thread = std::thread::spawn(name_scrubbing_exempt).join().unwrap();
            assert!(!other_thread);
        });
    }

    #[test]
    fn exemption_returns_the_closures_value() {
        assert_eq!(without_name_scrubbing(|| 7), 7);
    }

    #[test]
    fn text_is_untouched_without_a_scrubber() {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_text_scrubber(None);
        assert!(matches!(scrub("Steve"), Cow::Borrowed("Steve")));
        let input = [span("Steve")];
        assert!(matches!(scrub_spans(&input), Cow::Borrowed(_)));
    }

    #[test]
    fn installed_scrubber_rewrites_text() {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        install_hiding("Steve");
        assert_eq!(scrub("hi Steve!"), "hi ###!");
        assert!(matches!(scrub("nothing here"), Cow::Borrowed(_)));
        set_text_scrubber(None);
    }

    #[test]
    fn spans_are_scrubbed_individually() {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        install_hiding("Steve");
        let input = [span("[INFO] "), span("Steve joined")];
        let scrubbed = scrub_spans(&input);
        let texts: Vec<&str> = scrubbed.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(texts, vec!["[INFO] ", "### joined"]);
        set_text_scrubber(None);
    }

    #[test]
    fn a_name_split_across_spans_is_still_hidden() {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        install_hiding("Steve");
        let input = [span("Ste"), span("ve left")];
        let scrubbed = scrub_spans(&input);
        let shown: String = scrubbed.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(shown, "### left");
        set_text_scrubber(None);
    }
}
