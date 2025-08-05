import { getCurrent } from "@tauri-apps/plugin-deep-link";
import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { AppSidebar } from "./components/AppSidebar";
import { GitHubIdentitySetupModal } from "./components/GitHubIdentitySetupModal";
import { MainContent } from "./components/MainContent";
import { ThemeProvider } from "./components/theme-provider";
import { TopBar } from "./components/TopBar";
import { TopBarProvider } from "./components/TopBarContext";
import { SidebarProvider, useSidebar } from "./components/ui/sidebar";
import { Toaster } from "./components/ui/sonner";
import { useConfigInitialization, useConfigSection } from "./lib/config/hooks";
import { testDeepLink, useDeepLinkManager } from "./lib/deeplink";
import { FeatureConfigProvider } from "./lib/features/config";
import { KeyboardProvider } from "./lib/keyboard/KeyboardProvider";
import { createShortcut } from "./lib/keyboard/types";
import { useKeyboardShortcuts } from "./lib/keyboard/useKeyboardShortcuts";
import { useAppStore } from "./lib/store";

// Component that handles global keyboard shortcuts within the sidebar context
function GlobalKeyboardShortcuts() {
  const { toggleSidebar } = useSidebar();

  const globalShortcuts = [
    createShortcut("b", () => toggleSidebar(), "Toggle Sidebar", {
      cmd: true
    })
  ];

  useKeyboardShortcuts(globalShortcuts, {
    enabled: true,
    context: "global"
  });

  return null;
}

function App() {
  const { initialize } = useAppStore((state) => state);
  const [isSetupCompleted, setIsSetupCompleted] = useState<boolean | null>(
    null
  );
  const networkConfig = useConfigSection("network");

  // Initialize config store
  useConfigInitialization();

  // Initialize deep-link manager with navigation functions
  const setActiveApp = useAppStore((state) => state.setActiveApp);
  const navigateToDocumentsList = useAppStore(
    (state) => state.documentsActions.navigateToDocumentsList
  );
  const navigateToDocument = useAppStore(
    (state) => state.documentsActions.navigateToDocument
  );
  const navigateToDrafts = useAppStore(
    (state) => state.documentsActions.navigateToDrafts
  );
  const navigateToPublish = useAppStore(
    (state) => state.documentsActions.navigateToPublish
  );
  const navigateToDebug = useAppStore(
    (state) => state.documentsActions.navigateToDebug
  );

  const navigation = useMemo(
    () => ({
      setActiveApp,
      navigateToDocumentsList,
      navigateToDocument,
      navigateToDrafts,
      navigateToPublish,
      navigateToDebug
    }),
    [
      setActiveApp,
      navigateToDocumentsList,
      navigateToDocument,
      navigateToDrafts,
      navigateToPublish,
      navigateToDebug
    ]
  );

  useDeepLinkManager(navigation);

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
      <FeatureConfigProvider>
        <KeyboardProvider>
          <div className="h-screen overflow-hidden overscroll-none">
            {/* TODO: Maybe make this MacOS-only? */}
            {/* <div
              data-tauri-drag-region
              className="fixed top-0 left-0 right-0 z-[99]! h-[20px]"
              onDoubleClick={() => {
                getCurrentWindow().maximize();
              }}
            ></div> */}

            <SidebarProvider className="h-screen">
              <TopBarProvider>
                <GlobalKeyboardShortcuts />
                <TopBar />
                <AppSidebar />
                <div className="mt-(--top-bar-height) w-full h-full">
                  <MainContent />
                </div>
              </TopBarProvider>
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
