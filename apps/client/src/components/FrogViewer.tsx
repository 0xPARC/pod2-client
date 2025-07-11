import { useAppStore } from "../lib/store";
import { Badge } from "./ui/badge";
import { Card, CardContent } from "./ui/card";
import { ScrollArea } from "./ui/scroll-area";
import { StarIcon, FolderIcon } from "lucide-react";
import { Button } from "./ui/button";
import { requestFrog } from "@/lib/rpc";
import { useEffect, useState } from "react";

export function FrogViewer() {
  const {
    getFilteredPodsBy,
    setSelectedPodId,
    selectedPodId,
    togglePodPinned,
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

  const filteredPods = getFilteredPodsBy("signed", "frogs");

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
    <div className="h-full flex flex-col">
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
        <h3 className="font-semibold text-lg">Frogs ({filteredPods.length})</h3>
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
                    </div>
                    <img src={(pod.data.pod_data_payload as SignedPod).entries.image_url as string}></img>
                    <div className="flex items-center justify-between">
                      <div className="text-xs text-muted-foreground font-mono">
                        {formatId(pod.id)}
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
  );
}
