import { useState } from "react";
import { FrogViewer } from "./FrogViewer";
import { Frogedex } from "./Frogedex";
import { Button } from "./ui/button";
import { requestScore } from "@/lib/rpc";
import { useEffect } from "react";

export function FrogCrypto() {
  const [frogedexView, setFrogedexView] = useState(false);
  const [score, setScore] = useState(5);
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
          className={`max-w-48 ${!frogedexView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => setFrogedexView(false)}
        >
          get frogs
        </Button>
        <Button
          className={`max-w-48 ${frogedexView ? "bg-accent" : ""}`}
          variant="outline"
          onClick={() => setFrogedexView(true)}
        >
          frogedex
        </Button>
      </div>
      {!frogedexView && <FrogViewer setScore={setScore} />}
      {frogedexView && <Frogedex />}
    </div>
  );
}
