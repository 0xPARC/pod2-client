import { FrogViewer } from "./FrogViewer";
import { Frogedex } from "./Frogedex";
import { Leaderboard } from "./Leaderboard";
import { Button } from "./ui/button";
import { requestScore, fixFrogDescriptions, reregisterFrogs } from "@/lib/rpc";
import { useEffect } from "react";
import { useFrogCrypto } from "@/lib/store";

// We need to write out all of the text, shadow, etc + color combinations, or tailwind won't generate the right css
export const RARITY_TEXT_COLORS = [
  "text-green-500",
  "text-sky-500",
  "text-purple-500",
  "text-orange-500",
  "text-black"
];
export const RARITY_SHADOW_COLORS = [
  "shadow-[0_0_25px_theme(colors.green.500)]",
  "shadow-[0_0_25px_theme(colors.sky.500)]",
  "shadow-[0_0_25px_theme(colors.purple.500)]",
  "shadow-[0_0_25px_theme(colors.orange.500)]",
  "shadow-[0_0_25px_theme(colors.black.500)]"
];
export const RARITY_BG_COLORS = [
  "bg-green-500",
  "bg-sky-500",
  "bg-purple-500",
  "bg-orange-500",
  "bg-black"
];

export function FrogCrypto() {
  const { currentScreen, navigateToScreen, score, setScore } = useFrogCrypto();
  const frogView = currentScreen == "game";
  const frogedexView = currentScreen == "frogedex";
  const leaderboardView = currentScreen == "leaderboard";
  useEffect(() => {
    async function updateScore() {
      try {
        const scoreResponse = await requestScore();
        setScore(scoreResponse.score);
      } catch (e) {}
    }
    updateScore();
  }, []);
  return (
    <div className="flex flex-col grow h-full">
      <h2 className="text-2xl font-bold">FROGCRYPTO</h2>
      <p>SCORE: {score}</p>
      <div className="flex">
        <Button
          className={`max-w-48 ${frogView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => navigateToScreen("game")}
        >
          get frogs
        </Button>
        <Button
          className={`max-w-48 ${frogedexView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => navigateToScreen("frogedex")}
        >
          frogedex
        </Button>
        <Button
          className={`max-w-48 ${leaderboardView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => navigateToScreen("leaderboard")}
        >
          leaderboard
        </Button>
      </div>
      <Button
        className="max-w-48"
        variant="outline"
        onClick={() => fixFrogDescriptions()}
      >
        reload frog descriptions
      </Button>
      <Button
        className="max-w-48"
        variant="outline"
        onClick={() => reregisterFrogs()}
      >
        reupload mined frogs
      </Button>


      {frogView && <FrogViewer />}
      {frogedexView && <Frogedex />}
      {leaderboardView && <Leaderboard />}
    </div>
  );
}
