import { useState } from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup
} from "../ui/resizable";

import { EditorPane } from "./EditorPane";
import { EditorControls } from "./EditorControls";
import { EditorResults } from "./EditorResults";

const COLLAPSED_RESULTS_SIZE = 4; // Percentage when collapsed
const DEFAULT_RESULTS_SIZE = 30; // Percentage when expanded
const MIN_EDITOR_SIZE = 30; // Minimum percentage for editor panel
const MAX_RESULTS_SIZE = 70; // Maximum percentage for results panel

export function EditorView() {
  // Results panel state
  const [isResultsOpen, setIsResultsOpen] = useState(false);
  const [resultsPanelSize, setResultsPanelSize] = useState(DEFAULT_RESULTS_SIZE);

  // Handle results toggle
  const handleResultsToggle = () => {
    setIsResultsOpen(!isResultsOpen);
  };

  // Calculate panel sizes
  const editorSize = isResultsOpen 
    ? 100 - resultsPanelSize 
    : 100 - COLLAPSED_RESULTS_SIZE;
  
  const resultsSize = isResultsOpen 
    ? resultsPanelSize 
    : COLLAPSED_RESULTS_SIZE;

  return (
    <div className="h-full w-full flex flex-col">
      {/* Editor Controls */}
      <EditorControls />
      
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