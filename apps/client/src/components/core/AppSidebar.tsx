import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from "@/components/ui/dropdown-menu";
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
  BugIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  DatabaseIcon,
  EditIcon,
  FilePenLineIcon,
  FileTextIcon,
  FolderIcon,
  Folders,
  Github,
  ImportIcon,
  PencilLineIcon,
  SettingsIcon
} from "lucide-react";
import { useState } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import { toast } from "sonner";
import { useAppStore, usePodCollection } from "../../lib/store";
import CreateSignedPodDialog from "../CreateSignedPodDialog";
import { ImportPodDialog } from "../ImportPodDialog";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle
} from "../ui/alert-dialog";
import { Button } from "../ui/button";
import { ScrollArea } from "../ui/scroll-area";
import { PublicKeyAvatar } from "./PublicKeyAvatar";

export function AppSidebar() {
  const { appState, folders, foldersLoading, privateKeyInfo, buildInfo } =
    useAppStore();

  const { selectedFolderId, selectFolder } = usePodCollection();
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  // Get current route for active state
  const [isCreateSignedPodDialogOpen, setIsCreateSignedPodDialogOpen] =
    useState(false);
  const [allFoldersExpanded, setAllFoldersExpanded] = useState(true);
  const [showResetConfirmation, setShowResetConfirmation] = useState(false);
  const [isResetting, setIsResetting] = useState(false);

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

  const handleResetDatabase = async () => {
    setIsResetting(true);
    try {
      await invoke("reset_database");
      toast.success("Database reset successfully! Redirecting to setup...");
      setShowResetConfirmation(false);

      // Trigger a page reload to restart the app state and show identity setup
      setTimeout(() => {
        window.location.reload();
      }, 1000);
    } catch (error) {
      toast.error(`Failed to reset database: ${error}`);
    } finally {
      setIsResetting(false);
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
                    <SidebarMenuButton asChild isActive={pathname === "/pods"}>
                      <Link to="/pods" onClick={() => selectFolder("all")}>
                        {allFoldersExpanded ? (
                          <ChevronDownIcon size={16} />
                        ) : (
                          <ChevronRightIcon size={16} />
                        )}
                        <Folders />
                        <span>All</span>
                        <SidebarMenuBadge>
                          {appState.pod_stats.total_pods}
                        </SidebarMenuBadge>
                      </Link>
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
                                asChild
                                isActive={selectedFolderId === folder.id}
                              >
                                <Link
                                  to="/pods"
                                  search={{ space: folder.id }}
                                  onClick={() => selectFolder(folder.id)}
                                >
                                  <FolderIcon />
                                  <span>{folder.id}</span>
                                  <SidebarMenuBadge>
                                    {podCount}
                                  </SidebarMenuBadge>
                                </Link>
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
                  asChild
                  isActive={
                    pathname.startsWith("/documents") &&
                    !pathname.includes("/drafts")
                  }
                >
                  <Link to="/documents">
                    <FileTextIcon />
                    <span>Documents</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={pathname.startsWith("/documents/drafts")}
                >
                  <Link to="/documents/drafts">
                    <PencilLineIcon />
                    <span>Drafts</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupLabel>Authoring</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild isActive={pathname === "/editor"}>
                  <Link to="/editor">
                    <EditIcon />
                    <span>POD Editor</span>
                  </Link>
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
        <SidebarGroup>
          <SidebarGroupLabel>Extras</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={pathname === "/frogcrypto"}
                >
                  <Link to="/frogcrypto">FrogCrypto</Link>
                </SidebarMenuButton>
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

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="w-full justify-start text-muted-foreground hover:text-foreground"
            >
              <BugIcon className="h-4 w-4 mr-2" />
              Debug
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-48">
            <DropdownMenuItem
              onClick={() => {
                invoke("insert_zukyc_pods")
                  .then(() => {
                    toast.success("ZuKYC PODs added successfully");
                  })
                  .catch((error) => {
                    toast.error(`Failed to add ZuKYC PODs: ${error}`);
                  });
              }}
            >
              <ImportIcon className="h-4 w-4 mr-2" />
              Add ZuKYC PODs
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem asChild>
              <Link to="/debug">
                <SettingsIcon className="h-4 w-4 mr-2" />
                View Config
              </Link>
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => setShowResetConfirmation(true)}
              className="text-destructive focus:text-destructive"
            >
              <DatabaseIcon className="h-4 w-4 mr-2" />
              Reset Database
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        {/* GitHub Link with Commit SHA */}
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-center text-muted-foreground hover:text-foreground"
          onClick={() => {
            // If we have build info, link to the specific commit, otherwise to the repo
            const url = buildInfo
              ? `https://github.com/0xPARC/pod2-client/commit/${buildInfo}`
              : "https://github.com/0xPARC/pod2-client";
            openUrl(url);
          }}
          title={
            buildInfo
              ? `View commit ${buildInfo.slice(0, 7)} on GitHub`
              : "View on GitHub"
          }
        >
          <div className="flex items-center gap-2">
            <Github className="h-4 w-4" />
            {buildInfo && (
              <span className="font-mono text-[10px] opacity-60">
                {buildInfo.slice(0, 7)}
              </span>
            )}
          </div>
        </Button>
      </SidebarFooter>

      {/* Reset Database Confirmation Dialog */}
      <AlertDialog
        open={showResetConfirmation}
        onOpenChange={setShowResetConfirmation}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle className="text-destructive flex items-center gap-2">
              <DatabaseIcon className="h-5 w-5" />
              Reset Database - Permanent Action
            </AlertDialogTitle>
            <AlertDialogDescription className="text-left">
              <div className="space-y-3 mt-3">
                <p className="font-medium text-destructive">
                  This will permanently delete:
                </p>
                <ul className="list-disc list-inside text-sm space-y-1 ml-2">
                  <li>Your private key</li>
                  <li>All PODs (signed and main)</li>
                  <li>All folders</li>
                  <li>Your identity POD</li>
                  <li>All other application data</li>
                </ul>
                <p className="text-sm font-medium">
                  This will <span className="font-bold">NOT</span> delete any
                  documents posted to the PodNet server, or remove your identity
                  from the PodNet server or the FrogCrypto leaderboard. However,
                  you will be unable to re-claim your identity.
                </p>
                <p className="text-sm font-medium mt-4">
                  The application will restart and show the setup screen.
                </p>
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isResetting}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleResetDatabase}
              disabled={isResetting}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {isResetting ? (
                <>
                  <div className="animate-spin rounded-full h-4 w-4 border-b border-white mr-2"></div>
                  Resetting...
                </>
              ) : (
                <>
                  <DatabaseIcon className="h-4 w-4 mr-2" />
                  Reset Database
                </>
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Sidebar>
  );
}
