import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { useCallback, useEffect, useState } from "react";
import "./App.css";
import { AppSidebar } from "./components/AppSidebar";
import { MainContent } from "./components/MainContent";
import { Button } from "./components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./components/ui/dialog";
import { SidebarProvider } from "./components/ui/sidebar";
import { Textarea } from "./components/ui/textarea";
import { useAppStore } from "./lib/store";

type ResultStatus = "success" | "error" | "pending";

function PodRequestDialog() {
  const { externalPodRequest, setExternalPodRequest } = useAppStore((state) => state);
  const [resultStatus, setResultStatus] = useState<ResultStatus | undefined>(undefined);

  const submitPodRequest = useCallback(
    async (requestText: string) => {
      try {
        setResultStatus("pending");
        const result = await invoke("submit_pod_request", {
          request: requestText,
        });
        await writeText(JSON.stringify(result));
        setResultStatus("success");
      } catch (error) {
        console.error(error);
        setResultStatus("error");
      }
    },
    [],
  );

  const handleSubmit = () => {
    if (externalPodRequest) {
      submitPodRequest(externalPodRequest);
    }
  };
  return <Dialog open={externalPodRequest !== undefined} onOpenChange={(open) => {
    if (!open) {
      setExternalPodRequest(undefined);
    }
  }}>
    <DialogContent>
      <DialogHeader>
        <DialogTitle>POD Request</DialogTitle>
      </DialogHeader>
      <DialogDescription>
        Enter your POD request here.
      </DialogDescription>
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
  </Dialog>;
}

function App() {

  const { setExternalPodRequest } = useAppStore((state) => state);

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

  return (
    <div className="h-screen overflow-hidden overscroll-none">
      {/* TODO: Maybe make this MacOS-only? */}
      <div
        data-tauri-drag-region
        className="fixed top-0 left-0 right-0 z-50 h-[20px]"
        onDoubleClick={() => {
          getCurrentWindow().toggleMaximize();
        }}
      ></div>
      <SidebarProvider className="h-screen">
        <AppSidebar />
        <MainContent />
        <PodRequestDialog />
      </SidebarProvider>
    </div>
  );
}

export default App;
