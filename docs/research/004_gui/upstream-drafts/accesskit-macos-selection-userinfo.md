<!-- 草稿 · P12 · ggpui 2026-11-14 · 未发。**发前必须先人试**：做一个带 TextRun 的输入框，开 VO 按 ←/→、Option+←/→、输入几个字，听读不读字 / 词。问题不存在就不发。 -->

# macOS: add text-state userInfo to `AXSelectedTextChanged` (and `AXValueChanged` for edits)

## Summary

When a text node's selection changes, the macOS adapter posts `NSAccessibilitySelectedTextChangedNotification` with no userInfo (`adapters/macos/src/event.rs`, the `raw_text_selection` comparison in the node-updated handler). WebKit and Chromium attach a userInfo dictionary describing *what kind* of change happened (type, direction, granularity). As we understand it from those implementations and from the Mozilla discussion below, VoiceOver uses this to decide whether to speak the character, word or line the caret moved over, and to echo typed characters.

[TO VERIFY BY LISTENING BEFORE FILING: with the current adapter, moving the caret in a GPUI text field with VoiceOver on — does VoiceOver speak the character / word? Describe what it actually says.]

## Prior art

- WebKit, `Source/WebCore/accessibility/mac/AXObjectCacheMac.mm` (WebCore-7611.2.7.0.4 on opensource.apple.com; I still need to re-read it line by line before filing): adds `AXTextStateChangeType` (edit / selection move / selection extend) and, for moves and extends, `AXTextSelectionDirection` and `AXTextSelectionGranularity` (character, word, line, …); for edits, `AXTextChangeValues` with `AXTextEditType` and the inserted / deleted text.
- Chromium, `content/browser/accessibility/browser_accessibility_manager_mac.mm` (older revision `0d26e79ae48f`; we have not located the current equivalent, and I still need to read it line by line before filing): appears to build the same keys for selection-changed and value-changed notifications.
- Firefox: Mozilla bugs 1653421 and 1686995 discuss the same keys; from my reading of the summary, the latter says VoiceOver needs the edit type to echo typed keys and cannot otherwise distinguish typing from pasting (to re-read in full before filing).

These keys are not documented AppKit API; all three engines use string literals.

## Proposed implementation

1. In the event generator, when the selection changes, compute the change type from the old and new `TextSelection`:
   - anchor == focus before and after → *selection move*; direction from comparing positions (next / previous); granularity is the hard part (see 3)
   - anchor unchanged, focus moved → *selection extend*
   - the node's text also changed in the same update → *edit*, with the inserted / deleted string from diffing the old and new runs
2. Post `AXSelectedTextChanged` (and `AXValueChanged` for edits) with `NSAccessibilityPostNotificationWithUserInfo`.
3. **Granularity cannot be inferred reliably from two positions.** Options: (a) add an optional hint to the tree update / node (e.g. "last selection change was by word") that the app sets when it handles the key; (b) infer: one character → character, landing on a `word_starts` boundary → word, on a line boundary → line, else unknown. WebKit gets it from the editing command. I'd like the maintainers' view on whether a schema addition (a) is acceptable or (b) is preferred.

## Platform parity

The Windows adapter raises `UIA_Text_TextSelectionChangedEventId` on selection changes (`adapters/windows/src/node.rs`, line 766 in 0.34.0) and the AT-SPI adapter emits `ObjectEvent::CaretMoved` (`adapters/atspi-common/src/adapter.rs` on `main`); on those platforms the screen reader derives the rest. This issue only fills the macOS-specific payload.

## Tests

Integration tests that apply a tree update moving a caret by one character, by a word, and extending a selection; capture the posted notifications (the adapter's queued events) and assert type / direction / granularity; and an edit inserting text asserts the edit type and inserted string.
