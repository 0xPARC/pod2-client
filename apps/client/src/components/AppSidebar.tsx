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
  getPrivateKeyInfo,
  insertZuKycPods,
  PrivateKeyInfo,
  sendMessageAsPod,
  sendPodToPeer,
  startP2pNode
} from "@/lib/rpc";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@radix-ui/react-collapsible";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  CodeIcon,
  FileCheck2Icon,
  FileIcon,
  FilePenLineIcon,
  FolderIcon,
  InboxIcon,
  MessageSquareIcon,
  SettingsIcon,
  StarIcon
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
    selectedFolderFilter,
    folders,
    foldersLoading,
    setCurrentView,
    setSelectedFilter,
    setSelectedFolderFilter,
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
  const [privateKeyInfo, setPrivateKeyInfo] = useState<PrivateKeyInfo | null>(
    null
  );
  const [isCreateSignedPodDialogOpen, setIsCreateSignedPodDialogOpen] =
    useState(false);
  const [foldersExpanded, setFoldersExpanded] = useState(true);

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

  const loadPrivateKeyInfo = async () => {
    try {
      const keyInfo = await getPrivateKeyInfo();
      setPrivateKeyInfo(keyInfo);
    } catch (error) {
      console.error("Failed to load private key info:", error);
    }
  };

  const handleCopyPublicKey = async () => {
    if (privateKeyInfo?.public_key) {
      try {
        await writeText(privateKeyInfo.public_key);
        console.log("Public key copied to clipboard");
      } catch (error) {
        console.error("Failed to copy public key:", error);
      }
    }
  };

  const handleInsertZuKycPods = async () => {
    try {
      await insertZuKycPods();
      console.log("ZuKYC pods inserted successfully");
    } catch (error) {
      console.error("Failed to insert ZuKYC pods:", error);
    }
  };

  useEffect(() => {
    initialize();
    loadPrivateKeyInfo();
  }, [initialize]);

  return (
    <Sidebar>
      <SidebarHeader></SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <Collapsible open={foldersExpanded} onOpenChange={setFoldersExpanded}>
            <CollapsibleTrigger asChild>
              <SidebarGroupLabel className="cursor-pointer hover:bg-accent hover:text-accent-foreground rounded px-2 py-1 flex items-center gap-2">
                {foldersExpanded ? (
                  <ChevronDownIcon size={16} />
                ) : (
                  <ChevronRightIcon size={16} />
                )}
                Folders
              </SidebarGroupLabel>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <SidebarGroupContent>
                <SidebarMenu>
                  <SidebarMenuItem>
                    <SidebarMenuButton
                      onClick={() => {
                        setCurrentView("pods");
                        setSelectedFolderFilter("all");
                        setSelectedFilter("all");
                      }}
                      isActive={
                        currentView === "pods" && selectedFolderFilter === "all"
                      }
                    >
                      <FileIcon />
                      All Folders
                    </SidebarMenuButton>
                    <SidebarMenuBadge>
                      {appState.pod_stats.total_pods}
                    </SidebarMenuBadge>
                  </SidebarMenuItem>

                  {foldersLoading ? (
                    <SidebarMenuItem>
                      <SidebarMenuButton disabled>
                        <FolderIcon />
                        Loading...
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ) : (
                    folders.map((folder) => {
                      const podCount = [
                        ...appState.pod_lists.signed_pods,
                        ...appState.pod_lists.main_pods
                      ].filter((p) => p.space === folder.id).length;

                      return (
                        <SidebarMenuItem key={folder.id}>
                          <SidebarMenuButton
                            onClick={() => {
                              setCurrentView("pods");
                              setSelectedFolderFilter(folder.id);
                              setSelectedFilter("all");
                            }}
                            isActive={
                              currentView === "pods" &&
                              selectedFolderFilter === folder.id
                            }
                          >
                            <FolderIcon />
                            {folder.id}
                          </SidebarMenuButton>
                          <SidebarMenuBadge>{podCount}</SidebarMenuBadge>
                        </SidebarMenuItem>
                      );
                    })
                  )}
                </SidebarMenu>
              </SidebarGroupContent>
            </CollapsibleContent>
          </Collapsible>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupLabel>Filters</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => {
                    setCurrentView("pods");
                    setSelectedFilter("pinned");
                  }}
                  isActive={
                    currentView === "pods" && selectedFilter === "pinned"
                  }
                >
                  <StarIcon />
                  Pinned
                </SidebarMenuButton>
                <SidebarMenuBadge>
                  {
                    [
                      ...appState.pod_lists.signed_pods,
                      ...appState.pod_lists.main_pods
                    ].filter((p) => p.pinned).length
                  }
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
        {/* Public Key Display */}
        {privateKeyInfo && (
          <div
            onClick={handleCopyPublicKey}
            className="px-2 py-1 text-xs text-muted-foreground hover:text-foreground cursor-pointer hover:bg-accent rounded transition-colors break-all"
            title={`Click to copy: ${privateKeyInfo.public_key}`}
          >
            🔑{" "}
            {
              /*privateKeyInfo.public_key.substring(0, 12)}...{privateKeyInfo.public_key.slice(-8)*/ privateKeyInfo.public_key
            }
          </div>
        )}
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
            <DropdownMenuItem onClick={handleInsertZuKycPods}>
              Insert ZuKYC PODs
            </DropdownMenuItem>
            {/* Private key is automatically created when needed */}
            {privateKeyInfo && (
              <DropdownMenuItem disabled>
                Key: {privateKeyInfo.alias || "Default"} (
                {privateKeyInfo.public_key.substring(0, 8)}...)
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

                  {sendMode === "message" && !privateKeyInfo && (
                    <div className="text-xs text-muted-foreground">
                      💡 Private key will be auto-created when sending
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
