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
import { getPrivateKeyInfo, PrivateKeyInfo } from "@/lib/rpc";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@radix-ui/react-collapsible";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  CodeIcon,
  EditIcon,
  FileCheck2Icon,
  FileIcon,
  FilePenLineIcon,
  FileTextIcon,
  FolderIcon,
  Github,
  InboxIcon,
  MessageSquareIcon,
  StarIcon,
  UploadIcon
} from "lucide-react";
import { useEffect, useState } from "react";
import { useAppStore } from "../lib/store";
import { FeatureGate } from "../lib/features/config";
import CreateSignedPodDialog from "./CreateSignedPodDialog";
import { Button } from "./ui/button";
import { ImportPodDialog } from "./ImportPodDialog";
import { openUrl } from "@tauri-apps/plugin-opener";

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
    setSelectedFolderFilter
  } = useAppStore();
  const [privateKeyInfo, setPrivateKeyInfo] = useState<PrivateKeyInfo | null>(
    null
  );
  const [isCreateSignedPodDialogOpen, setIsCreateSignedPodDialogOpen] =
    useState(false);
  const [foldersExpanded, setFoldersExpanded] = useState(true);

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

  useEffect(() => {
    initialize();
    loadPrivateKeyInfo();
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
          <SidebarGroupLabel>Tools</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => setCurrentView("documents")}
                  isActive={currentView === "documents"}
                >
                  <FileTextIcon />
                  Documents
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <FeatureGate feature="networking">
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
        </FeatureGate>

        <FeatureGate feature="authoring">
          <SidebarGroup>
            <SidebarGroupLabel>Authoring</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    onClick={() => setCurrentView("editor")}
                    isActive={currentView === "editor"}
                  >
                    <EditIcon />
                    Podlang Editor
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <ImportPodDialog
                    trigger={
                      <SidebarMenuButton>
                        <UploadIcon />
                        Import POD
                      </SidebarMenuButton>
                    }
                  />
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
        </FeatureGate>
        <FeatureGate feature="frogcrypto">
          <SidebarGroup>
            <SidebarGroupLabel>Extras</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton onClick={() => setCurrentView("frogs")}>
                    FrogCrypto
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </FeatureGate>

        <FeatureGate feature="networking">
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
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </FeatureGate>
      </SidebarContent>
      <SidebarFooter>
        {/* Public Key Display */}
        {privateKeyInfo && (
          <div
            onClick={handleCopyPublicKey}
            className="px-2 py-1 text-xs text-muted-foreground hover:text-foreground cursor-pointer hover:bg-accent rounded transition-colors break-all"
            title={`Click to copy: ${privateKeyInfo.public_key}`}
          >
            Your public key:
            <span className="text-xs text-accent-foreground">
              {
                /*privateKeyInfo.public_key.substring(0, 12)}...{privateKeyInfo.public_key.slice(-8)*/ privateKeyInfo.public_key
              }
            </span>
          </div>
        )}

        {/* GitHub Link */}
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start text-muted-foreground hover:text-foreground"
          onClick={() => {
            // Use Tauri's opener plugin to open external URL
            openUrl("https://github.com/0xPARC/pod2-client");
          }}
        >
          <Github className="mr-2 h-4 w-4" />
          View on GitHub
        </Button>
      </SidebarFooter>
    </Sidebar>
  );
}
