import { useCallback, useEffect, useRef } from "react";
import type { KeyboardShortcut } from "./types";
import { matchesShortcut } from "./types";

// Hook for registering keyboard shortcuts within a component
export const useKeyboardShortcuts = (
  shortcuts: KeyboardShortcut[],
  options: {
    // Whether these shortcuts are currently active
    enabled?: boolean;
    // Context name for debugging
    context?: string;
  } = {}
) => {
  const { enabled = true, context = "unknown" } = options;
  const shortcutsRef = useRef(shortcuts);

  // Update shortcuts ref when they change
  useEffect(() => {
    shortcutsRef.current = shortcuts;
  }, [shortcuts]);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (!enabled) return;

      // Find matching shortcuts, sorted by priority (highest first)
      const matchingShortcuts = shortcutsRef.current
        .filter((shortcut) => matchesShortcut(event, shortcut))
        .sort((a, b) => (b.priority || 0) - (a.priority || 0));

      if (matchingShortcuts.length > 0) {
        const shortcut = matchingShortcuts[0];

        if (shortcut.preventDefault) {
          event.preventDefault();
          event.stopPropagation();
        }

        console.debug(
          `Keyboard shortcut triggered: ${shortcut.description} (${context})`
        );
        shortcut.action();
      }
    },
    [enabled, context]
  );

  useEffect(() => {
    if (!enabled) return;

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown, enabled]);

  return {
    // Utility to check if a specific key combination is registered
    hasShortcut: useCallback(
      (
        key: string,
        modifiers: {
          cmd?: boolean;
          ctrl?: boolean;
          shift?: boolean;
          alt?: boolean;
        } = {}
      ) => {
        return shortcuts.some(
          (shortcut) =>
            shortcut.key === key.toLowerCase() &&
            !!shortcut.metaKey === !!modifiers.cmd &&
            !!shortcut.ctrlKey === !!modifiers.ctrl &&
            !!shortcut.shiftKey === !!modifiers.shift &&
            !!shortcut.altKey === !!modifiers.alt
        );
      },
      [shortcuts]
    )
  };
};
