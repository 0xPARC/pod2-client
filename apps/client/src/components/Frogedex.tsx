import { ScrollArea } from "./ui/scroll-area";
import { useAppStore } from "../lib/store";
import { SignedPod } from "@pod2/pod2js";
import { ListIcon, Grid3x3Icon } from "lucide-react";
import { useState } from "react";
import { Button } from "./ui/button";

const FROG_LIST: [string, number][] = [
  ["1", 0],
  ["2", 0],
  ["3", 0],
  ["4", 0],
  ["5", 0],
  ["6", 0],
  ["7", 0],
  ["8", 0],
  ["9", 0],
  ["10", 0],
  ["11", 0],
  ["12", 0],
  ["13", 0],
  ["14", 0],
  ["15", 0],
  ["16", 0],
  ["17", 0],
  ["18", 0],
  ["19", 0],
  ["20", 0],
  ["21", 0],
  ["22", 0],
  ["23", 0],
  ["24", 0],
  ["25", 0],
  ["26", 0],
  ["27", 0],
  ["28", 0],
  ["29", 0],
  ["30", 0],
  ["31", 0],
  ["32", 0],
  ["33", 0],
  ["34", 0],
  ["35", 0],
  ["36", 0],
  ["37", 0],
  ["38", 0],
  ["39", 0],
  ["40", 0],
  ["41", 1],
  ["42", 1],
  ["43", 1],
  ["44", 1],
  ["45", 1],
  ["46", 1],
  ["47", 1],
  ["48", 1],
  ["49", 1],
  ["50", 1],
  ["51", 1],
  ["52", 1],
  ["53", 1],
  ["54", 1],
  ["55", 1],
  ["56", 2],
  ["57", 2],
  ["58", 2],
  ["59", 2],
  ["60", 2],
  ["61", 3],
  ["62", 3],
  ["63", 4],
  ["64", 3],
  ["65", 3],
  ["66", 3],
  ["67", 4],
  ["68", 3],
  ["69", 3],
  ["70", 3],
  ["71", 1],
  ["72", 1],
  ["73", 1],
  ["74", 1],
  ["75", 1],
  ["76", 1],
  ["77", 1],
  ["78", 1],
  ["79", 1],
  ["80", 1]
];

const RARITY_NAMES = ["NORM", "RARE", "EPIC", "LGND", "MYTH"];

function frogName(frogId: string, frogData: Map<string, SignedPod[]>): string {
  if (frogData.has(frogId)) {
    const pod = frogData.get(frogId)![0]!;
    return pod.entries.name as string;
  } else {
    return "???";
  }
}

function frogImage(frogId: string, frogData: Map<string, SignedPod[]>): string {
  if (frogData.has(frogId)) {
    const pod = frogData.get(frogId)![0]!;
    return pod.entries.image_url as string;
  } else {
    return "https://frogcrypto.vercel.app/images/pixel_frog.png";
  }
}

function frogCount(frogId: string, frogData: Map<string, SignedPod[]>): string {
  if (frogData.has(frogId)) {
    const count = frogData.get(frogId)!.length;
    return `x${count}`;
  } else {
    return "";
  }
}

export function Frogedex() {
  const { getFilteredPodsBy } = useAppStore();

  const frogPods = getFilteredPodsBy("signed", "frogs");
  let frogData = new Map<string, SignedPod[]>();
  for (const pod of frogPods) {
    const signedPod = pod.data.pod_data_payload as SignedPod;
    const frogId = (signedPod.entries.frogId as { Int: string }).Int;
    if (!frogData.has(frogId)) {
      frogData.set(frogId, []);
    }
    frogData.get(frogId)!.push(signedPod);
  }

  const [iconView, setIconView] = useState(false);

  return (
    <ScrollArea className="h-full flex-1 min-h-0">
      <div className="p-4">
        <Button
          className={`max-w-48 ${!iconView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => setIconView(false)}
        >
          <ListIcon />
        </Button>
        <Button
          className={`max-w-48 ${iconView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => setIconView(true)}
        >
          <Grid3x3Icon />
        </Button>
        {!iconView && (
          <table className="[&_td]:px-4 text-center">
            <tbody>
              {FROG_LIST.map((entry) => (
                <tr key={entry[0]}>
                  <td>{entry[0]}</td>
                  <td>{RARITY_NAMES[entry[1]]}</td>
                  <td>{frogName(entry[0], frogData)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {iconView && (
          <div className="grid grid-cols-3 text-center justify-center items-center">
            {FROG_LIST.map((entry) => (
              <div key={entry[0]}>
                <div>
                  <img
                    src={frogImage(entry[0], frogData)}
                    className="w-48 h-48 object-cover mx-auto"
                  />
                </div>
                <div>{frogName(entry[0], frogData)}</div>
                <div>{frogCount(entry[0], frogData)}</div>
              </div>
            ))}
          </div>
        )}
      </div>
    </ScrollArea>
  );
}
