import { FrogCard, RarityType } from "@/components/ui/frogcrypto";
import { listFrogs, Frog } from "@/lib/rpc";
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

type FrogPod = Frog;

// Map rarity indices to rarity types
const RARITY_MAP: RarityType[] = [
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

export function FrogViewer() {
  const [frogs, setFrogs] = useState<FrogPod[]>([]);

  async function updateFrogs() {
    try {
      const frogList = await listFrogs();
      setFrogs(frogList);
    } catch (e) {}
  }

  useEffect(() => {
    updateFrogs();
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
    const unlisten = listen("refresh-frogs", () => {
      updateFrogs();
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Get rarity type from frog data
  const getRarityType = (frog: Frog): RarityType => {
    if (frog.derived && "rarity" in frog.derived) {
      const rarityIndex = (frog.derived as any).rarity;
      return RARITY_MAP[rarityIndex] || "NORM";
    }
    return "NORM";
  };

  return (
    <div className="h-full flex flex-col">
      {/* Frogs Grid - accounting for sidebar width (16rem = 256px) */}
      <div className="flex-1 overflow-y-auto">
        <div className="flex justify-center">
          <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 gap-4 sm:gap-6 p-4 sm:p-6 max-w-fit">
            {frogs.map((frog, index) => (
              <FrogCard
                key={frog.id}
                frog={frog}
                rarity={getRarityType(frog)}
                index={index}
              />
            ))}
            {/* Add placeholder cards for empty slots */}
            {Array.from({ length: Math.max(0, 6 - frogs.length) }).map(
              (_, index) => (
                <EmptyFrogCard key={`empty-${index}`} />
              )
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// Empty placeholder card for unfilled slots
function EmptyFrogCard() {
  return (
    <div
      className="relative bg-white rounded-xl border-2 border-white shadow-lg"
      style={{
        width: "280px",
        height: "400px"
      }}
    >
      {/* Green Header */}
      <div className="absolute top-0 left-0 right-0 h-12 bg-green-600 rounded-t-xl flex items-center justify-center">
        <div
          className="text-white font-semibold text-base tracking-wide"
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: "16px",
            fontWeight: 600,
            lineHeight: "20px",
            letterSpacing: "1px"
          }}
        >
          #?? UNKNOWN FROG
        </div>
      </div>

      {/* Small question mark placeholder */}
      <div className="absolute top-20 left-4 right-4 flex items-center justify-center">
        <div className="w-16 h-16 bg-blue-100 rounded flex items-center justify-center">
          <div
            className="text-2xl text-blue-600 font-bold"
            style={{
              fontFamily: "SF Pro Display"
            }}
          >
            ?
          </div>
        </div>
      </div>

      {/* Stats Section */}
      <div className="absolute bottom-16 left-0 right-0 px-4">
        <div className="space-y-3 px-8">
          {/* First row: JUMP, VIBE */}
          <div className="grid grid-cols-2 gap-6">
            <div className="text-center">
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 600,
                  lineHeight: "normal",
                  letterSpacing: "0.28px",
                  textTransform: "uppercase"
                }}
              >
                JUMP
              </div>
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "normal",
                  letterSpacing: "0.28px"
                }}
              >
                ??
              </div>
            </div>

            <div className="text-center">
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 600,
                  lineHeight: "normal",
                  letterSpacing: "0.28px",
                  textTransform: "uppercase"
                }}
              >
                VIBE
              </div>
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "normal",
                  letterSpacing: "0.28px"
                }}
              >
                ??
              </div>
            </div>
          </div>

          {/* Second row: SPED, INTL, BUTY */}
          <div className="grid grid-cols-3 gap-4">
            <div className="text-center">
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 600,
                  lineHeight: "normal",
                  letterSpacing: "0.28px",
                  textTransform: "uppercase"
                }}
              >
                SPED
              </div>
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "normal",
                  letterSpacing: "0.28px"
                }}
              >
                ??
              </div>
            </div>

            <div className="text-center">
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 600,
                  lineHeight: "normal",
                  letterSpacing: "0.28px",
                  textTransform: "uppercase"
                }}
              >
                INTL
              </div>
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "normal",
                  letterSpacing: "0.28px"
                }}
              >
                ??
              </div>
            </div>

            <div className="text-center">
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 600,
                  lineHeight: "normal",
                  letterSpacing: "0.28px",
                  textTransform: "uppercase"
                }}
              >
                BUTY
              </div>
              <div
                style={{
                  color: "#293231",
                  textAlign: "center",
                  fontFamily: "var(--font-sans)",
                  fontSize: "14px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "normal",
                  letterSpacing: "0.28px"
                }}
              >
                ??
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* See more link */}
      <div className="absolute bottom-4 left-0 right-0 text-center">
        <div
          className="text-teal-600 text-sm font-medium hover:text-teal-700 transition-colors"
          style={{
            fontFamily: "var(--font-sans)",
            fontSize: "14px",
            fontWeight: 500
          }}
        >
          See more
        </div>
      </div>
    </div>
  );
}
