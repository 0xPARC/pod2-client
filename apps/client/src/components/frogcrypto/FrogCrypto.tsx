import { FrogViewer } from "./FrogViewer";
import { Frogedex } from "./Frogedex";
import { Leaderboard } from "./Leaderboard";
import { 
  FrogCryptoLogo, 
  NavigationTabs, 
  SearchSwampButton, 
  MiningToggle 
} from "@/components/ui/frogcrypto";
import { requestScore, requestFrog, fixFrogDescriptions, reregisterFrogs } from "@/lib/rpc";
import { useEffect, useState } from "react";
import { useFrogCrypto } from "@/lib/store";
import { toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import "./FrogCrypto.css";

// We need to write out all of the text, shadow, etc + color combinations, or tailwind won't generate the right css
export const RARITY_TEXT_COLORS = [
  "text-green-500",
  "text-sky-500",
  "text-purple-500",
  "text-orange-500",
  "text-rose-500",
  "text-gray-500",
  "text-yellow-500",
  "text-gray-700",
  "text-teal-500",
  "text-black",
  "text-black"
];
export const RARITY_SHADOW_COLORS = [
  "shadow-[0_0_25px_theme(colors.green.500)]",
  "shadow-[0_0_25px_theme(colors.sky.500)]",
  "shadow-[0_0_25px_theme(colors.purple.500)]",
  "shadow-[0_0_25px_theme(colors.orange.500)]",
  "shadow-[0_0_25px_theme(colors.rose.500)]",
  "shadow-[0_0_25px_theme(colors.gray.500)]",
  "shadow-[0_0_25px_theme(colors.yellow.500)]",
  "shadow-[0_0_25px_theme(colors.gray.700)]",
  "shadow-[0_0_25px_theme(colors.teal.500)]",
  "shadow-[0_0_25px_theme(colors.black)]",
  "shadow-[0_0_25px_theme(colors.black)]"
];
export const RARITY_BG_COLORS = [
  "bg-green-500",
  "bg-sky-500",
  "bg-purple-500",
  "bg-orange-500",
  "bg-rose-500",
  "bg-gray-500",
  "bg-yellow-500",
  "bg-gray-700",
  "bg-teal-500",
  "bg-black",
  "bg-black"
];

export function FrogCrypto() {
  const { currentScreen, navigateToScreen, score, setScore, mining, setMining, frogTimeout, setFrogTimeout, hashesChecked, setHashesChecked } = useFrogCrypto();
  const frogView = currentScreen == "game";
  const frogedexView = currentScreen == "frogedex";
  const leaderboardView = currentScreen == "leaderboard";
  const gymView = currentScreen == "gym";
  const breedingView = currentScreen == "breeding";
  
  const [time, setTime] = useState(new Date().getTime());

  
  useEffect(() => {
    async function updateScore() {
      try {
        const scoreResponse = await requestScore();
        setScore(scoreResponse.score);
        if (scoreResponse.timeout > 0) {
          setFrogTimeout(new Date().getTime() + 1000 * scoreResponse.timeout);
        }
      } catch (e) {}
    }
    updateScore();
  }, [setScore, setFrogTimeout]);

  useEffect(() => {
    const interval = setInterval(() => setTime(new Date().getTime()), 1000);
    return () => {
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    const unlisten = listen("frog-background", (event) => {
      setHashesChecked(`${event.payload}`);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, [setHashesChecked]);

  const handleSearchSwamp = async () => {
    try {
      const newScore = await requestFrog();
      setScore(newScore);
      // Set 15-minute timeout (900000ms)
      setFrogTimeout(new Date().getTime() + 900000);
    } catch (e) {
      if (e instanceof Error) {
        toast.error(e.toString());
      }
    }
  };

  // Calculate remaining time and wait text
  const timeRemaining = frogTimeout === null || time >= frogTimeout
    ? 0
    : Math.ceil(0.001 * (frogTimeout - time));
  const searchDisabled = timeRemaining > 0;
  
  function waitText(timeRemaining: number) {
    const mins = Math.floor(timeRemaining / 60);
    const secs = timeRemaining % 60;
    const minsText = mins == 0 ? "" : ` ${mins}m`;
    const secsText = secs == 0 ? "" : ` ${secs}s`;
    return ` (wait ${minsText}${secsText})`;
  }
  
  const searchButtonWaitText = searchDisabled ? waitText(timeRemaining) : "";

  return (
        <div
      className="flex flex-col h-full relative frogcrypto-container"
      style={{
        background: '#293231',
        boxShadow: '0 4px 4px 0 rgba(0, 0, 0, 0.25)'
      }}
    >
      {/* Header Section */}
      <header className="relative z-10 px-8 py-2 flex-shrink-0">
        {/* Logo and Score */}
        <div className="text-center mb-4">
          <FrogCryptoLogo className="mb-1" />
          <p 
            className="text-white text-base"
            style={{
              fontFamily: 'var(--font-sans)',
              fontSize: '16px',
              fontWeight: 400
            }}
          >
            Score: {score}
          </p>
        </div>

        {/* Navigation Tabs */}
        <div className="mb-4">
          <NavigationTabs 
            activeTab={currentScreen}
            onTabChange={(tab) => navigateToScreen(tab as any)}
          />
        </div>

                 {/* Utility Buttons - Second Row */}
         <div className="flex justify-center gap-2 mb-4">
           <button
             onClick={() => fixFrogDescriptions()}
             className="px-3 py-1.5 text-xs font-medium text-white hover:bg-gray-500 rounded transition-colors duration-200"
             style={{
               fontFamily: 'var(--font-sans)',
               fontSize: '12px',
               fontWeight: 500,
               letterSpacing: '0.1px',
               backgroundColor: '#325F58'
             }}
           >
             reload descriptions
           </button>
           <button
             onClick={() => reregisterFrogs()}
             className="px-3 py-1.5 text-xs font-medium text-white hover:bg-gray-500 rounded transition-colors duration-200"
             style={{
               fontFamily: 'var(--font-sans)',
               fontSize: '12px',
               fontWeight: 500,
               letterSpacing: '0.1px',
               backgroundColor: '#325F58'
             }}
           >
             reupload frogs
           </button>
         </div>

        {/* Search Swamp Button */}
        <div className="flex justify-center mb-3">
          <SearchSwampButton 
            onClick={handleSearchSwamp}
            disabled={searchDisabled}
            loading={false}
            waitText={searchButtonWaitText}
          />
        </div>

        {/* Mining Toggle */}
        <div className="flex justify-center">
          <MiningToggle 
            enabled={mining}
            onToggle={setMining}
            hashCount={hashesChecked}
          />
        </div>
      </header>

      {/* Content Area - Scrollable */}
      <main className="flex-1 relative z-10 pt-2 px-8 pb-2 overflow-auto">
        {frogView && <FrogViewer />}
        {frogedexView && <Frogedex />}
        {leaderboardView && <Leaderboard />}
        {gymView && <ComingSoon title="Frog Gym" />}
        {breedingView && <ComingSoon title="Breeding" />}
      </main>
    </div>
  );
}

// Coming Soon component for gym and breeding
function ComingSoon({ title }: { title: string }) {
  return (
    <div className="h-full flex items-center justify-center">
      <div 
        className="text-white text-center coming-soon-container"
        style={{
          color: '#FFF',
          fontFamily: 'var(--font-sans)',
          fontSize: '32px',
          fontStyle: 'normal',
          fontWeight: 600,
          lineHeight: '40px',
          letterSpacing: '1px',
          transform: 'rotate(-3deg)',
          textShadow: '2px 2px 4px rgba(0, 0, 0, 0.3)'
        }}
      >
        {title}
        <br />
        <span 
          className="coming-soon-subtitle"
          style={{
            fontSize: '18px',
            fontWeight: 400,
            letterSpacing: '0.5px',
            opacity: 0.8
          }}
        >
          Coming Soon
        </span>
      </div>
    </div>
  );
}
