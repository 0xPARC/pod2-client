import { cn } from "@/lib/utils";

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
  return (
    <div className={cn("", className)}>
      <div className="mb-6">
        <h3 
          className="text-white text-center"
          style={{
            color: '#FFF',
            fontFamily: 'var(--font-sans)',
            fontSize: '18px',
            fontStyle: 'normal',
            fontWeight: 400,
            lineHeight: '24px',
            letterSpacing: '0.36px'
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
                entry.isCurrentUser && "bg-green-600/20"
              )}
            >
              <span 
                className="text-white"
                style={{
                  color: '#FFF',
                  fontFamily: 'var(--font-sans)',
                  fontSize: '18px',
                  fontStyle: 'normal',
                  fontWeight: 400,
                  lineHeight: '24px',
                  letterSpacing: '0.36px',
                  minWidth: '30px'
                }}
              >
                #{entry.rank}
              </span>
              <span 
                className="text-white ml-2"
                style={{
                  color: '#FFF',
                  fontFamily: 'var(--font-sans)',
                  fontSize: '18px',
                  fontStyle: 'normal',
                  fontWeight: 400,
                  lineHeight: '24px',
                  letterSpacing: '0.36px'
                }}
              >
                {entry.username}
              </span>
              <span 
                className="text-green-400 ml-auto"
                style={{
                  color: '#4ade80',
                  fontFamily: 'var(--font-sans)',
                  fontSize: '18px',
                  fontStyle: 'normal',
                  fontWeight: 400,
                  lineHeight: '24px',
                  letterSpacing: '0.36px'
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
            color: '#FFF',
            fontFamily: 'var(--font-sans)',
            fontSize: '18px',
            fontStyle: 'normal',
            fontWeight: 400,
            lineHeight: '24px',
            letterSpacing: '0.36px',
            opacity: 0.6
          }}
        >
          No scores yet. Be the first to play!
        </div>
      )}
    </div>
  );
}
