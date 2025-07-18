import { requestFrog } from "@/lib/rpc";
import { FolderIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { useAppStore } from "../lib/store";
import MainPodCard from "./MainPodCard";
import SignedPodCard from "./SignedPodCard";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent } from "./ui/card";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup
} from "./ui/resizable";
import { ScrollArea } from "./ui/scroll-area";
import { Separator } from "./ui/separator";

export function FrogViewer() {
  const {
    getPodsInFolder: getFilteredPodsBy,
    getSelectedPod,
    setSelectedPodId,
    selectedPodId,
    setFrogTimeout,
    frogTimeout
  } = useAppStore();

  const [time, setTime] = useState(new Date().getTime());

  useEffect(() => {
    const interval = setInterval(() => setTime(new Date().getTime()), 1000);
    return () => {
      clearInterval(interval);
    };
  }, []);

  const filteredPods = getFilteredPodsBy("frogs");
  const selectedPod = getSelectedPod();

  const formatLabel = (pod: any) => {
    return pod.label || `${pod.pod_type} POD`;
  };

  const formatId = (id: string) => {
    return `${id.slice(0, 8)}...${id.slice(-4)}`;
  };

  const requestFrogAndUpdateTimeout = async () => {
    await requestFrog();
    setFrogTimeout(new Date().getTime() + 900000);
  };

  const timeRemaining =
    frogTimeout === null || time >= frogTimeout
      ? 0
      : Math.ceil(0.001 * (frogTimeout - time));
  const searchDisabled = timeRemaining > 0;
  const searchButtonWaitText = searchDisabled
    ? ` (wait ${timeRemaining}s)`
    : "";

  return (
    <ResizablePanelGroup direction="horizontal" className="h-full">
      {/* Left panel - POD list */}
      <ResizablePanel defaultSize={35} minSize={25} maxSize={60}>
        <div className="h-full flex flex-col">
          <div className="p-4 border-b border-border">
            <h3 className="font-semibold text-lg">FROGCRYPTO</h3>
          </div>
          <div className="p-4 border-b border-border">
            <Button
              variant="outline"
              onClick={() => requestFrogAndUpdateTimeout()}
              disabled={searchDisabled}
            >
              Search SWAMP {searchButtonWaitText}
            </Button>
          </div>
          <div className="p-4 border-b border-border">
            <h3 className="font-semibold text-lg">
              Frogs ({filteredPods.length})
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
                    }`}
                    onClick={() => setSelectedPodId(pod.id)}
                  >
                    <CardContent className="p-3">
                      <div className="space-y-2">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2 min-w-0">
                            <span className="font-medium text-sm truncate">
                              {formatLabel(pod)}
                            </span>
                          </div>
                          <Badge
                            variant="secondary"
                            className="text-xs shrink-0"
                          >
                            {pod.pod_type}
                          </Badge>
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
                <Badge variant="outline">{selectedPod.pod_type}</Badge>
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
  );
}
