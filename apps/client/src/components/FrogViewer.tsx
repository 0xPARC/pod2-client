import { useEffect, useState } from "react";
import { useAppStore } from "../lib/store";
import { Card, CardContent } from "./ui/card";
import { ScrollArea } from "./ui/scroll-area";
import { Button } from "./ui/button";
import {
  requestFrog,
  requestScore,
  listFrogs,
  FrogPod,
  FrogData
} from "@/lib/rpc";
import { toast } from "sonner";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@radix-ui/react-collapsible";
import { Switch } from "./ui/switch";
import { listen, emit } from "@tauri-apps/api/event";
import { RARITY_SHADOW_COLORS } from "./FrogCrypto";

interface FrogViewerProps {
  setScore: (score: number) => void;
}

function waitText(timeRemaining: number) {
  const mins = Math.floor(timeRemaining / 60);
  const secs = timeRemaining % 60;
  const minsText = mins == 0 ? "" : ` ${mins}m`;
  const secsText = secs == 0 ? "" : ` ${secs}s`;
  return ` (wait ${minsText}${secsText})`;
}

const temperaments = new Map<number, string>([
  [1, "N/A"],
  [2, "ANGY"],
  [3, "BORD"],
  [4, "CALM"],
  [7, "DARK"],
  [10, "HNGY"],
  [16, "SADG"],
  [18, "SLPY"]
]);

export function FrogViewer({ setScore }: FrogViewerProps) {
  const { setFrogTimeout, frogTimeout } = useAppStore();

  const [time, setTime] = useState(new Date().getTime());
  const [frogs, setFrogs] = useState<FrogPod[]>([]);
  const [hashesChecked, setHashesChecked] = useState("");
  const [selection, setSelection] = useState("");

  async function updateFrogs() {
    try {
      const frogList = await listFrogs();
      setFrogs(frogList);
    } catch (e) {}
  }

  useEffect(() => {
    const interval = setInterval(() => setTime(new Date().getTime()), 1000);
    return () => {
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    updateFrogs();
  }, []);

  const requestFrogAndUpdateTimeout = async () => {
    try {
      setScore(await requestFrog());
      setFrogTimeout(new Date().getTime() + 900000);
    } catch (e) {
      if (e instanceof Error) {
        toast.error(e.toString());
      }
    }
  };

  useEffect(() => {
    async function updateTimeout() {
      try {
        const scoreResponse = await requestScore();
        if (scoreResponse.timeout > 0) {
          setFrogTimeout(new Date().getTime() + 1000 * scoreResponse.timeout);
        }
      } catch (e) {}
    }
    updateTimeout();
  }, []);

  useEffect(() => {
    const unlisten = listen("frog-alert", (event) => {
      toast(event.payload as string);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen("frog-background", (event) => {
      setHashesChecked(`${event.payload}K hashes checked`);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen("refresh-frogs", () => {
      updateFrogs();
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const toggleMining = async (b: boolean) => {
    await emit("toggle-mining", b);
  };

  const timeRemaining =
    frogTimeout === null || time >= frogTimeout
      ? 0
      : Math.ceil(0.001 * (frogTimeout - time));
  const searchDisabled = timeRemaining > 0;
  const searchButtonWaitText = searchDisabled ? waitText(timeRemaining) : "";

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <div className="p-4 border-b border-border flex flex-col">
        <Button
          variant="outline"
          className="w-fit"
          onClick={() => requestFrogAndUpdateTimeout()}
          disabled={searchDisabled}
        >
          Search SWAMP {searchButtonWaitText}
        </Button>
        <div className="py-4">
          <label htmlFor="mining">Mining enabled</label>
          <Switch id="mining" onCheckedChange={toggleMining} />
          {hashesChecked}
        </div>
      </div>
      <div className="flex-1 min-h-0 overflow-hidden">
        <ScrollArea className="h-full">
          <div>
            {frogs.map((pod) => (
              <FrogCard pod={pod} key={pod.pod_id} />
            ))}
          </div>
        </ScrollArea>
      </div>
    </div>
  );
}

function vibeEntry(index: number | undefined): string {
  if (index == null) {
    return "???";
  } else {
    return temperaments.get(index as number) ?? "???";
  }
}

interface FrogCardProps {
  pod: FrogPod;
}

function FrogCard({ pod }: FrogCardProps) {
  const [expanded, setExpanded] = useState(false);

  const haveDesc = pod.data != null;

  return (
    <Card className="py-0 cursor-pointer transition-colors hover:bg-accent/50 max-w-sm">
      <CardContent className="p-3 flex flex-col text-center justify-center items-center">
        <div className="space-y-2">
          {haveDesc && (
            <img
              src={(pod.data as FrogData).image_url}
              className={`max-w-xs drop-shadow-xl ${RARITY_SHADOW_COLORS[(pod.data as FrogData).rarity]}`}
            ></img>
          )}
          <h2>{(pod.data?.name ?? "???").toUpperCase()}</h2>
        </div>
        <div>
          <table className="[&_th]:px-4 text-center">
            <thead>
              <tr>
                <th>JMP</th>
                <th>VIB</th>
                <th>SPD</th>
                <th>INT</th>
                <th>BTY</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>{pod.data?.jump ?? "???"}</td>
                <td>{vibeEntry(pod.data?.temperament)}</td>
                <td>{pod.data?.speed ?? "???"}</td>
                <td>{pod.data?.intelligence ?? "???"}</td>
                <td>{pod.data?.beauty ?? "???"}</td>
              </tr>
            </tbody>
          </table>
        </div>
        {haveDesc && (
          <Collapsible open={expanded} onOpenChange={setExpanded}>
            <CollapsibleTrigger asChild>
              {expanded ? <span>Collapse</span> : <span>See more</span>}
            </CollapsibleTrigger>
            <CollapsibleContent>
              {(pod.data as FrogData).description}
            </CollapsibleContent>
          </Collapsible>
        )}
      </CardContent>
    </Card>
  );
}
