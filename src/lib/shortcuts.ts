export type ShortcutAction = "toggleProjects" | "toggleAgent" | "toggleTimeline" | "undo" | "redo";

export interface KeyboardShortcut {
  key: string;
  meta: boolean;
  shift: boolean;
  alt: boolean;
  ctrl: boolean;
}

export type KeyboardShortcuts = Record<ShortcutAction, KeyboardShortcut>;

export const DEFAULT_KEYBOARD_SHORTCUTS: KeyboardShortcuts = {
  toggleProjects: { key: "b", meta: true, shift: false, alt: false, ctrl: false },
  toggleAgent: { key: "i", meta: true, shift: false, alt: false, ctrl: false },
  toggleTimeline: { key: "j", meta: true, shift: false, alt: false, ctrl: false },
  undo: { key: "z", meta: true, shift: false, alt: false, ctrl: false },
  redo: { key: "z", meta: true, shift: true, alt: false, ctrl: false },
};

export const SHORTCUT_STORAGE_KEY = "open-editor.keyboard-shortcuts.v1";

export function shortcutFromEvent(event: KeyboardEvent): KeyboardShortcut | undefined {
  const key = event.key.toLowerCase();
  if (["meta", "shift", "alt", "control"].includes(key)) return undefined;
  return { key, meta: event.metaKey, shift: event.shiftKey, alt: event.altKey, ctrl: event.ctrlKey };
}

export function matchesShortcut(event: KeyboardEvent, shortcut: KeyboardShortcut): boolean {
  return event.key.toLowerCase() === shortcut.key && event.metaKey === shortcut.meta &&
    event.shiftKey === shortcut.shift && event.altKey === shortcut.alt && event.ctrlKey === shortcut.ctrl;
}

export function formatShortcut(shortcut: KeyboardShortcut): string {
  const keyNames: Record<string, string> = { arrowleft: "←", arrowright: "→", arrowup: "↑", arrowdown: "↓", escape: "Esc", " ": "Space" };
  return `${shortcut.ctrl ? "⌃" : ""}${shortcut.alt ? "⌥" : ""}${shortcut.shift ? "⇧" : ""}${shortcut.meta ? "⌘" : ""}${keyNames[shortcut.key] ?? shortcut.key.toUpperCase()}`;
}

export function loadKeyboardShortcuts(storage: Pick<Storage, "getItem"> = window.localStorage): KeyboardShortcuts {
  try {
    const saved = JSON.parse(storage.getItem(SHORTCUT_STORAGE_KEY) ?? "{}") as Partial<KeyboardShortcuts>;
    return Object.fromEntries(Object.entries(DEFAULT_KEYBOARD_SHORTCUTS).map(([action, fallback]) => {
      const candidate = saved[action as ShortcutAction];
      const valid = candidate && typeof candidate.key === "string" && ["meta", "shift", "alt", "ctrl"].every((modifier) => typeof candidate[modifier as keyof KeyboardShortcut] === "boolean");
      return [action, valid ? candidate : fallback];
    })) as KeyboardShortcuts;
  } catch {
    return structuredClone(DEFAULT_KEYBOARD_SHORTCUTS);
  }
}

