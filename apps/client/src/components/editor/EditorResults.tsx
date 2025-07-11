import {
  ChevronDown,
  ChevronUp,
  CheckCircle,
  AlertTriangle,
  Copy
} from "lucide-react";

import { Button } from "../ui/button";
import { ScrollArea } from "../ui/scroll-area";
import { formatExecutionResult } from "../../lib/features/authoring/editor";
import { useAppStore } from "../../lib/store";

interface EditorResultsProps {
  isOpen: boolean;
  onToggle: () => void;
  className?: string;
}

export function EditorResults({
  isOpen,
  onToggle,
  className
}: EditorResultsProps) {
  // Results state from store
  const executionResult = useAppStore((state) => state.executionResult);
  const executionError = useAppStore((state) => state.executionError);
  const isExecuting = useAppStore((state) => state.isExecuting);

  // Copy result to clipboard
  const handleCopyResult = async () => {
    if (!executionResult) return;

    try {
      const formattedResult = formatExecutionResult(executionResult);
      await navigator.clipboard.writeText(formattedResult);
      // Could add a toast notification here
    } catch (error) {
      console.error("Failed to copy result:", error);
    }
  };

  // Determine what to display
  let content;
  let statusIcon;
  let statusText;

  if (isExecuting) {
    statusIcon = (
      <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-blue-500" />
    );
    statusText = "Executing...";
    content = (
      <div className="flex items-center justify-center p-8 text-gray-500">
        <div className="text-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500 mx-auto mb-2" />
          <p>Executing Podlang code...</p>
        </div>
      </div>
    );
  } else if (executionError) {
    statusIcon = <AlertTriangle className="h-4 w-4 text-red-500" />;
    statusText = "Execution failed";
    content = (
      <div className="p-4">
        <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4">
          <div className="flex items-start gap-3">
            <AlertTriangle className="h-5 w-5 text-red-500 flex-shrink-0 mt-0.5" />
            <div className="flex-1">
              <h4 className="text-sm font-medium text-red-800 dark:text-red-200 mb-1">
                Execution Error
              </h4>
              <p className="text-sm text-red-700 dark:text-red-300 whitespace-pre-wrap">
                {executionError}
              </p>
            </div>
          </div>
        </div>
      </div>
    );
  } else if (executionResult) {
    statusIcon = <CheckCircle className="h-4 w-4 text-green-500" />;
    statusText = "Execution successful";
    content = (
      <div className="p-4">
        <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
          <div className="flex items-center justify-between p-4 border-b border-green-200 dark:border-green-800">
            <div className="flex items-center gap-2">
              <CheckCircle className="h-5 w-5 text-green-500" />
              <h4 className="text-sm font-medium text-green-800 dark:text-green-200">
                Execution Result
              </h4>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={handleCopyResult}
              className="text-green-700 dark:text-green-300 border-green-300 dark:border-green-700"
            >
              <Copy className="h-4 w-4 mr-1" />
              Copy
            </Button>
          </div>
          <div className="p-4">
            <ScrollArea className="h-80 w-full">
              <pre className="text-sm text-green-800 dark:text-green-100 whitespace-pre-wrap">
                {formatExecutionResult(executionResult)}
              </pre>
            </ScrollArea>
          </div>
        </div>
      </div>
    );
  } else {
    statusIcon = null;
    statusText = "No results";
    content = (
      <div className="flex items-center justify-center p-8 text-gray-500">
        <p>Click Execute to run your Podlang code</p>
      </div>
    );
  }

  return (
    <div
      className={`bg-sidebar border-t border-sidebar-border ${className || ""}`}
    >
      {/* Results Header */}
      <div className="flex items-center justify-between p-3 bg-sidebar border-b border-sidebar-border">
        <div className="flex items-center gap-2">
          {statusIcon}
          <span className="text-sm font-medium text-sidebar-foreground">
            Results
          </span>
          {statusText && (
            <span className="text-xs text-sidebar-foreground/70">
              • {statusText}
            </span>
          )}
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={onToggle}
          className="p-1 h-8 w-8"
        >
          {isOpen ? (
            <ChevronDown className="h-4 w-4" />
          ) : (
            <ChevronUp className="h-4 w-4" />
          )}
        </Button>
      </div>

      {/* Results Content */}
      {isOpen && <div className="max-h-96 overflow-hidden">{content}</div>}
    </div>
  );
}
