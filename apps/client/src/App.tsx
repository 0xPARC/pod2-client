import { submitPodRequest } from "./lib/rpc";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { useCallback, useEffect, useState } from "react";
import "./App.css";
import { AppSidebar } from "./components/AppSidebar";
import { MainContent } from "./components/MainContent";
import { IdentitySetupModal } from "./components/IdentitySetupModal";
import { GitHubIdentitySetupModal } from "./components/GitHubIdentitySetupModal";
import { Button } from "./components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "./components/ui/dialog";
import { SidebarProvider } from "./components/ui/sidebar";
import { Textarea } from "./components/ui/textarea";
import { Toaster } from "./components/ui/sonner";
import { useAppStore } from "./lib/store";
import { useConfigInitialization, useConfigSection } from "./lib/config/hooks";
import { setupGitHubIdentityServer } from "./lib/github-oauth";
import { FeatureConfigProvider } from "./lib/features/config";
import { ThemeProvider } from "./components/theme-provider";

type ResultStatus = "success" | "error" | "pending";

function PodRequestDialog() {
  const { externalPodRequest, setExternalPodRequest } = useAppStore(
    (state) => state
  );
  const [resultStatus, setResultStatus] = useState<ResultStatus | undefined>(
    undefined
  );

  const submitPodRequestHandler = useCallback(async (requestText: string) => {
    try {
      setResultStatus("pending");
      const result = await submitPodRequest(requestText);
      await writeText(JSON.stringify(result));
      setResultStatus("success");
    } catch (error) {
      console.error(error);
      setResultStatus("error");
    }
  }, []);

  const handleSubmit = () => {
    if (externalPodRequest) {
      submitPodRequestHandler(externalPodRequest);
    }
  };
  return (
    <Dialog
      open={externalPodRequest !== undefined}
      onOpenChange={(open) => {
        if (!open) {
          setExternalPodRequest(undefined);
        }
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>POD Request</DialogTitle>
        </DialogHeader>
        <DialogDescription>Enter your POD request here.</DialogDescription>
        <Textarea
          id="pod-request"
          rows={10}
          value={externalPodRequest}
          onChange={(e) => setExternalPodRequest(e.target.value)}
        />
        {resultStatus === "success" && (
          <div className="text-green-500">
            POD created! Result copied to clipboard.
          </div>
        )}
        {resultStatus === "error" && (
          <div className="text-red-500">Request failed</div>
        )}
        {resultStatus === "pending" && (
          <div className="text-yellow-500">
            Please wait while we create your POD...
          </div>
        )}
        <DialogFooter>
          <Button onClick={handleSubmit} disabled={resultStatus === "pending"}>
            Submit
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function App() {
  const { setExternalPodRequest, initialize } = useAppStore((state) => state);
  const [isSetupCompleted, setIsSetupCompleted] = useState<boolean | null>(
    null
  );
  const [isGitHubServer, setIsGitHubServer] = useState<boolean | null>(null);
  const networkConfig = useConfigSection("network");

  // Initialize config store
  useConfigInitialization();

  useEffect(() => {
    onOpenUrl((urls) => {
      if (urls.length > 0) {
        const url = new URL(urls[0]);
        const request = url.searchParams.get("request");
        if (request) {
          setExternalPodRequest(request);
        }
      }
    });
  }, []);

  // Check if setup is completed and detect GitHub OAuth server
  useEffect(() => {
    const checkSetupStatus = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const completed = await invoke("is_setup_completed");
        setIsSetupCompleted(completed as boolean);

        // If setup is not completed and we have network config, check if it's a GitHub server
        if (!completed && networkConfig?.identity_server) {
          try {
            const { is_github_server } = await setupGitHubIdentityServer(
              networkConfig.identity_server
            );
            setIsGitHubServer(is_github_server);
          } catch (error) {
            console.log(
              "Server detection failed, will use default setup:",
              error
            );
            setIsGitHubServer(false);
          }
        }
      } catch (error) {
        console.error("Failed to check setup status:", error);
        setIsSetupCompleted(false);
        setIsGitHubServer(false);
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
  if (
    isSetupCompleted === null ||
    (!isSetupCompleted && isGitHubServer === null)
  ) {
    return (
      <ThemeProvider>
        <div className="h-screen flex items-center justify-center">
          <div className="text-center">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4"></div>
            <p className="text-muted-foreground">
              {isSetupCompleted === null
                ? "Loading..."
                : "Detecting identity server..."}
            </p>
          </div>
        </div>
      </ThemeProvider>
    );
  }

  return (
    <ThemeProvider>
      <FeatureConfigProvider>
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
            <PodRequestDialog />
          </SidebarProvider>
          <Toaster />

          {/* Identity Setup Modal - Use GitHub OAuth modal if detected */}
          {isGitHubServer ? (
            <GitHubIdentitySetupModal
              open={!isSetupCompleted}
              onComplete={handleSetupComplete}
            />
          ) : (
            <IdentitySetupModal
              open={!isSetupCompleted}
              onComplete={handleSetupComplete}
            />
          )}
        </div>
      </FeatureConfigProvider>
    </ThemeProvider>
  );
}

export default App;
