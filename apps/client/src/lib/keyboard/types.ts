// Keyboard shortcut system types

export interface KeyboardShortcut {
  key: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  action: () => void;
  description: string;
  // Priority: higher numbers take precedence
  priority?: number;
  // Whether this shortcut should prevent default browser behavior
  preventDefault?: boolean;
}

export interface KeyboardContext {
  name: string;
  shortcuts: KeyboardShortcut[];
  // Whether this context is currently active
  active: boolean;
}

// Platform-aware modifier key helper
export const isMac =
  typeof navigator !== "undefined" &&
  navigator.platform.toUpperCase().indexOf("MAC") >= 0;
export const cmdOrCtrl = (event: KeyboardEvent) =>
  isMac ? event.metaKey : event.ctrlKey;

// Helper to create platform-aware shortcuts
export const createShortcut = (
  key: string,
  action: () => void,
  description: string,
  options: {
    cmd?: boolean;
    ctrl?: boolean;
    shift?: boolean;
    alt?: boolean;
    priority?: number;
    preventDefault?: boolean;
  } = {}
): KeyboardShortcut => {
  const {
    cmd = false,
    ctrl = false,
    shift = false,
    alt = false,
    priority = 0,
    preventDefault = true
  } = options;

  return {
    key: key.toLowerCase(),
    metaKey: cmd,
    ctrlKey: ctrl,
    shiftKey: shift,
    altKey: alt,
    action,
    description,
    priority,
    preventDefault
  };
};

// Helper to check if a keyboard event matches a shortcut
export const matchesShortcut = (
  event: KeyboardEvent,
  shortcut: KeyboardShortcut
): boolean => {
  return (
    event.key.toLowerCase() === shortcut.key &&
    !!event.metaKey === !!shortcut.metaKey &&
    !!event.ctrlKey === !!shortcut.ctrlKey &&
    !!event.shiftKey === !!shortcut.shiftKey &&
    !!event.altKey === !!shortcut.altKey
  );
};
