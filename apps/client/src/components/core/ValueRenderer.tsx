import type {
  Dictionary,
  Array as PodArray,
  Set as PodSet,
  RawValue,
  Value
} from "@pod2/pod2js";
import React, { useState } from "react";
import { Badge } from "../ui/badge";
import { PublicKeyAvatar } from "./PublicKeyAvatar";

interface ValueRendererProps {
  value: Value;
}

const SecretKeyRenderer: React.FC<{ secretKey: string }> = ({ secretKey }) => {
  const [isHidden, setIsHidden] = useState(true);
  const toggleHidden = () => setIsHidden(!isHidden);

  return (
    <span className="font-mono text-red-600 dark:text-red-400 flex items-center gap-2">
      <div className="h-[32px] w-[32px] flex items-center justify-center">
        <div className="text-lg">🤫</div>
      </div>
      {isHidden ? "*".repeat(secretKey.length) : secretKey}
      <button onClick={toggleHidden} className="text-xs text-muted-foreground">
        {isHidden ? "Show" : "Hide"}
      </button>
    </span>
  );
};

const ValueRenderer: React.FC<ValueRendererProps> = ({ value }) => {
  if (value === null || value === undefined) {
    return <span className="text-gray-400 dark:text-gray-500">N/A</span>;
  }

  if (typeof value === "string") {
    return <span title={value}>{value}</span>; // Render string directly
  }

  if (typeof value === "boolean") {
    return (
      <span className="font-mono text-purple-600 dark:text-purple-400">
        {value.toString()}
      </span>
    );
  }

  if (typeof value === "object") {
    if ("Int" in value) {
      return (
        <span className="font-mono text-green-600 dark:text-green-400">
          {String((value as { Int: string }).Int)}
        </span>
      );
    }
    if ("Raw" in value) {
      return (
        <span
          className="italic text-gray-700 dark:text-gray-300"
          title={(value as { Raw: RawValue }).Raw}
        >
          {(value as { Raw: RawValue }).Raw}
        </span>
      );
    }
    if ("PublicKey" in value) {
      return (
        <span className="font-mono text-blue-600 dark:text-blue-400 flex items-center gap-2">
          <PublicKeyAvatar publicKey={value.PublicKey} size={32} />
          {value.PublicKey}
        </span>
      );
    }
    if ("SecretKey" in value) {
      return <SecretKeyRenderer secretKey={value.SecretKey} />;
    }
    if ("Root" in value) {
      return (
        <span className="font-mono text-blue-600 dark:text-blue-400 flex items-center gap-2">
          <Badge className="px-1 w-[32px]">Root</Badge>
          0x{value.Root}
        </span>
      );
    }
    if ("array" in value) {
      let arr = value as PodArray;
      if (arr.array.length === 0)
        return (
          <span className="font-mono text-blue-600 dark:text-blue-400">[]</span>
        );
      return (
        <div className="font-mono text-blue-600 dark:text-blue-400">
          <div>[</div>
          {arr.array.map((item, index) => (
            <div key={index} className="pl-4">
              <ValueRenderer value={item} />
              {index < arr.array.length - 1 ? "," : ""}
            </div>
          ))}
          <div>]</div>
        </div>
      );
    }
    if ("set" in value) {
      const set = value as PodSet;
      if (set.set.length === 0)
        return (
          <span className="font-mono text-orange-600 dark:text-orange-400">
            Set([])
          </span>
        );
      return (
        <div className="font-mono text-orange-600 dark:text-orange-400">
          <div>Set([</div>
          {set.set.map((item, index) => (
            <div key={index} className="pl-4">
              <ValueRenderer value={item} />
              {index < set.set.length - 1 ? "," : ""}
            </div>
          ))}
          <div>])</div>
        </div>
      );
    }
    if ("kvs" in value) {
      const dict = value as Dictionary;
      const entries = Object.entries(dict.kvs);
      if (entries.length === 0)
        return (
          <span className="font-mono text-indigo-600 dark:text-indigo-400">
            {"{}"}
          </span>
        );
      return (
        <div className="font-mono text-indigo-600 dark:text-indigo-400">
          <div>{"{"}</div>
          {entries.map(([key, val], index) => (
            <div key={key} className="pl-4">
              <span className="text-red-500 dark:text-red-400">{key}</span>:{" "}
              <ValueRenderer value={val} />
              {index < entries.length - 1 ? "," : ""}
            </div>
          ))}
          <div>{"}"}</div>
        </div>
      );
    }
  }

  // Fallback for unknown types or complex structures not yet handled
  return (
    <span className="text-xs text-gray-500 dark:text-gray-400">
      {JSON.stringify(value)}
    </span>
  );
};

export default ValueRenderer;
