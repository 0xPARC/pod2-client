import {
  FileCheck,
  FilePen,
  FolderIcon,
  FoldersIcon,
  MoreHorizontal,
  Trash2
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { createShortcut } from "../../lib/keyboard/types";
import { useKeyboardShortcuts } from "../../lib/keyboard/useKeyboardShortcuts";
import { usePodCollection } from "../../lib/store";
import { DeletePodDialog } from "../DeletePodDialog";
import MainPodCard from "../core/MainPodCard";
import SignedPodCard from "../core/SignedPodCard";
import { TopBarSlot } from "../core/TopBarContext";
import { Button } from "../ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger
} from "../ui/dropdown-menu";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup
} from "../ui/resizable";
import { ScrollArea } from "../ui/scroll-area";

export function PodViewer() {
  const {
    filteredPods,
    selectedPod,
    selectedPodId,
    selectPod,
    selectedFolder
  } = usePodCollection();

  // Delete dialog state
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const [podToDelete, setPodToDelete] = useState<any>(null);

  // Navigate through POD list with arrow keys
  const navigatePods = useCallback(
    (direction: "up" | "down") => {
      if (filteredPods.length === 0) return;

      const currentIndex = selectedPodId
        ? filteredPods.findIndex((pod) => pod.id === selectedPodId)
        : -1;

      let nextIndex;
      if (direction === "up") {
        nextIndex =
          currentIndex <= 0 ? filteredPods.length - 1 : currentIndex - 1;
      } else {
        nextIndex =
          currentIndex >= filteredPods.length - 1 ? 0 : currentIndex + 1;
      }

      selectPod(filteredPods[nextIndex].id);
    },
    [filteredPods, selectedPodId, selectPod]
  );

  // POD Collection keyboard shortcuts
  const shortcuts = [
    // Delete selected POD
    createShortcut(
      "d",
      () => {
        if (selectedPod) {
          setPodToDelete(selectedPod);
          setIsDeleteDialogOpen(true);
        }
      },
      "Delete Selected POD",
      {
        cmd: true
      }
    ),
    // Navigate up
    createShortcut("ArrowUp", () => navigatePods("up"), "Navigate Up", {
      preventDefault: true
    }),
    // Navigate down
    createShortcut("ArrowDown", () => navigatePods("down"), "Navigate Down", {
      preventDefault: true
    }),
    // Select POD (if none selected, select first)
    createShortcut(
      "Enter",
      () => {
        if (!selectedPodId && filteredPods.length > 0) {
          selectPod(filteredPods[0].id);
        }
      },
      "Select First POD",
      {
        preventDefault: true
      }
    )
  ];

  useKeyboardShortcuts(shortcuts, {
    enabled: true,
    context: "pod-collection"
  });

  const formatLabel = (pod: any) => {
    return pod.label || `${pod.pod_type} POD`;
  };

  const handleDeleteFromHeader = () => {
    if (selectedPod) {
      setPodToDelete(selectedPod);
      setIsDeleteDialogOpen(true);
    }
  };

  // Keyboard shortcut support
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Only handle Delete key when a POD is selected and no dialogs are open
      if (
        e.key === "Delete" &&
        selectedPod &&
        !isDeleteDialogOpen &&
        !e.ctrlKey &&
        !e.metaKey &&
        !e.altKey &&
        !e.shiftKey
      ) {
        // Make sure we're not in an input field or text area
        const activeElement = document.activeElement;
        if (
          activeElement &&
          (activeElement.tagName === "INPUT" ||
            activeElement.tagName === "TEXTAREA")
        ) {
          return;
        }

        e.preventDefault();
        setPodToDelete(selectedPod);
        setIsDeleteDialogOpen(true);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [selectedPod, isDeleteDialogOpen]);

  return (
    <>
      <ResizablePanelGroup direction="horizontal" className="h-full">
        {/* Left panel - POD list */}
        <ResizablePanel defaultSize={35} minSize={25} maxSize={60}>
          <div className="h-full flex flex-col">
            <TopBarSlot position="left">
              {" "}
              <h3 className="font-normal text-base flex items-center gap-2">
                {selectedFolder === "all" ? (
                  <div className="flex items-center gap-2 font-semibold">
                    <FoldersIcon className="h-4 w-4" /> All PODs
                  </div>
                ) : selectedFolder ? (
                  <div className="flex items-center gap-2 font-semibold">
                    <FolderIcon className="h-4 w-4" /> {selectedFolder}
                  </div>
                ) : (
                  <div className="flex items-center gap-2 font-semibold">
                    <FolderIcon className="h-4 w-4" /> PODs
                  </div>
                )}{" "}
                ({filteredPods.length})
              </h3>
            </TopBarSlot>
            <ScrollArea className="flex-1 min-h-0">
              <div className="p-0">
                {filteredPods.length === 0 ? (
                  <div className="text-center text-muted-foreground py-8">
                    No PODs found
                  </div>
                ) : (
                  filteredPods.map((pod) => (
                    <div
                      key={pod.id}
                      className={`flex items-center gap-2 px-2 py-1.5 rounded cursor-pointer transition-colors hover:bg-accent/50 ${
                        selectedPodId === pod.id
                          ? "bg-accent text-accent-foreground"
                          : ""
                      }`}
                      onClick={() => selectPod(pod.id)}
                    >
                      {pod.pod_type === "signed" ? (
                        <FilePen className="h-4 w-4 shrink-0 text-blue-600 dark:text-blue-400" />
                      ) : (
                        <FileCheck className="h-4 w-4 shrink-0 text-fuchsia-600 dark:text-fuchsia-400" />
                      )}
                      <span className="font-base text-sm truncate">
                        {formatLabel(pod)}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </ScrollArea>
          </div>
        </ResizablePanel>

        <ResizableHandle />

        {/* Right panel - POD details */}
        <ResizablePanel defaultSize={65}>
          {selectedPod ? (
            <div className="h-full flex flex-col">
              <div className="p-4 border-b border-border flex-shrink-0">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <div>
                      {selectedPod.pod_type === "signed" ? (
                        <FilePen className="h-5 w-5 text-blue-600 dark:text-blue-400" />
                      ) : (
                        <FileCheck className="h-5 w-5 text-fuchsia-600 dark:text-fuchsia-400" />
                      )}
                    </div>
                    <h3 className="font-semibold text-lg">
                      {formatLabel(selectedPod)}
                    </h3>
                  </div>
                  <div className="flex items-center gap-2">
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button
                          variant="ghost"
                          size="default"
                          className="h-6  px-2"
                        >
                          <MoreHorizontal className="h-6 w-6" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem
                          onClick={handleDeleteFromHeader}
                          className="text-red-600 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-900/20"
                        >
                          <Trash2 className="h-4 w-4 mr-2" />
                          Delete
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </div>
              </div>
              <ScrollArea className="flex-1 min-h-0 [&>div>div]:!block">
                <div className="p-4 space-y-6">
                  <div>
                    {selectedPod.data.pod_data_variant === "Main" && (
                      <MainPodCard
                        mainPod={selectedPod.data.pod_data_payload}
                      />
                    )}
                    {selectedPod.data.pod_data_variant === "Signed" && (
                      <SignedPodCard
                        signedPod={selectedPod.data.pod_data_payload}
                      />
                    )}
                  </div>
                </div>
              </ScrollArea>
            </div>
          ) : (
            <div className="flex items-center justify-center h-full text-muted-foreground">
              <div className="text-center">
                <div className="text-lg mb-2">Select a POD</div>
                <div className="text-sm">
                  Choose a POD from the list to view its details
                </div>
              </div>
            </div>
          )}
        </ResizablePanel>
      </ResizablePanelGroup>

      {/* Delete confirmation dialog */}
      <DeletePodDialog
        pod={podToDelete}
        isOpen={isDeleteDialogOpen}
        onOpenChange={(open) => {
          setIsDeleteDialogOpen(open);
          if (!open) {
            setPodToDelete(null);
          }
        }}
      />
    </>
  );
}
