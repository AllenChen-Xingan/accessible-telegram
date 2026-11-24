<!-- 草稿 · P2 P3 P4 · ggpui 2026-11-14 · 未发。发前人改定；「VoiceOver …」的句子未经人试，见 README -->

# macOS: expose `expanded`, `position_in_set` / `size_of_set`, and `keyboard_shortcut`

## Summary

Three node properties that AccessKit already models are not exposed by the macOS adapter, although each has an established NSAccessibility mapping that Chromium uses. Two of them (`expanded`, `position_in_set` / `size_of_set`) are already mapped by the Windows adapter, so macOS is currently the odd one out.

| Property | Windows adapter | AT-SPI adapter | macOS adapter (0.26.3 and `main`) | Chromium on macOS |
|---|---|---|---|---|
| `expanded` | ExpandCollapse pattern (`adapters/windows/src/node.rs`, `expand_collapse_state`; line 717 in the 0.34.0 release) | not mapped (no `expanded` reference in `adapters/atspi-common/src`) | not mapped | `isAccessibilityExpanded` → `NSAccessibilityExpandedAttribute` (`ui/accessibility/platform/ax_platform_node_cocoa.mm`, attribute table) |
| `position_in_set` / `size_of_set` | `UIA_PositionInSetPropertyId` / `UIA_SizeOfSetPropertyId` (`adapters/windows/src/node.rs`; lines 681-690 and 1325-1326 in 0.34.0) | mapped (`adapters/atspi-common/src/node.rs`, `position_in_set` / `size_of_set`, lines 395-401 on `main`) | not mapped | `AXARIAPosInSet` / `AXARIASetSize`, only on item-like roles and only when explicitly set (`ax_platform_node_cocoa.mm`, `-AXARIAPosInSet`, `-AXARIASetSize`) |
| `keyboard_shortcut` | not mapped | not mapped | not mapped | `AXKeyShortcutsValue` (`ax_platform_node_cocoa.mm`, `-AXKeyShortcutsValue`) |

(Sources checked: `accesskit_macos` 0.26.3 and `main`; `accesskit_windows` 0.34.0 and `main`; `adapters/atspi-common` on `main`. A search for these property names in `adapters/macos/src` returns nothing. Chromium references are to `main` as of 2026-11-14; line numbers will drift.)

## Why it matters

We are building a VoiceOver-first desktop app on GPUI (which uses AccessKit). Its message and conversation lists are virtualized: only the visible rows exist in the tree, so the platform cannot count siblings to say "item 501 of 1000". The app knows the real position and sets `position_in_set` / `size_of_set`, but they never reach VoiceOver. A comment next to Chromium's mapping says that exposing these attributes makes VoiceOver use its ARIA code path instead of counting children, and that WebKit only exposes them when they are explicitly set. If that holds for native apps too, it is exactly what a virtualized list needs; we have not yet verified it by listening. (For our own app the main consumer is automation that reads the macOS accessibility tree; our VoiceOver users asked not to hear positions, so we would not rely on VoiceOver speaking them.)

`expanded` is needed for disclosure-style rows (e.g. "show details" on a message), and exposing `keyboard_shortcut` would let assistive technology surface an app-specific shortcut without the user opening documentation (whether VoiceOver speaks `AXKeyShortcutsValue` for native apps is something we still need to check by listening).

## Proposed implementation (macOS)

All three live in `adapters/macos/src/node.rs`:

1. **`expanded`**: add `isAccessibilityExpanded` returning the node's expanded state, and allow the selector in `isAccessibilitySelectorAllowed:` only when the property is set. Open question for maintainers: Chromium also posts an expanded-changed notification when the state flips; should the adapter do the same, or keep the event set minimal?
2. **`position_in_set` / `size_of_set`**: serve `AXARIAPosInSet` and `AXARIASetSize` through the existing `accessibilityAttributeValue:` override (the same place that already serves `AXBrailleLabel` / `AXBrailleRoleDescription`). That override currently returns `Option<Id<NSString>>`, so it would need to return a general object (`NSObject`) to hand back an `NSNumber`. Follow Chromium/WebKit: only for item-like roles (`NodeWrapper::is_item_like`) and only when the author set the property, so plain lists keep VoiceOver's native counting. Extend the `accessibilityAttributeValue:` gate in `isAccessibilitySelectorAllowed:` accordingly.
3. **`keyboard_shortcut`**: serve `AXKeyShortcutsValue` through the same override, as an `NSString`.

The attribute-name strings are not public AppKit constants; Chromium and WebKit define them as string literals, which is also how the braille attributes are handled today.

## Platform parity

- Windows already maps `expanded` and the set properties. AT-SPI maps the set properties.
- For `expanded` on AT-SPI (`State::Expandable` / `State::Expanded`) and `keyboard_shortcut` on Windows (`AcceleratorKey`) and AT-SPI, I can include them in the same change if you prefer to land the three properties on all platforms together.

## Tests

Following `adapters/macos/tests/integration`: build a small tree with a `ListItem` that has `position_in_set = 3`, `size_of_set = 1000`, `expanded = false`, `keyboard_shortcut = "R"`; query the platform node and assert the four attribute values, and assert that a `ListItem` *without* the properties does not expose `AXARIAPosInSet` (so native counting is preserved). Then flip `expanded` and assert the notification, if we add one.

## What I will verify before opening a PR

- Listen with VoiceOver whether "3 of 1000" is announced on a virtualized list row with the attributes set, and that a plain list is unchanged; check that an accessibility client reading the macOS tree gets the values.
- MSRV 1.87, `cargo test -p accesskit_macos`, Conventional Commit title (`feat: Expose expanded, set position and keyboard shortcuts on macOS`).
