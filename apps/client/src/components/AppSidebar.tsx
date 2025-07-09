import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem
} from "@/components/ui/sidebar";
import {
  startP2pNode,
  sendPodToPeer,
  sendMessageAsPod,
  listPrivateKeys,
  createPrivateKey
} from "@/lib/rpc";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  CodeIcon,
  FileCheck2Icon,
  FileIcon,
  FilePenLineIcon,
  InboxIcon,
  MessageSquareIcon,
  SettingsIcon
} from "lucide-react";
import { useEffect, useState } from "react";
import { useAppStore } from "../lib/store";
import CreateSignedPodDialog from "./CreateSignedPodDialog";
import { Button } from "./ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger
} from "./ui/dropdown-menu";
import { Input } from "./ui/input";

export function AppSidebar() {
  const {
    appState,
    initialize,
    currentView,
    selectedFilter,
    setCurrentView,
    setSelectedFilter,
    setExternalPodRequest,
    chatEnabled
  } = useAppStore();
  const [nodeId, setNodeId] = useState<string | null>(null);
  const [p2pLoading, setP2pLoading] = useState(false);
  const [sendPodForm, setSendPodForm] = useState({
    peerNodeId: "",
    podId: "",
    messageText: "",
    senderAlias: ""
  });
  const [sendMode, setSendMode] = useState<"pod" | "message">("pod");
  const [privateKeys, setPrivateKeys] = useState<any[]>([]);
  const [isCreateSignedPodDialogOpen, setIsCreateSignedPodDialogOpen] =
    useState(false);

  const handlePodRequestFromClipboard = async () => {
    try {
      const clipboardText = await readText();
      setExternalPodRequest(clipboardText);
    } catch (error) {
      console.error("Failed to read from clipboard:", error);
    }
  };

  const handleStartP2P = async () => {
    try {
      setP2pLoading(true);
      const nodeIdResult = await startP2pNode();
      setNodeId(nodeIdResult);
    } catch (error) {
      console.error("Failed to start P2P node:", error);
    } finally {
      setP2pLoading(false);
    }
  };

  const handleSendPod = async () => {
    try {
      if (sendMode === "pod") {
        // Send existing POD
        await sendPodToPeer(
          sendPodForm.peerNodeId,
          sendPodForm.podId,
          undefined, // No message when sending existing POD
          sendPodForm.senderAlias || undefined
        );
        console.log("POD sent successfully");
      } else {
        // Send message as POD (create new POD)
        await sendMessageAsPod(
          sendPodForm.peerNodeId,
          sendPodForm.messageText,
          sendPodForm.senderAlias || undefined
        );
        console.log("Message POD sent successfully");
      }

      // Reset form
      setSendPodForm({
        peerNodeId: "",
        podId: "",
        messageText: "",
        senderAlias: ""
      });
    } catch (error) {
      console.error("Failed to send:", error);
    }
  };

  const handleCopyNodeId = async () => {
    if (nodeId) {
      try {
        await writeText(nodeId);
        console.log("Node ID copied to clipboard");
      } catch (error) {
        console.error("Failed to copy Node ID:", error);
      }
    }
  };

  const loadPrivateKeys = async () => {
    try {
      const keys = await listPrivateKeys();
      setPrivateKeys(keys);
    } catch (error) {
      console.error("Failed to load private keys:", error);
    }
  };

  const handleCreatePrivateKey = async () => {
    try {
      const hasDefault = privateKeys.some((key) => key.is_default);
      await createPrivateKey(
        undefined, // No alias
        !hasDefault // Set as default if no default exists
      );
      console.log("Private key created successfully");
      loadPrivateKeys(); // Refresh the list
    } catch (error) {
      console.error("Failed to create private key:", error);
    }
  };

  useEffect(() => {
    initialize();
    loadPrivateKeys();
  }, [initialize]);

  return (
    <Sidebar>
      <SidebarHeader></SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>PODs</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => {
                    setCurrentView("pods");
                    setSelectedFilter("all");
                  }}
                  isActive={currentView === "pods" && selectedFilter === "all"}
                >
                  <FileIcon />
                  All
                </SidebarMenuButton>
                <SidebarMenuBadge>
                  {appState.pod_stats.total_pods}
                </SidebarMenuBadge>
              </SidebarMenuItem>

              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => {
                    setCurrentView("pods");
                    setSelectedFilter("signed");
                  }}
                  isActive={
                    currentView === "pods" && selectedFilter === "signed"
                  }
                >
                  <FilePenLineIcon />
                  Signed
                </SidebarMenuButton>
                <SidebarMenuBadge>
                  {appState.pod_stats.signed_pods}
                </SidebarMenuBadge>
              </SidebarMenuItem>

              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => {
                    setCurrentView("pods");
                    setSelectedFilter("main");
                  }}
                  isActive={currentView === "pods" && selectedFilter === "main"}
                >
                  <FileCheck2Icon />
                  Main
                </SidebarMenuButton>
                <SidebarMenuBadge>
                  {appState.pod_stats.main_pods}
                </SidebarMenuBadge>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        {chatEnabled && (
          <SidebarGroup>
            <SidebarGroupLabel>Messages</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    onClick={() => setCurrentView("inbox")}
                    isActive={currentView === "inbox"}
                  >
                    <InboxIcon />
                    Inbox
                  </SidebarMenuButton>
                </SidebarMenuItem>

                <SidebarMenuItem>
                  <SidebarMenuButton
                    onClick={() => setCurrentView("chats")}
                    isActive={currentView === "chats"}
                  >
                    <MessageSquareIcon />
                    Chats
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        )}
        <SidebarGroup>
          <SidebarGroupLabel>Actions</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton>
                  <CodeIcon />
                  POD Request
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => setIsCreateSignedPodDialogOpen(true)}
                >
                  <FilePenLineIcon />
                  Sign POD
                </SidebarMenuButton>
                <CreateSignedPodDialog
                  isOpen={isCreateSignedPodDialogOpen}
                  onOpenChange={setIsCreateSignedPodDialogOpen}
                />
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline">
              <SettingsIcon /> Debug
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-64">
            <DropdownMenuItem onClick={handlePodRequestFromClipboard}>
              POD Request from Clipboard
            </DropdownMenuItem>
            <DropdownMenuItem onClick={handleStartP2P} disabled={p2pLoading}>
              {p2pLoading ? "Starting P2P..." : "Start P2P Node"}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={handleCreatePrivateKey}>
              Create Private Key
            </DropdownMenuItem>
            {privateKeys.length > 0 && (
              <DropdownMenuItem disabled>
                Keys: {privateKeys.length} (
                {privateKeys.filter((k) => k.is_default).length} default)
              </DropdownMenuItem>
            )}
            {nodeId && (
              <DropdownMenuItem onClick={handleCopyNodeId}>
                NodeID: {nodeId.slice(0, 16)}... (click to copy)
              </DropdownMenuItem>
            )}
            {nodeId && (
              <>
                <div className="p-2 space-y-2">
                  <div className="flex gap-2">
                    <Button
                      variant={sendMode === "pod" ? "default" : "outline"}
                      onClick={() => setSendMode("pod")}
                      className="flex-1"
                    >
                      Send POD
                    </Button>
                    <Button
                      variant={sendMode === "message" ? "default" : "outline"}
                      onClick={() => setSendMode("message")}
                      className="flex-1"
                    >
                      Send Message
                    </Button>
                  </div>

                  <Input
                    placeholder="Peer Node ID"
                    value={sendPodForm.peerNodeId}
                    onChange={(e) =>
                      setSendPodForm((prev) => ({
                        ...prev,
                        peerNodeId: e.target.value
                      }))
                    }
                  />

                  {sendMode === "pod" && (
                    <Input
                      placeholder="POD ID"
                      value={sendPodForm.podId}
                      onChange={(e) =>
                        setSendPodForm((prev) => ({
                          ...prev,
                          podId: e.target.value
                        }))
                      }
                    />
                  )}

                  {sendMode === "message" && (
                    <Input
                      placeholder="Message text"
                      value={sendPodForm.messageText}
                      onChange={(e) =>
                        setSendPodForm((prev) => ({
                          ...prev,
                          messageText: e.target.value
                        }))
                      }
                    />
                  )}

                  <Input
                    placeholder="Your alias (optional)"
                    value={sendPodForm.senderAlias}
                    onChange={(e) =>
                      setSendPodForm((prev) => ({
                        ...prev,
                        senderAlias: e.target.value
                      }))
                    }
                  />

                  <Button
                    onClick={handleSendPod}
                    disabled={
                      !sendPodForm.peerNodeId ||
                      (sendMode === "pod" && !sendPodForm.podId) ||
                      (sendMode === "message" && !sendPodForm.messageText)
                    }
                    className="w-full"
                  >
                    {sendMode === "pod" ? "Send POD" : "Send Message as POD"}
                  </Button>

                  {sendMode === "message" &&
                    privateKeys.filter((k) => k.is_default).length === 0 && (
                      <div className="text-xs text-orange-600">
                        ⚠️ No default private key found. Create one first.
                      </div>
                    )}
                </div>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
