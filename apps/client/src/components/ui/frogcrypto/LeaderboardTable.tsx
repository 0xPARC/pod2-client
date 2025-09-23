import { cn } from "@/lib/utils";
import { useState } from "react";

function truncateAddress(address: string): string {
  if (address.length <= 30) return address;
  return `${address.slice(0, 8)}...${address.slice(-4)}`;
}

async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (error) {
    console.error("Failed to copy to clipboard:", error);
    return false;
  }
}

interface LeaderboardEntry {
  rank: number;
  username: string;
  score: number;
  isCurrentUser?: boolean;
}

interface LeaderboardTableProps {
  entries: LeaderboardEntry[];
  currentUser?: string;
  className?: string;
}

export function LeaderboardTable({
  entries,
  currentUser,
  className
}: LeaderboardTableProps) {
  const [copiedAddress, setCopiedAddress] = useState<string | null>(null);

  const handleAddressClick = async (address: string) => {
    const success = await copyToClipboard(address);
    if (success) {
      setCopiedAddress(address);
      setTimeout(() => setCopiedAddress(null), 2000);
    }
  };

  return (
    <div className={cn("", className)}>
      <div className="mb-6">
        <h3
          className="text-white text-center"
          style={{
            color: "#FFF",
            fontFamily: "var(--font-sans)",
            fontSize: "18px",
            fontStyle: "normal",
            fontWeight: 400,
            lineHeight: "24px",
            letterSpacing: "0.36px"
          }}
        >
          LEADERBOARD
        </h3>
      </div>

      <div className="overflow-y-auto max-h-96">
        <div className="max-w-md mx-auto">
          {entries.map((entry) => (
            <div
              key={entry.rank}
              className={cn(
                "flex items-center py-2",
                (entry.isCurrentUser || entry.username === currentUser) &&
                  "bg-green-600/20"
              )}
            >
              <span
                className="text-white"
                style={{
                  color: "#FFF",
                  fontFamily: "var(--font-sans)",
                  fontSize: "18px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "24px",
                  letterSpacing: "0.36px",
                  minWidth: "30px"
                }}
              >
                #{entry.rank}
              </span>
              <button
                className="text-white ml-2 hover:text-gray-300 transition-colors cursor-pointer text-left"
                onClick={() => handleAddressClick(entry.username)}
                title={
                  copiedAddress === entry.username
                    ? "Copied!"
                    : "Click to copy full address"
                }
                style={{
                  color: copiedAddress === entry.username ? "#4ade80" : "#FFF",
                  fontFamily: "var(--font-sans)",
                  fontSize: "18px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "24px",
                  letterSpacing: "0.36px",
                  background: "none",
                  border: "none",
                  padding: 0
                }}
              >
                {copiedAddress === entry.username
                  ? "Copied!"
                  : truncateAddress(entry.username)}
              </button>
              <span
                className="text-green-400 ml-auto"
                style={{
                  color: "#4ade80",
                  fontFamily: "var(--font-sans)",
                  fontSize: "18px",
                  fontStyle: "normal",
                  fontWeight: 400,
                  lineHeight: "24px",
                  letterSpacing: "0.36px"
                }}
              >
                {entry.score.toLocaleString()}
              </span>
            </div>
          ))}
        </div>
      </div>

      {entries.length === 0 && (
        <div
          className="p-8 text-center text-white/60"
          style={{
            color: "#FFF",
            fontFamily: "var(--font-sans)",
            fontSize: "18px",
            fontStyle: "normal",
            fontWeight: 400,
            lineHeight: "24px",
            letterSpacing: "0.36px",
            opacity: 0.6
          }}
        >
          No scores yet. Be the first to play!
        </div>
      )}
    </div>
  );
}
