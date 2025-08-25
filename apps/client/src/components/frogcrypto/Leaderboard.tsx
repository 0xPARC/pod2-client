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

  return (
    <div>
      <table className="[&_td]:px-4">
        <tbody>
          {leaderboard.map((entry) => (
            <tr key={entry.username}>
              <td>{entry.username}</td>
              <td>{entry.score}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
