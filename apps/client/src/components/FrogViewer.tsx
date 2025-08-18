import { useEffect, useState } from "react";
import { useFrogCrypto } from "../lib/store";
import { Card, CardContent } from "./ui/card";
import { ScrollArea } from "./ui/scroll-area";
import { Button } from "./ui/button";
import {
  requestFrog,
  requestScore,
  listFrogs,
  Frog,
  FrogDerived
} from "@/lib/rpc";
import { toast } from "sonner";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@radix-ui/react-collapsible";
import { Switch } from "./ui/switch";
import { listen } from "@tauri-apps/api/event";
import { RARITY_SHADOW_COLORS } from "./FrogCrypto";

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

export function FrogViewer() {
  const { frogTimeout, setFrogTimeout, setScore, mining, setMining } = useFrogCrypto();

  const [time, setTime] = useState(new Date().getTime());
  const [frogs, setFrogs] = useState<FrogPod[]>([]);
  const [hashesChecked, setHashesChecked] = useState("");

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
          <Switch id="mining" checked={mining} onCheckedChange={setMining} />
          {hashesChecked}
        </div>
      </div>
      <div className="flex-1 min-h-0 overflow-hidden">
        <ScrollArea className="h-full">
          <div>
            {frogs.map((pod) => (
              <FrogCard pod={pod} key={pod.id} />
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
  pod: Frog;
}

function FrogCard({ pod }: FrogCardProps) {
  const [expanded, setExpanded] = useState(false);

  const haveDesc = pod.derived != null;
  const { levelUpId, setLevelUpId } = useFrogCrypto();
  const [levelProgress, setLevelProgress] = useState("");

  useEffect(() => {
      const unlisten = listen("level-up-status", (event) => {
        setLevelProgress(event.payload);
      });

      return () => {
        unlisten.then((f) => f());
      };
    }, []);


  return (
    <Card className="py-0 cursor-pointer transition-colors hover:bg-accent/50 max-w-sm">
      <CardContent className="p-3 flex flex-col text-center justify-center items-center">
        <div className="space-y-2">
          {haveDesc && (
            <img
              src={(pod.derived as FrogDerived).image_url}
              className={`mx-auto max-w-xs ${RARITY_SHADOW_COLORS[(pod.derived as FrogData).rarity]}`}
            ></img>
          )}
          <h2>{(pod.derived?.name ?? "???").toUpperCase()}</h2>
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
                <td>{pod.derived?.jump ?? "???"}</td>
                <td>{vibeEntry(pod.derived?.temperament)}</td>
                <td>{pod.derived?.speed ?? "???"}</td>
                <td>{pod.derived?.intelligence ?? "???"}</td>
                <td>{pod.derived?.beauty ?? "???"}</td>
              </tr>
            </tbody>
          </table>
        </div>
        {pod.offer_level_up &&
          <div>
          <Button
            disabled={levelUpId !== null}
            onClick={() => setLevelUpId(pod.id)}>
            Level up
          </Button>
          {levelUpId == pod.id &&
            <Button
              onClick={() => setLevelUpId(null)}>
              Cancel
            </Button>
          }
          <span>
            {levelProgress}
          </span>
          </div>
        }
        {haveDesc && (
          <Collapsible open={expanded} onOpenChange={setExpanded}>
            <CollapsibleTrigger asChild>
              {expanded ? <span>Collapse</span> : <span>See more</span>}
            </CollapsibleTrigger>
            <CollapsibleContent>
              {(pod.derived as FrogDerived).description}
            </CollapsibleContent>
          </Collapsible>
        )}
      </CardContent>
    </Card>
  );
}
