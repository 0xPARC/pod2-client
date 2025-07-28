import { ScrollArea } from "./ui/scroll-area";
import { ListIcon, Grid3x3Icon } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "./ui/button";
import { getFrogedex, FrogedexEntry } from "@/lib/rpc";
import { listen } from "@tauri-apps/api/event";

const RARITY_NAMES = ["NORM", "RARE", "EPIC", "LGND", "MYTH"];

export function Frogedex() {
  const [frogedex, setFrogedex] = useState<FrogedexEntry[]>([]);

  const [iconView, setIconView] = useState(false);

  async function updateFrogedex() {
    try {
      const data = await getFrogedex();
      setFrogedex(data);
    } catch (e) {}
  }

  useEffect(() => {
    updateFrogedex();
  }, []);

  useEffect(() => {
    const unlisten = listen("refresh-frogs", () => {
      updateFrogedex();
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

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
              {frogedex.map((entry) => (
                <tr key={entry.frog_id}>
                  <td>{entry.frog_id}</td>
                  <td>{RARITY_NAMES[entry.rarity]}</td>
                  <td>{entry.name}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {iconView && (
          <div className="grid grid-cols-3 text-center justify-center items-center">
            {frogedex.map((entry) => (
              <div key={entry.frog_id}>
                <div>
                  <img
                    src={entry.image_url}
                    className="w-48 h-48 object-cover mx-auto"
                  />
                </div>
                <div>{entry.name}</div>
              </div>
            ))}
          </div>
        )}
      </div>
    </ScrollArea>
  );
}
