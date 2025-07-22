import { invoke } from "@tauri-apps/api/core";
import { CopyIcon, GithubIcon, SaveIcon, TerminalIcon, UserIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useAppStore } from "../../lib/store";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from "../ui/dialog";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { Textarea } from "../ui/textarea";

interface IdentitySettingsProps {
  currentUsername?: string;
  onUsernameUpdate?: (username: string) => void;
}

export function IdentitySettings({
  currentUsername = "unknown_user", // Default fallback
  onUsernameUpdate
}: IdentitySettingsProps) {
  const { appState } = useAppStore();
  const [githubUsername, setGithubUsername] = useState<string>("");
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [savedGithubUsername, setSavedGithubUsername] = useState<string>("");
  const [showSignatureDialog, setShowSignatureDialog] = useState(false);
  const [signature, setSignature] = useState<string>("");

  // Extract existing GitHub username from RSA intro PODs
  const getExistingGitHubUsername = () => {
    const identityRsaPods = appState.pod_lists.rsa_intro_pods.filter(
      (pod) => pod.space === "identity"
    );

    if (identityRsaPods.length > 0) {
      try {
        // Get the most recent RSA intro POD (in case there are multiple)
        const latestPod = identityRsaPods.sort((a, b) => 
          new Date(b.created_at || 0).getTime() - new Date(a.created_at || 0).getTime()
        )[0];
        
        // Extract github_username from the POD data
        if (latestPod.data?.pod_data_payload?.github_username) {
          return latestPod.data.pod_data_payload.github_username;
        }
      } catch (error) {
        console.error("Failed to extract GitHub username from RSA intro POD:", error);
      }
    }
    
    return null;
  };

  // Load existing GitHub username on component mount and when app state changes
  useEffect(() => {
    const existingUsername = getExistingGitHubUsername();
    if (existingUsername && !savedGithubUsername) {
      setSavedGithubUsername(existingUsername);
    }
  }, [appState.pod_lists.rsa_intro_pods, savedGithubUsername]);

  const handleSave = () => {
    if (!githubUsername.trim()) {
      toast.error("GitHub username cannot be empty");
      return;
    }

    // Basic validation for GitHub username format
    const githubUsernameRegex = /^[a-z\d](?:[a-z\d]|-(?=[a-z\d])){0,38}$/i;
    if (!githubUsernameRegex.test(githubUsername.trim())) {
      toast.error("Invalid GitHub username format");
      return;
    }

    // Show signature dialog instead of directly saving
    setShowSignatureDialog(true);
  };

  const handleSignatureSubmit = async () => {
    if (!signature.trim()) {
      toast.error("Signature cannot be empty");
      return;
    }

    setIsSaving(true);
    
    try {
      console.log("Validating SSH signature for GitHub username:", githubUsername.trim());
      
      // Call the Tauri command for SSH signature validation
      const result = await invoke<string>("validate_ssh_signature", {
        githubUsername: githubUsername.trim(),
        sshSignature: signature.trim(),
      });
      
      console.log("Validation result:", result);
      
      // Success - save the GitHub username
      setSavedGithubUsername(githubUsername.trim());
      setIsEditing(false);
      setShowSignatureDialog(false);
      setSignature("");
      toast.success("GitHub username linked successfully!");
    } catch (error) {
      console.error("Failed to validate SSH signature:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      toast.error(`Failed to link GitHub account: ${errorMessage}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleSignatureCancel = () => {
    setShowSignatureDialog(false);
    setSignature("");
  };

  const copyCommand = async () => {
    const command = 'echo "E PLURIBUS UNUM; DO NOT SHARE" | ssh-keygen -Y sign -n "podnet" -f ~/.ssh/id_rsa';
    try {
      await navigator.clipboard.writeText(command);
      toast.success("Command copied to clipboard!");
    } catch (error) {
      console.error("Failed to copy command:", error);
      toast.error("Failed to copy command");
    }
  };

  const handleEdit = () => {
    setGithubUsername(savedGithubUsername);
    setIsEditing(true);
  };

  const handleCancel = () => {
    setGithubUsername("");
    setIsEditing(false);
  };

  const handleUnlink = async () => {
    try {
      const identityRsaPods = appState.pod_lists.rsa_intro_pods.filter(
        (pod) => pod.space === "identity"
      );

      if (identityRsaPods.length > 0) {
        // Delete all RSA intro PODs from the identity space
        for (const pod of identityRsaPods) {
          await invoke("delete_pod", {
            podId: pod.id,
            spaceId: "identity"
          });
        }
        toast.success("GitHub account unlinked and RSA intro POD deleted");
      } else {
        toast.success("GitHub account unlinked");
      }
      
      setSavedGithubUsername("");
    } catch (error) {
      console.error("Failed to unlink GitHub account:", error);
      toast.error("Failed to unlink GitHub account");
    }
  };

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold mb-2">Identity Settings</h1>
        <p className="text-muted-foreground">
          Manage your identity and external account connections
        </p>
      </div>

      {/* Current Identity */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <UserIcon className="h-5 w-5" />
            Current Identity
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            <div>
              <Label className="text-sm font-medium text-muted-foreground">
                POD Username
              </Label>
              <div className="flex items-center gap-2 mt-1">
                <Badge variant="outline" className="font-mono">
                  {currentUsername}
                </Badge>
                <Badge variant="secondary" className="text-xs">
                  Verified
                </Badge>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* GitHub Integration */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <GithubIcon className="h-5 w-5" />
            GitHub Integration
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">
              Link your GitHub account to display your GitHub username alongside your POD identity.
            </p>
            
            {!isEditing && !savedGithubUsername ? (
              <div className="space-y-3">
                <div className="text-sm text-muted-foreground">
                  No GitHub account linked
                </div>
                <Button onClick={() => setIsEditing(true)} size="sm">
                  <GithubIcon className="h-4 w-4 mr-2" />
                  Link GitHub Account
                </Button>
              </div>
            ) : !isEditing && savedGithubUsername ? (
              <div className="space-y-3">
                <div>
                  <Label className="text-sm font-medium text-muted-foreground">
                    Linked GitHub Account
                  </Label>
                  <div className="flex items-center gap-2 mt-1">
                    <Badge variant="outline" className="font-mono">
                      <GithubIcon className="h-3 w-3 mr-1" />
                      {savedGithubUsername}
                    </Badge>
                    <Badge variant="secondary" className="text-xs bg-green-50 text-green-700 border-green-200">
                      Verified with RSA POD
                    </Badge>
                  </div>
                  <p className="text-xs text-muted-foreground mt-2">
                    Account linked via cryptographic RSA intro POD. This provides verifiable proof 
                    that you control the SSH private key associated with your GitHub account.
                  </p>
                </div>
                <div className="flex gap-2">
                  <Button onClick={handleEdit} variant="outline" size="sm">
                    Edit
                  </Button>
                  <Button 
                    onClick={handleUnlink} 
                    variant="outline" 
                    size="sm"
                    className="text-destructive hover:text-destructive"
                  >
                    Unlink
                  </Button>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <div>
                  <Label htmlFor="github-username" className="text-sm font-medium">
                    GitHub Username
                  </Label>
                  <Input
                    id="github-username"
                    type="text"
                    placeholder="octocat"
                    value={githubUsername}
                    onChange={(e) => setGithubUsername(e.target.value)}
                    className="mt-1"
                    autoFocus
                  />
                  <p className="text-xs text-muted-foreground mt-1">
                    Enter your GitHub username (without the @ symbol)
                  </p>
                </div>
                <div className="flex gap-2">
                  <Button
                    onClick={handleSave}
                    disabled={!githubUsername.trim() || isSaving}
                    size="sm"
                  >
                    {isSaving ? (
                      <>
                        <div className="animate-spin rounded-full h-3 w-3 border-b border-current mr-2"></div>
                        Saving...
                      </>
                    ) : (
                      <>
                        <SaveIcon className="h-4 w-4 mr-2" />
                        Save
                      </>
                    )}
                  </Button>
                  <Button onClick={handleCancel} variant="outline" size="sm">
                    Cancel
                  </Button>
                </div>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Signature Dialog */}
      <Dialog open={showSignatureDialog} onOpenChange={setShowSignatureDialog}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <TerminalIcon className="h-5 w-5" />
              Generate SSH Signature
            </DialogTitle>
            <DialogDescription>
              To verify your GitHub account ownership, please generate a signature using your SSH key.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div>
              <Label className="text-sm font-medium">Step 1: Run this command</Label>
              <div className="mt-2 p-3 bg-muted rounded-lg font-mono text-sm break-all">
                <div className="flex items-center justify-between gap-2">
                  <code className="flex-1">
                    echo "E PLURIBUS UNUM; DO NOT SHARE" | ssh-keygen -Y sign -n "podnet" -f ~/.ssh/id_rsa
                  </code>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={copyCommand}
                    className="h-8 w-8 p-0 flex-shrink-0"
                  >
                    <CopyIcon className="h-4 w-4" />
                  </Button>
                </div>
              </div>
              <p className="text-xs text-muted-foreground mt-2">
                This command will generate a signature using your default SSH key (~/.ssh/id_rsa). 
                If you use a different key file, replace the path accordingly.
              </p>
            </div>

            <div>
              <Label htmlFor="signature" className="text-sm font-medium">
                Step 2: Paste the signature output
              </Label>
              <Textarea
                id="signature"
                placeholder="-----BEGIN SSH SIGNATURE-----
U1NIU0lH...
-----END SSH SIGNATURE-----"
                value={signature}
                onChange={(e) => setSignature(e.target.value)}
                className="mt-2 font-mono text-xs"
                rows={8}
              />
              <p className="text-xs text-muted-foreground mt-2">
                Copy the entire signature output (including the BEGIN and END lines) and paste it here.
              </p>
            </div>
          </div>

          <DialogFooter className="flex gap-2">
            <Button
              variant="outline"
              onClick={handleSignatureCancel}
              disabled={isSaving}
            >
              Cancel
            </Button>
            <Button
              onClick={handleSignatureSubmit}
              disabled={!signature.trim() || isSaving}
            >
              {isSaving ? (
                <>
                  <div className="animate-spin rounded-full h-3 w-3 border-b border-current mr-2"></div>
                  Linking...
                </>
              ) : (
                <>
                  <GithubIcon className="h-4 w-4 mr-2" />
                  Link Account
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

    </div>
  );
}