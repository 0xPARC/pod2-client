import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { getCurrent } from "@tauri-apps/plugin-deep-link";
import { useEffect, useState } from "react";
import "./App.css";
import { ThemeProvider } from "./components/core/theme-provider";
import { GitHubIdentitySetupModal } from "./components/identity/GitHubIdentitySetupModal";
import { Toaster } from "./components/ui/sonner";
import { useConfigInitialization, useConfigSection } from "./lib/config/hooks";
import { simpleDeepLinkManager, testDeepLink } from "./lib/deeplink-simple";
import { KeyboardProvider } from "./lib/keyboard/KeyboardProvider";
import { queryClient } from "./lib/query";
import { router } from "./lib/router";
import { useAppStore } from "./lib/store";

function App() {
  const { initialize } = useAppStore((state) => state);
  const [isSetupCompleted, setIsSetupCompleted] = useState<boolean | null>(
    null
  );
  const networkConfig = useConfigSection("network");

  // Initialize config store
  useConfigInitialization();

  // Initialize simplified deep-link manager
  useEffect(() => {
    simpleDeepLinkManager.initialize(router);
    simpleDeepLinkManager.startListening();

    return () => {
      simpleDeepLinkManager.stopListening();
    };
  }, [router]);

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

      // Check for deep-link URL that launched the app
      const checkLaunchUrl = async () => {
        try {
          const launchUrls = await getCurrent();
          if (launchUrls && launchUrls.length > 0) {
            console.log("[App] App launched with deep-link URLs:", launchUrls);
            launchUrls.forEach((url) => {
              console.log("[App] Processing launch URL:", url);
              testDeepLink(url);
            });
          }
        } catch (error) {
          console.log("[App] No launch URL or error getting current:", error);
        }
      };

      checkLaunchUrl();
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
      <KeyboardProvider>
        <QueryClientProvider client={queryClient}>
          <div className="h-screen overflow-hidden overscroll-none">
            {/* TODO: Maybe make this MacOS-only? */}
            {/* <div
                data-tauri-drag-region
                className="fixed top-0 left-0 right-0 z-[99]! h-[20px]"
                onDoubleClick={() => {
                  getCurrentWindow().maximize();
                }}
              ></div> */}

            {/* Router renders AppSidebar + route content via file-based Root route */}
            <RouterProvider router={router} context={{ queryClient }} />

            <Toaster />

            {/* Identity Setup Modal - Use GitHub OAuth modal if detected */}
            <GitHubIdentitySetupModal
              open={!isSetupCompleted}
              onComplete={handleSetupComplete}
            />
          </div>
        </QueryClientProvider>
      </KeyboardProvider>
    </ThemeProvider>
  );
}

export default App;
