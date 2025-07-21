import { useRef, useEffect, forwardRef } from "react";
import { useConsoleStore } from "../../lib/console/store";

export const ConsoleInput = forwardRef<HTMLInputElement>((props, ref) => {
  const {
    inputValue,
    state,
    executeCommand,
    setInputValue,
    navigateHistory,
    error,
    clearError
  } = useConsoleStore();

  const internalRef = useRef<HTMLInputElement>(null);
  const inputRef = ref || internalRef;

  // Focus input on mount and when needed
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (inputValue.trim()) {
      await executeCommand(inputValue.trim());
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      navigateHistory("up");
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      navigateHistory("down");
    } else if (error && e.key !== "ArrowUp" && e.key !== "ArrowDown") {
      // Clear error when user starts typing
      clearError();
    }
  };

  const currentFolder = state?.current_folder || "default";

  return (
    <div className="border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Error display */}
      {error && (
        <div className="px-3 py-2 bg-red-50 dark:bg-red-900/20 border-b border-red-200 dark:border-red-800">
          <div className="text-red-700 dark:text-red-300 text-sm font-mono">
            ✗ {error}
          </div>
        </div>
      )}

      {/* Input form */}
      <form onSubmit={handleSubmit} className="flex items-center p-3">
        {/* Folder prompt */}
        <div className="flex items-center gap-1 text-sm font-mono">
          <span className="text-blue-600 dark:text-blue-400">
            {currentFolder !== "default" ? `${currentFolder}/` : ""}
          </span>
          <span className="text-gray-600 dark:text-gray-300">{">"}</span>
        </div>

        {/* Input field */}
        <input
          ref={inputRef}
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleKeyDown}
          className="flex-1 ml-2 bg-transparent outline-none font-mono text-sm text-gray-900 dark:text-gray-100 placeholder-gray-500"
          placeholder="Enter command..."
          spellCheck={false}
          autoComplete="off"
        />
      </form>
    </div>
  );
});

ConsoleInput.displayName = "ConsoleInput";
