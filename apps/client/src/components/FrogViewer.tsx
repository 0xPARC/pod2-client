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
import type { SignedPod, Value } from "@pod2/pod2js";
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
    <div className="h-full flex flex-col overflow-hidden">
      <div className="p-4 border-b border-border">
        <Button
          variant="outline"
          onClick={() => requestFrogAndUpdateTimeout()}
          disabled={searchDisabled}
        >
          Search SWAMP {searchButtonWaitText}
        </Button>
      </div>
      <div className="flex-1 min-h-0 overflow-hidden">
        <ScrollArea className="h-full">
          <div>
            {filteredPods.map((pod) => (
              <FrogCard pod={pod} key={pod.id} />
            ))}
          </div>
        </ScrollArea>
      </div>
    </div>
  );
}

function intEntry(value: Value): string {
  const entry = (value as { Int: string })?.Int;
  if (entry === undefined) {
    return "";
  } else {
    return entry;
  }
}

interface FrogCardProps {
  pod: PodInfo;
}

function FrogCard({ pod }: FrogCardProps) {
  const [expanded, setExpanded] = useState(false);

  const entries = (pod.data.pod_data_payload as SignedPod).entries;

  return (
    <Card
      key={pod.id}
      className="py-0 cursor-pointer transition-colors hover:bg-accent/50 max-w-sm"
    >
      <CardContent className="p-3 flex flex-col text-center justify-center items-center">
        <div className="space-y-2">
          <img
            src={
              (pod.data.pod_data_payload as SignedPod).entries
                .image_url as string
            }
            className="max-w-xs"
          ></img>
          <h2>{(entries.name as string).toUpperCase()}</h2>
        </div>
        <div>
          <table className="[&_th]:px-4 text-center">
            <thead>
              <tr>
                <th>JMP</th>
                <th>SPD</th>
                <th>INT</th>
                <th>BTY</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>{intEntry(entries.jump)}</td>
                <td>{intEntry(entries.speed)}</td>
                <td>{intEntry(entries.intelligence)}</td>
                <td>{intEntry(entries.beauty)}</td>
              </tr>
            </tbody>
          </table>
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
