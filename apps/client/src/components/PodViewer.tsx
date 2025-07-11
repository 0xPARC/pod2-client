import { useState, useEffect } from "react";
import { useAppStore } from "../lib/store";
import MainPodCard from "./MainPodCard";
import SignedPodCard from "./SignedPodCard";
import { DeletePodDialog } from "./DeletePodDialog";
import { Badge } from "./ui/badge";
import { Card, CardContent } from "./ui/card";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup
} from "./ui/resizable";
import { ScrollArea } from "./ui/scroll-area";
import { Separator } from "./ui/separator";
import { StarIcon, FolderIcon, Trash2, MoreHorizontal } from "lucide-react";
import { Button } from "./ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger
} from "./ui/dropdown-menu";

export function PodViewer() {
  const {
    getFilteredPods,
    getSelectedPod,
    setSelectedPodId,
    selectedPodId,
    togglePodPinned
  } = useAppStore();

  const filteredPods = getFilteredPods();
  const selectedPod = getSelectedPod();

  // Delete dialog state
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const [podToDelete, setPodToDelete] = useState<any>(null);

  const formatLabel = (pod: any) => {
    return pod.label || `${pod.pod_type} POD`;
  };

  const formatId = (id: string) => {
    return `${id.slice(0, 8)}...${id.slice(-4)}`;
  };

  const handleStarClick = (e: React.MouseEvent, pod: any) => {
    e.stopPropagation(); // Prevent card selection
    togglePodPinned(pod.id, pod.space);
  };

  const handleDeleteClick = (e: React.MouseEvent, pod: any) => {
    e.stopPropagation(); // Prevent card selection
    setPodToDelete(pod);
    setIsDeleteDialogOpen(true);
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
            activeElement.tagName === "TEXTAREA" ||
            activeElement.isContentEditable)
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
          <div className="p-4 border-b border-border">
            <h3 className="font-semibold text-lg">
              PODs ({filteredPods.length})
            </h3>
          </div>
          <ScrollArea className="flex-1 min-h-0">
            <div className="p-2 space-y-2">
              {filteredPods.length === 0 ? (
                <div className="text-center text-muted-foreground py-8">
                  No PODs found
                </div>
              ) : (
                filteredPods.map((pod) => (
                  <Card
                    key={pod.id}
                    className={`py-0 cursor-pointer transition-colors hover:bg-accent/50 ${
                      selectedPodId === pod.id
                        ? "bg-accent border-accent-foreground/20"
                        : ""
                    } ${pod.pinned ? "ring-1 ring-amber-200 bg-amber-50/30" : ""}`}
                    onClick={() => setSelectedPodId(pod.id)}
                  >
                    <CardContent className="p-3">
                      <div className="space-y-2">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2 min-w-0">
                            <Button
                              variant="ghost"
                              size="sm"
                              className={`p-0 h-4 w-4 hover:bg-transparent ${
                                pod.pinned
                                  ? "text-amber-500 hover:text-amber-600"
                                  : "text-muted-foreground hover:text-amber-500"
                              }`}
                              onClick={(e) => handleStarClick(e, pod)}
                            >
                              <StarIcon
                                className={`h-3 w-3 ${pod.pinned ? "fill-current" : ""}`}
                              />
                            </Button>
                            <span className="font-medium text-sm truncate">
                              {formatLabel(pod)}
                            </span>
                          </div>
                          <div className="flex items-center gap-1">
                            <Badge
                              variant="secondary"
                              className="text-xs shrink-0"
                            >
                              {pod.pod_type}
                            </Badge>
                            <DropdownMenu>
                              <DropdownMenuTrigger asChild>
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  className="p-0 h-4 w-4 hover:bg-transparent text-muted-foreground hover:text-foreground"
                                  onClick={(e) => e.stopPropagation()}
                                >
                                  <MoreHorizontal className="h-3 w-3" />
                                </Button>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent align="end">
                                <DropdownMenuItem
                                  onClick={(e) => handleDeleteClick(e, pod)}
                                  className="text-red-600 focus:text-red-600"
                                >
                                  <Trash2 className="h-3 w-3 mr-2" />
                                  Delete POD
                                </DropdownMenuItem>
                              </DropdownMenuContent>
                            </DropdownMenu>
                          </div>
                        </div>
                        <div className="flex items-center justify-between">
                          <div className="text-xs text-muted-foreground font-mono">
                            {formatId(pod.id)}
                          </div>
                          <div className="flex items-center gap-1 text-xs text-muted-foreground">
                            <FolderIcon className="h-3 w-3" />
                            <span
                              className="truncate max-w-[60px]"
                              title={pod.space}
                            >
                              {pod.space}
                            </span>
                          </div>
                        </div>
                      </div>
                    </CardContent>
                  </Card>
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
                <h3 className="font-semibold text-lg">
                  {formatLabel(selectedPod)}
                </h3>
                <div className="flex items-center gap-2">
                  <Badge variant="outline">{selectedPod.pod_type}</Badge>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleDeleteFromHeader}
                    className="text-red-600 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-900/20"
                  >
                    <Trash2 className="h-4 w-4 mr-1" />
                    Delete
                  </Button>
                </div>
              </div>
            </div>
            <ScrollArea className="flex-1 min-h-0">
              <div className="p-4 space-y-6">
                {/* Basic Info */}
                <div>
                  <h4 className="font-medium mb-3">Basic Information</h4>
                  <div className="space-y-2 text-sm">
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">ID:</span>
                      <span className="font-mono text-xs">
                        {selectedPod.id}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Type:</span>
                      <span>{selectedPod.pod_type}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Label:</span>
                      <span>{selectedPod.label || "None"}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Created:</span>
                      <span>
                        {new Date(selectedPod.created_at).toLocaleString()}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Space:</span>
                      <span>{selectedPod.space}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Pinned:</span>
                      <span className="flex items-center gap-1">
                        <StarIcon
                          className={`h-3 w-3 ${selectedPod.pinned ? "fill-current text-amber-500" : "text-muted-foreground"}`}
                        />
                        {selectedPod.pinned ? "Yes" : "No"}
                      </span>
                    </div>
                  </div>
                </div>

                <Separator />

                {/* POD Data */}
                <div>
                  <h4 className="font-medium mb-3">POD Data</h4>
                  {selectedPod.pod_type}

                  {selectedPod.data.pod_data_variant === "Main" && (
                    <MainPodCard mainPod={selectedPod.data.pod_data_payload} />
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
