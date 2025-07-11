import { useState } from "react";
import { FrogViewer } from "./FrogViewer";
import { Frogedex } from "./Frogedex";
import { Button } from "./ui/button";

export function FrogCrypto() {
  const [frogedexView, setFrogedexView] = useState(false);
  return (
    <div className="flex flex-col grow h-full">
      <h2 className="text-2xl font-bold">FROGCRYPTO</h2>
      <div className="flex">
        <Button
          className="max-w-48"
          variant="outline"
          onClick={() => setFrogedexView(false)}
        >
          get frogs
        </Button>
        <Button
          className="max-w-48"
          variant="outline"
          onClick={() => setFrogedexView(true)}
        >
          frogedex
        </Button>
      </div>
      {!frogedexView && <FrogViewer />}
      {frogedexView && <Frogedex />}
    </div>
  );
}
