import { useEffect, useRef } from "react";
import { useConsoleStore } from "../../lib/console/store";
import { ConsoleOutput } from "./ConsoleOutput";
import { ConsoleInput } from "./ConsoleInput";

export function ConsoleView() {
  const { initialize, isLoading, state, setInputValue, inputValue } =
    useConsoleStore();
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    initialize();

    // No cleanup on unmount - using singleton listeners
  }, []); // Empty dependency array to run only once

  // Handle keydown events to redirect focus to input
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't interfere if an input, textarea, or contenteditable is focused
      const activeElement = document.activeElement;
      if (
        activeElement &&
        (activeElement.tagName === "INPUT" ||
          activeElement.tagName === "TEXTAREA" ||
          activeElement.hasAttribute("contenteditable"))
      ) {
        return;
      }

      // Don't redirect for modifier key shortcuts (but allow shift for printable characters)
      if (e.ctrlKey || e.metaKey || e.altKey) {
        return;
      }

      // Don't redirect for special keys (except Enter, Space, Backspace, Delete)
      if (
        e.key.length > 1 &&
        !["Enter", "Space", "Backspace", "Delete"].includes(e.key)
      ) {
        return;
      }

      // Focus the input and add the typed character
      if (inputRef.current) {
        inputRef.current.focus();

        // For printable characters, add them to the input
        if (e.key.length === 1) {
          e.preventDefault();
          setInputValue(inputValue + e.key);
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [setInputValue, inputValue]);

  return (
    <div className="flex flex-col h-full w-full bg-white dark:bg-gray-900">
      {/* Header */}
      <div className="border-b border-gray-200 dark:border-gray-700 p-3">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Console
          </h1>
          <div className="flex items-center gap-4 text-sm text-gray-600 dark:text-gray-400">
            {state && (
              <>
                <span>
                  Folder:{" "}
                  <code className="font-mono bg-gray-100 dark:bg-gray-800 px-1 rounded">
                    /{state.current_folder}
                  </code>
                </span>
                <span>Messages: {state.total_message_count}</span>
              </>
            )}
            {isLoading && (
              <span className="text-blue-600 dark:text-blue-400">
                Loading...
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Console output area */}
      <ConsoleOutput />

      {/* Console input */}
      <ConsoleInput ref={inputRef} />
    </div>
  );
}
