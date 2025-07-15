import { useAppStore } from "../lib/store";
import { Card, CardContent } from "./ui/card";
import { ScrollArea } from "./ui/scroll-area";
import { Button } from "./ui/button";
import { requestFrog } from "@/lib/rpc";
import { useEffect, useState } from "react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@radix-ui/react-collapsible";
import type { SignedPod } from "@pod2/pod2js";
import type { PodInfo } from "@/lib/rpc";

interface FrogViewerProps {
  setScore: (score: number) => void;
}

export function FrogViewer({ setScore }: FrogViewerProps) {
  const { getFilteredPodsBy, setFrogTimeout, frogTimeout } = useAppStore();

  const [time, setTime] = useState(new Date().getTime());

  useEffect(() => {
    const interval = setInterval(() => setTime(new Date().getTime()), 1000);
    return () => {
      clearInterval(interval);
    };
  }, []);

  const filteredPods = getFilteredPodsBy("signed", "frogs");

  const requestFrogAndUpdateTimeout = async () => {
    setScore(await requestFrog());
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
      <ScrollArea className="flex-1 min-h-0">
        <div className="p-2 space-y-2">{filteredPods.map(FrogCard)}</div>
      </ScrollArea>
    </div>
  );
}

function FrogCard(pod: PodInfo) {
  const [expanded, setExpanded] = useState(false);

  return (
    <Card
      key={pod.id}
      className="py-0 cursor-pointer transition-colors hover:bg-accent/50"
    >
      <CardContent className="p-3">
        <div className="space-y-2">
          <img
            src={
              (pod.data.pod_data_payload as SignedPod).entries
                .image_url as string
            }
            className="max-w-xs"
          ></img>
          <span className="font-medium text-sm truncate">
            {(
              (pod.data.pod_data_payload as SignedPod).entries.name as string
            ).toUpperCase()}
          </span>
        </div>
        <Collapsible open={expanded} onOpenChange={setExpanded}>
          <CollapsibleTrigger asChild>
            {expanded ? <span>Collapse</span> : <span>See more</span>}
          </CollapsibleTrigger>
          <CollapsibleContent>
            {
              (pod.data.pod_data_payload as SignedPod).entries
                .description as string
            }
          </CollapsibleContent>
        </Collapsible>
      </CardContent>
    </Card>
  );
}
