import { Button } from "@/components/ui/button";
import { FrogedexItem, RarityType } from "@/components/ui/frogcrypto";
import { FrogedexEntry, getFrogedex } from "@/lib/rpc";
import { listen } from "@tauri-apps/api/event";
import { Grid3x3Icon, ListIcon } from "lucide-react";
import { useEffect, useState } from "react";

const RARITY_NAMES: RarityType[] = [
  "NORM",
  "RARE",
  "EPIC",
  "LGND",
  "MYTH",
  "MYTH",
  "GOD",
  "GOD",
  "????",
  "ART",
  "JUNK"
];

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
    <div className="h-full flex flex-col">
      <div className="p-6 flex-shrink-0">
        {/* View Toggle Buttons */}
        <div className="flex gap-3 mb-6 justify-center">
          <Button
            className={`px-6 py-3 rounded-lg font-semibold transition-all duration-200 ${
              !iconView
                ? "bg-green-600 hover:bg-green-700 text-white shadow-lg"
                : "hover:bg-white/30 text-white border border-white/30"
            }`}
            onClick={() => setIconView(false)}
            style={{
              backgroundColor: !iconView ? undefined : "#325F58"
            }}
          >
            <ListIcon className="w-4 h-4 mr-2" />
            List View
          </Button>
          <Button
            className={`px-6 py-3 rounded-lg font-semibold transition-all duration-200 ${
              iconView
                ? "bg-green-600 hover:bg-green-700 text-white shadow-lg"
                : "hover:bg-white/30 text-white border border-white/30"
            }`}
            onClick={() => setIconView(true)}
            style={{
              backgroundColor: iconView ? undefined : "#325F58"
            }}
          >
            <Grid3x3Icon className="w-4 h-4 mr-2" />
            Grid View
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-6 pb-6">
        {/* List View */}
        {!iconView && (
          <div className="flex justify-center">
            <div className="overflow-x-auto">
              <div className="min-w-full max-w-md">
                {frogedex.map((entry) => (
                  <FrogedexItem
                    key={entry.frog_id}
                    frogId={String(entry.frog_id)}
                    name={entry.name}
                    rarity={RARITY_NAMES[entry.rarity]}
                    seen={entry.seen}
                    imageUrl={entry.image_url}
                  />
                ))}
              </div>
            </div>
          </div>
        )}

        {/* Grid View */}
        {iconView && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
            {frogedex.map((entry) => (
              <div
                key={entry.frog_id}
                className="bg-white/10 backdrop-blur-sm rounded-xl border border-white/20 p-4 hover:bg-white/20 transition-all duration-300 group"
              >
                <div className="text-center">
                  <div className="mb-4">
                    <img
                      src={entry.image_url}
                      className="w-32 h-32 object-cover mx-auto rounded-lg shadow-lg group-hover:scale-105 transition-transform duration-300"
                      alt={entry.name}
                    />
                  </div>
                  <div className="text-lg font-bold text-white">
                    {entry.seen ? entry.name : "???"}
                  </div>
                  <div className="text-white/80 text-sm mt-1">
                    ID: {entry.frog_id}
                  </div>
                  <div className="mt-2">
                    <FrogedexItem
                      frogId={String(entry.frog_id)}
                      name={entry.name}
                      rarity={RARITY_NAMES[entry.rarity]}
                      seen={entry.seen}
                      imageUrl={entry.image_url}
                      className="justify-center p-0 border-0 hover:bg-transparent"
                    />
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
