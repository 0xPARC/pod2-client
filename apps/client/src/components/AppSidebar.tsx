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
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem
} from "@/components/ui/sidebar";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@radix-ui/react-collapsible";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  CodeIcon,
  EditIcon,
  FilePenLineIcon,
  FileTextIcon,
  FolderIcon,
  Folders,
  Github,
  ImportIcon,
  InboxIcon,
  MessageSquareIcon,
  UploadIcon
} from "lucide-react";
import { useState } from "react";
import { FeatureGate } from "../lib/features/config";
import { useAppStore } from "../lib/store";
import CreateSignedPodDialog from "./CreateSignedPodDialog";
import { ImportPodDialog } from "./ImportPodDialog";
import { PublicKeyAvatar } from "./PublicKeyAvatar";
import { Button } from "./ui/button";
import { ScrollArea } from "./ui/scroll-area";

export function AppSidebar() {
  const {
    appState,
    currentView,
    selectedFolderFilter,
    folders,
    foldersLoading,
    privateKeyInfo,
    setCurrentView,
    setSelectedFolderFilter
  } = useAppStore();
  const [isCreateSignedPodDialogOpen, setIsCreateSignedPodDialogOpen] =
    useState(false);
  const [allFoldersExpanded, setAllFoldersExpanded] = useState(true);

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

  return (
    <Sidebar>
      <SidebarHeader></SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>My PODs</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <Collapsible
                open={allFoldersExpanded}
                onOpenChange={setAllFoldersExpanded}
              >
                <SidebarMenuItem>
                  <CollapsibleTrigger asChild>
                    <SidebarMenuButton
                      onClick={() => {
                        setCurrentView("pods");
                        setSelectedFolderFilter("all");
                      }}
                      isActive={
                        currentView === "pods" && selectedFolderFilter === "all"
                      }
                    >
                      {allFoldersExpanded ? (
                        <ChevronDownIcon size={16} />
                      ) : (
                        <ChevronRightIcon size={16} />
                      )}
                      <Folders />
                      All
                      <SidebarMenuBadge>
                        {appState.pod_stats.total_pods}
                      </SidebarMenuBadge>
                    </SidebarMenuButton>
                  </CollapsibleTrigger>
                </SidebarMenuItem>
                <CollapsibleContent>
                  <ScrollArea className="max-h-64 overflow-hidden">
                    <SidebarMenuSub className="mr-0 pr-0">
                      {foldersLoading ? (
                        <SidebarMenuSubItem>
                          <SidebarMenuSubButton>
                            <FolderIcon />
                            Loading...
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                      ) : (
                        folders.map((folder) => {
                          const podCount = [
                            ...appState.pod_lists.signed_pods,
                            ...appState.pod_lists.main_pods
                          ].filter((p) => p.space === folder.id).length;

                          return (
                            <SidebarMenuSubItem key={folder.id}>
                              <SidebarMenuSubButton
                                onClick={() => {
                                  setCurrentView("pods");
                                  setSelectedFolderFilter(folder.id);
                                }}
                                isActive={
                                  currentView === "pods" &&
                                  selectedFolderFilter === folder.id
                                }
                              >
                                <FolderIcon />
                                {folder.id}
                                <SidebarMenuBadge>{podCount}</SidebarMenuBadge>
                              </SidebarMenuSubButton>
                            </SidebarMenuSubItem>
                          );
                        })
                      )}
                    </SidebarMenuSub>
                  </ScrollArea>
                </CollapsibleContent>
              </Collapsible>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupLabel>Podnet</SidebarGroupLabel>
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
              <SidebarMenuItem>
                <SidebarMenuButton
                  onClick={() => setCurrentView("publish")}
                  isActive={currentView === "publish"}
                >
                  <UploadIcon />
                  Publish
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
                    POD Request
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <ImportPodDialog
                    trigger={
                      <SidebarMenuButton>
                        <ImportIcon />
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
            className="px-2 py-1 text-xs text-muted-foreground hover:text-foreground cursor-pointer hover:bg-accent rounded transition-colors"
            title={`Click to copy: ${privateKeyInfo.public_key}`}
          >
            <div className="flex items-center gap-2">
              <PublicKeyAvatar
                publicKey={privateKeyInfo.public_key}
                size={32}
              />
              <div className="flex-1 min-w-0">
                <div className="text-xs">Your public key:</div>
                <div className="text-xs text-accent-foreground truncate">
                  {privateKeyInfo.public_key}
                </div>
              </div>
            </div>
          </div>
        )}

        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start text-muted-foreground hover:text-foreground"
          onClick={() => {
            invoke("insert_zukyc_pods").then(() => {
              console.log("Zukyc PODs inserted");
            });
          }}
        >
          Zukyc PODs
        </Button>

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
