<!-- 草稿 · P1 · ggpui 2026-11-14 · 未发。发前人改定；先等 P2/P3/P4 那份拿到回应 -->

# macOS: expose `custom_actions` as `NSAccessibilityCustomAction`

## Summary

`Node::custom_actions` / `Action::CustomAction` exist in the schema (`accesskit/src/lib.rs`: `struct CustomAction`, `set_custom_actions`, `Action::CustomAction` with `ActionData::CustomAction`), but no adapter consumes them: `custom_action` does not appear in `adapters/macos`, `adapters/windows` or `adapters/atspi-common` on `main`. An app can set them, but nothing reaches `accessibilityCustomActions`, so we expect VoiceOver's Actions rotor (VO-Command-Space) to have nothing to show for them (not yet verified by listening).

## Why it matters

In a chat list each row has secondary actions (reply, forward, delete). On macOS the platform convention for these is `accessibilityCustomActions`, which VoiceOver lists in its Actions rotor. Without it, the only route is app-specific shortcuts that the user has to learn.

## Prior art

Chromium implements `-accessibilityCustomActions` in `ui/accessibility/platform/browser_accessibility_cocoa.mm`: for each action it builds `[[NSAccessibilityCustomAction alloc] initWithName:handler:]`; the handler captures the target id by value, re-resolves the node when invoked (the tree may have changed), returns `NO` if it is gone, and otherwise performs the action. It returns `nil` when there are no actions.

## Proposed implementation (macOS)

In `adapters/macos/src/node.rs`:

1. Add `accessibilityCustomActions`, returning `nil` when the node has no custom actions, otherwise one `NSAccessibilityCustomAction` per `CustomAction`, named by its `description`.
2. The block handler captures `(tree id, node id, custom action id)` by value and, when invoked, re-resolves the node through the adapter context (like `set_value` does today) and calls `context.do_action(ActionRequest { action: Action::CustomAction, data: Some(ActionData::CustomAction(id)), .. })`. Return `NO` if the node no longer exists or no longer has that action.
3. Allow the selector in `isAccessibilitySelectorAllowed:` only when custom actions are present.

This needs Objective-C block support: `accesskit_macos` does not currently depend on `block2` (its dependencies are `objc2`, `objc2-foundation`, `objc2-app-kit`), so this would add one, or use `NSAccessibilityCustomAction`'s target / selector initializer instead. Which is preferred?

## Platform parity

On Windows I have not found a direct UIA equivalent for per-element custom actions; on Linux, AT-SPI's `Action` interface (`get_n_actions` / `do_action`) looks like the natural carrier, but the AT-SPI adapter does not use `custom_actions` today. I am happy to do macOS first and open a follow-up for AT-SPI, or include it — please advise.

## Tests

Integration test: a node with two custom actions; query `accessibilityCustomActions`, assert names and count; invoke the first handler and assert the action handler received `Action::CustomAction` with the right id; remove the node, invoke the stored handler again and assert it returns `NO` without panicking.
