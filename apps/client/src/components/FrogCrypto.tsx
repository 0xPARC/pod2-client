import { useState } from "react";
import { FrogViewer } from "./FrogViewer";
import { Frogedex } from "./Frogedex";
import { Leaderboard } from "./Leaderboard";
import { Button } from "./ui/button";
import { requestScore, fixFrogDescriptions } from "@/lib/rpc";
import { useEffect } from "react";

enum View {
  Frogs,
  Frogedex,
  Leaderboard
}

export function FrogCrypto() {
  const [view, setView] = useState(View.Frogs);
  const [score, setScore] = useState(0);
  const frogView = view == View.Frogs;
  const frogedexView = view == View.Frogedex;
  const leaderboardView = view == View.Leaderboard;
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
          onClick={() => setView(View.Frogs)}
        >
          get frogs
        </Button>
        <Button
          className={`max-w-48 ${frogedexView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => setView(View.Frogedex)}
        >
          frogedex
        </Button>
        <Button
          className={`max-w-48 ${leaderboardView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => setView(View.Leaderboard)}
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

      {frogView && <FrogViewer setScore={setScore} />}
      {frogedexView && <Frogedex />}
      {leaderboardView && <Leaderboard />}
    </div>
  );
}
