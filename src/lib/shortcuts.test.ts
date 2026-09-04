import { describe, expect, it } from "vitest";
import { DEFAULT_KEYBOARD_SHORTCUTS, formatShortcut, loadKeyboardShortcuts, matchesShortcut } from "./shortcuts";

describe("keyboard shortcuts", () => {
  it("uses Command-I for the chat sidebar", () => {
    expect(formatShortcut(DEFAULT_KEYBOARD_SHORTCUTS.toggleAgent)).toBe("⌘I");
    const event = { key: "i", metaKey: true, shiftKey: false, altKey: false, ctrlKey: false } as KeyboardEvent;
    expect(matchesShortcut(event, DEFAULT_KEYBOARD_SHORTCUTS.toggleAgent)).toBe(true);
  });

  it("falls back safely when stored shortcuts are invalid", () => {
    const shortcuts = loadKeyboardShortcuts({ getItem: () => "not json" });
    expect(shortcuts).toEqual(DEFAULT_KEYBOARD_SHORTCUTS);
  });
});
