import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import "./App.css";
import { AppSidebar } from "./components/AppSidebar";
import { GitHubIdentitySetupModal } from "./components/GitHubIdentitySetupModal";
import { MainContent } from "./components/MainContent";
import { ThemeProvider } from "./components/theme-provider";
import { SidebarProvider } from "./components/ui/sidebar";
import { Toaster } from "./components/ui/sonner";
import { useConfigInitialization, useConfigSection } from "./lib/config/hooks";
import { FeatureConfigProvider } from "./lib/features/config";
import { KeyboardProvider } from "./lib/keyboard/KeyboardProvider";
import { useAppStore } from "./lib/store";

function App() {
  const { initialize } = useAppStore((state) => state);
  const [isSetupCompleted, setIsSetupCompleted] = useState<boolean | null>(
    null
  );
  const networkConfig = useConfigSection("network");

  // Initialize config store
  useConfigInitialization();

  // Check if setup is completed and detect GitHub OAuth server
  useEffect(() => {
    const checkSetupStatus = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const completed = await invoke("is_setup_completed");
        setIsSetupCompleted(completed as boolean);

        // GitHub server detection is now handled by the modal
      } catch (error) {
        console.error("Failed to check setup status:", error);
        setIsSetupCompleted(false);
      }
    };

    checkSetupStatus();
  }, [networkConfig]);

  // Initialize the app store when setup is completed
  useEffect(() => {
    if (isSetupCompleted === true) {
      initialize();
    }
  }, [isSetupCompleted, initialize]);

  const handleSetupComplete = () => {
    setIsSetupCompleted(true);
  };

  // Show loading state while checking setup status
  if (isSetupCompleted === null) {
    return (
      <ThemeProvider>
        <div className="h-screen flex items-center justify-center">
          <div className="text-center">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4"></div>
            <p className="text-muted-foreground">Loading...</p>
          </div>
        </div>
      </ThemeProvider>
    );
  }

  return (
    <ThemeProvider>
      <FeatureConfigProvider>
        <KeyboardProvider>
          <div className="h-screen overflow-hidden overscroll-none">
            {/* TODO: Maybe make this MacOS-only? */}
            <div
              data-tauri-drag-region
              className="fixed top-0 left-0 right-0 z-[100]! h-[20px]"
              onDoubleClick={() => {
                getCurrentWindow().maximize();
              }}
            ></div>
            <SidebarProvider className="h-screen">
              <AppSidebar />
              <MainContent />
            </SidebarProvider>
            <Toaster />

            {/* Identity Setup Modal - Use GitHub OAuth modal if detected */}
            <GitHubIdentitySetupModal
              open={!isSetupCompleted}
              onComplete={handleSetupComplete}
            />
          </div>
        </KeyboardProvider>
      </FeatureConfigProvider>
    </ThemeProvider>
  );
}

export default App;
