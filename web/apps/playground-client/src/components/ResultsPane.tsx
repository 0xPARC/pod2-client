import React, { useState } from "react";
import { useAppStore } from "../lib/store";
import MainPodCard from "./MainPodCard";
import type { MainPod } from "@pod2/pod2js";
import AddToSpaceDialog from "./AddToSpaceDialog";
import { Button } from "./ui/button";
import { PlusCircle } from "lucide-react";
import { MermaidDiagram } from "@lightenna/react-mermaid-diagram";
import { validateMainPod } from "@pod2/pod2js";

function isMainPod(obj: any): obj is MainPod {
  return validateMainPod(obj).success;
}

const ResultsPane: React.FC = () => {
  const isLoadingExecution = useAppStore((state) => state.isLoadingExecution);
  const executionResult = useAppStore((state) => state.executionResult);
  const executionError = useAppStore((state) => state.executionError);
  const activeSpaceId = useAppStore((state) => state.activeSpaceId); // Get activeSpaceId

  // State for the dialog
  const [isAddToSpaceDialogOpen, setIsAddToSpaceDialogOpen] = useState(false);
  const [podForModal, setPodForModal] = useState<MainPod | null>(null);

  const handleOpenAddToSpaceDialog = (pod: MainPod) => {
    setPodForModal(pod);
    setIsAddToSpaceDialogOpen(true);
  };

  let content;

  if (isLoadingExecution) {
    content = (
      <p className="p-4 text-gray-500 dark:text-gray-400">Executing...</p>
    );
  } else if (executionError) {
    content = (
      <pre className="p-4 text-sm text-red-600 dark:text-red-400 whitespace-pre-wrap break-all">
        Error: {executionError}
      </pre>
    );
  } else if (executionResult) {
    try {
      const parsedResult = JSON.parse(executionResult);
      if (isMainPod(parsedResult.main_pod)) {
        // If it's a MainPod, display MainPodCard and an "Add to Space" button
        content = (
          <div className="p-2 space-y-2">
            <MainPodCard mainPod={parsedResult.main_pod} />
            <Button
              onClick={() => handleOpenAddToSpaceDialog(parsedResult.main_pod)}
              variant="outline"
              size="sm"
              className="mt-2 w-full flex items-center justify-center"
            >
              <PlusCircle className="mr-2 h-4 w-4" />
              Add to Space
            </Button>
            <div className="bg-gray-300 dark:bg-gray-400 p-1">
              <h3 className="text-sm font-bold">Proof</h3>
              <MermaidDiagram>{parsedResult.diagram}</MermaidDiagram>
            </div>
          </div>
        );
      } else {
        // If it's not a MainPod or something else, display as string
        content = (
          <pre className="p-4 text-sm whitespace-pre-wrap break-all">
            {executionResult}
          </pre>
        );
      }
    } catch (e) {
      // If parsing fails, it's likely not JSON or not the JSON we expect for a POD
      console.warn(
        "Failed to parse execution result as JSON for PodCard, displaying as raw string:",
        e
      );
      content = (
        <pre className="p-4 text-sm whitespace-pre-wrap break-all">
          {executionResult}
        </pre>
      );
    }
  } else {
    content = (
      <p className="p-4 text-gray-500 dark:text-gray-400 text-sm">
        Execution results will appear here.
      </p>
    );
  }

  return (
    <div className="w-full h-full bg-gray-200 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded overflow-auto p-1">
      {content}
      {podForModal && (
        <AddToSpaceDialog
          isOpen={isAddToSpaceDialogOpen}
          onOpenChange={setIsAddToSpaceDialogOpen}
          mainPodToSave={podForModal}
          defaultSpaceId={activeSpaceId}
        />
      )}
    </div>
  );
};

export default ResultsPane;
