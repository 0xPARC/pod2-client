import { useState, useEffect } from "react";
import { Play, CheckCircle, XCircle, AlertCircle } from "lucide-react";

import { Button } from "../ui/button";
import { Label } from "../ui/label";
import { Switch } from "../ui/switch";
import { 
  hasValidationErrors, 
  getFirstErrorMessage,
  saveLastMockSetting,
  loadLastMockSetting
} from "../../lib/features/authoring/editor";
import { useAppStore } from "../../lib/store";

interface EditorControlsProps {
  className?: string;
}

export function EditorControls({ className }: EditorControlsProps) {
  // Editor state from store
  const editorDiagnostics = useAppStore((state) => state.editorDiagnostics);
  const isExecuting = useAppStore((state) => state.isExecuting);
  const isValidating = useAppStore((state) => state.isValidating);
  const executeEditorCode = useAppStore((state) => state.executeEditorCode);
  
  // Local mock setting state
  const [mock, setMock] = useState(() => loadLastMockSetting());
  
  // Validation state
  const hasErrors = hasValidationErrors(editorDiagnostics);
  const firstErrorMessage = getFirstErrorMessage(editorDiagnostics);
  
  // Save mock setting when it changes
  useEffect(() => {
    saveLastMockSetting(mock);
  }, [mock]);

  // Handle execute button click
  const handleExecute = () => {
    if (hasErrors || isExecuting) return;
    executeEditorCode(mock);
  };

  // Determine execute button state
  const canExecute = !hasErrors && !isExecuting && !isValidating;
  const executeButtonText = isExecuting ? "Executing..." : "Execute";

  // Validation status display
  let statusIcon;
  let statusText;
  let statusColor;

  if (isValidating) {
    statusIcon = <AlertCircle className="h-4 w-4 text-yellow-500" />;
    statusText = "Validating...";
    statusColor = "text-yellow-600 dark:text-yellow-400";
  } else if (hasErrors) {
    statusIcon = <XCircle className="h-4 w-4 text-red-500" />;
    statusText = firstErrorMessage || "Code has errors";
    statusColor = "text-red-600 dark:text-red-400";
  } else {
    statusIcon = <CheckCircle className="h-4 w-4 text-green-500" />;
    statusText = "Code is valid";
    statusColor = "text-green-600 dark:text-green-400";
  }

  return (
    <div className={`flex items-center justify-between bg-gray-200 dark:bg-gray-800 ${className || ""}`}>
      <div className="p-2 bg-gray-200 dark:bg-gray-800 flex items-center space-x-4">
        {/* Execute Button */}
        <Button
          onClick={handleExecute}
          disabled={!canExecute}
          size="sm"
          className="flex items-center gap-2"
        >
          <Play className="h-4 w-4" />
          {executeButtonText}
        </Button>

        {/* Mock Mode Switch */}
        <div className="flex items-center space-x-2">
          <Switch
            id="mock-mode"
            checked={mock}
            onCheckedChange={setMock}
            disabled={isExecuting}
          />
          <Label 
            htmlFor="mock-mode" 
            className="text-sm font-medium cursor-pointer select-none"
          >
            Mock Mode
          </Label>
        </div>
      </div>

      {/* Validation Status */}
      <div className="flex items-center gap-2">
        {statusIcon}
        <span className={`text-sm ${statusColor} max-w-md truncate`} title={statusText}>
          {statusText}
        </span>
      </div>
    </div>
  );
}