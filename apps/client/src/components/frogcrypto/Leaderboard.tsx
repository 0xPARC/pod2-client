import { LeaderboardTable } from "@/components/ui/frogcrypto";
import { requestLeaderboard, LeaderboardItem } from "@/lib/rpc";
import { useEffect, useState } from "react";

export function Leaderboard() {
  const [leaderboard, setLeaderboard] = useState([] as LeaderboardItem[]);

  useEffect(() => {
    async function updateLeaderboard() {
      setLeaderboard(await requestLeaderboard());
    }
    updateLeaderboard();
  }, []);

  // Transform data for the LeaderboardTable component
  const transformedEntries = leaderboard.map((entry, index) => ({
    rank: index + 1,
    username: entry.username,
    score: entry.score,
    isCurrentUser: false // You might want to add logic to determine current user
  }));

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-8">
          <LeaderboardTable entries={transformedEntries} />
        </div>
      </div>
    </div>
  );
}
