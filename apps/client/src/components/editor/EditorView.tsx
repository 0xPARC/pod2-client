import { AlertCircle, CheckCircle, Play, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import {
  getFirstErrorMessage,
  hasValidationErrors,
  loadLastMockSetting,
  saveLastMockSetting
} from "../../lib/features/authoring/editor";
import { createShortcut } from "../../lib/keyboard/types";
import { useKeyboardShortcuts } from "../../lib/keyboard/useKeyboardShortcuts";
import { usePodEditor } from "../../lib/store";
import { TopBarSlot } from "../core/TopBarContext";
import { Button } from "../ui/button";
import { Label } from "../ui/label";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup
} from "../ui/resizable";
import { Switch } from "../ui/switch";

import { EditorPane } from "./EditorPane";
import { EditorResults } from "./EditorResults";

const COLLAPSED_RESULTS_SIZE = 4; // Percentage when collapsed
const DEFAULT_RESULTS_SIZE = 30; // Percentage when expanded
const MIN_EDITOR_SIZE = 30; // Minimum percentage for editor panel
const MAX_RESULTS_SIZE = 70; // Maximum percentage for results panel

export function EditorView() {
  const { editorDiagnostics, isExecuting, isValidating, executeEditorCode } =
    usePodEditor();

  // Results panel state
  const [isResultsOpen, setIsResultsOpen] = useState(false);
  const [resultsPanelSize, setResultsPanelSize] =
    useState(DEFAULT_RESULTS_SIZE);

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

  // POD Editor keyboard shortcuts
  const shortcuts = [
    // Execute code
    createShortcut(
      "Enter",
      () => {
        executeEditorCode(false); // Execute with real proofs
      },
      "Execute Code",
      {
        cmd: true
      }
    ),
    // Execute with mock proofs
    createShortcut(
      "Enter",
      () => {
        executeEditorCode(true); // Execute with mock proofs
      },
      "Execute Code (Mock)",
      {
        cmd: true,
        shift: true
      }
    )
  ];

  useKeyboardShortcuts(shortcuts, {
    enabled: true,
    context: "pod-editor"
  });

  // Handle results toggle
  const handleResultsToggle = () => {
    setIsResultsOpen(!isResultsOpen);
  };

  // Calculate panel sizes
  const editorSize = isResultsOpen
    ? 100 - resultsPanelSize
    : 100 - COLLAPSED_RESULTS_SIZE;

  const resultsSize = isResultsOpen ? resultsPanelSize : COLLAPSED_RESULTS_SIZE;

  return (
    <div className="h-full w-full flex flex-col">
      {/* TopBar Content */}
      <TopBarSlot position="left">
        <div className="flex items-center space-x-4">
          {/* Execute Button */}
          <Button
            onClick={handleExecute}
            disabled={!canExecute}
            size="sm"
            className="flex items-center gap-2 h-7"
          >
            <Play className="h-3 w-3" />
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
      </TopBarSlot>

      <TopBarSlot position="right">
        <div className="flex items-center gap-2">
          {statusIcon}
          <span
            className={`text-sm ${statusColor} max-w-md truncate`}
            title={statusText}
          >
            {statusText}
          </span>
        </div>
      </TopBarSlot>

      {/* Main Editor Area with Resizable Panels */}
      <div className="flex-1 overflow-hidden">
        <ResizablePanelGroup
          direction="vertical"
          className="h-full"
          onLayout={(sizes: number[]) => {
            // Update results panel size when user manually resizes
            if (
              isResultsOpen &&
              sizes.length > 1 &&
              sizes[1] > COLLAPSED_RESULTS_SIZE + 1
            ) {
              setResultsPanelSize(sizes[1]);
            }
          }}
        >
          {/* Editor Panel */}
          <ResizablePanel
            defaultSize={editorSize}
            minSize={MIN_EDITOR_SIZE}
            className="relative"
          >
            <EditorPane className="h-full" />
          </ResizablePanel>

          {/* Resizable Handle */}
          <ResizableHandle
            withHandle
            className="bg-gray-200 dark:bg-gray-700 hover:bg-blue-300 dark:hover:bg-blue-600 data-[resize-handle-state=drag]:bg-blue-500"
          />

          {/* Results Panel */}
          <ResizablePanel
            className="overflow-scroll"
            defaultSize={resultsSize}
            minSize={COLLAPSED_RESULTS_SIZE}
            maxSize={MAX_RESULTS_SIZE}
            collapsible={true}
            collapsedSize={COLLAPSED_RESULTS_SIZE}
            onCollapse={() => {
              if (isResultsOpen) {
                setIsResultsOpen(false);
              }
            }}
            onExpand={() => {
              if (!isResultsOpen) {
                setIsResultsOpen(true);
              }
            }}
          >
            <EditorResults
              isOpen={isResultsOpen}
              onToggle={handleResultsToggle}
              className="h-full"
            />
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>
    </div>
  );
}
