import {
  CheckCircle,
  ExternalLink,
  Github,
  Loader2,
  Server,
  Shield,
  User
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useConfigSection } from "../lib/config/hooks";
import {
  GitHubOAuthClient,
  setupGitHubIdentityServer,
  extractGitHubInfoFromIdentityPod
} from "../lib/github-oauth";
import { useAppStore } from "../lib/store";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from "./ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle
} from "./ui/dialog";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Separator } from "./ui/separator";
import { Badge } from "./ui/badge";

interface GitHubIdentitySetupModalProps {
  open: boolean;
  onComplete: () => void;
}

enum SetupStep {
  SERVER_SETUP = "server_setup",
  GITHUB_AUTH = "github_auth",
  USERNAME_REGISTRATION = "username_registration",
  OAUTH_CALLBACK = "oauth_callback",
  SETUP_COMPLETE = "setup_complete"
}

interface GitHubServerInfo {
  server_id: string;
  public_key: any;
  is_github_server: boolean;
}

export function GitHubIdentitySetupModal({
  open,
  onComplete
}: GitHubIdentitySetupModalProps) {
  const { loadPrivateKeyInfo } = useAppStore();
  const networkConfig = useConfigSection("network");
  const [currentStep, setCurrentStep] = useState<SetupStep>(
    SetupStep.SERVER_SETUP
  );
  const [serverUrl, setServerUrl] = useState("");
  const [username, setUsername] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [serverInfo, setServerInfo] = useState<GitHubServerInfo | null>(null);
  const [oauthClient, setOauthClient] = useState<GitHubOAuthClient | null>(
    null
  );
  const [authUrl, setAuthUrl] = useState("");
  const [oauthState, setOauthState] = useState("");
  const [authCode, setAuthCode] = useState("");
  const [identityInfo, setIdentityInfo] = useState<any>(null);

  // Update serverUrl when networkConfig loads
  useEffect(() => {
    if (networkConfig) {
      setServerUrl(networkConfig.identity_server);
    }
  }, [networkConfig]);

  const handleServerSetup = async () => {
    if (!serverUrl.trim()) {
      toast.error("Please enter a server URL");
      return;
    }

    setIsLoading(true);
    try {
      const { server_info, is_github_server } =
        await setupGitHubIdentityServer(serverUrl);

      const enhancedServerInfo = {
        ...server_info,
        is_github_server
      };

      setServerInfo(enhancedServerInfo);

      if (is_github_server) {
        setOauthClient(new GitHubOAuthClient({ server_url: serverUrl }));
        setCurrentStep(SetupStep.GITHUB_AUTH);
        toast.success(
          `Connected to GitHub OAuth identity server: ${server_info.server_id}`
        );
      } else {
        // Fall back to regular identity setup for non-GitHub servers
        setCurrentStep(SetupStep.USERNAME_REGISTRATION);
        toast.success(`Connected to identity server: ${server_info.server_id}`);
      }

      // Store server info in Tauri backend
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("setup_identity_server", { serverUrl });
    } catch (error) {
      console.error("Server setup error:", error);
      toast.error(`Failed to connect to identity server: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleGitHubAuth = async () => {
    if (!username.trim()) {
      toast.error("Please enter your full name");
      return;
    }

    setIsLoading(true);
    try {
      // Request GitHub OAuth authorization URL via Tauri
      // The backend will handle private key creation/retrieval
      const { invoke } = await import("@tauri-apps/api/core");
      const authResponse = (await invoke("get_github_auth_url", {
        serverUrl,
        username
      })) as { auth_url: string; state: string };

      setAuthUrl(authResponse.auth_url);
      setOauthState(authResponse.state);

      // Open GitHub OAuth URL in browser
      if (oauthClient) {
        await oauthClient.openAuthUrl(authResponse.auth_url);
      }

      setCurrentStep(SetupStep.OAUTH_CALLBACK);
      toast.success(
        "GitHub authorization opened in browser. Complete the OAuth flow and return here."
      );
    } catch (error) {
      console.error("GitHub auth error:", error);
      toast.error(`Failed to initiate GitHub OAuth: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleOAuthCallback = async () => {
    if (!authCode.trim()) {
      toast.error("Please enter the authorization code from GitHub");
      return;
    }

    setIsLoading(true);
    try {
      // Complete GitHub OAuth identity verification via Tauri
      const { invoke } = await import("@tauri-apps/api/core");
      const identityResult = (await invoke(
        "complete_github_identity_verification",
        {
          serverUrl,
          code: authCode,
          state: oauthState,
          username
        }
      )) as {
        identity_pod: any;
        username: string;
        github_username?: string;
        server_id: string;
      };

      // Extract GitHub info for display
      const githubInfo = extractGitHubInfoFromIdentityPod(
        identityResult.identity_pod
      );
      setIdentityInfo(githubInfo);

      setCurrentStep(SetupStep.SETUP_COMPLETE);
      toast.success("GitHub identity verification completed successfully!");
    } catch (error) {
      console.error("OAuth callback error:", error);
      toast.error(`Failed to complete GitHub OAuth: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleRegularUsernameRegistration = async () => {
    if (!username.trim()) {
      toast.error("Please enter a username");
      return;
    }

    setIsLoading(true);
    try {
      // Use regular username registration for non-GitHub servers
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("register_username", { username, serverUrl });

      setCurrentStep(SetupStep.SETUP_COMPLETE);
      toast.success(`Successfully registered username: ${username}`);
    } catch (error) {
      console.error("Username registration error:", error);
      toast.error(`Failed to register username: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCompleteSetup = async () => {
    setIsLoading(true);
    try {
      // Complete setup and refresh app state
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("complete_identity_setup");
      await loadPrivateKeyInfo();

      toast.success("Identity setup completed successfully!");
      onComplete();
    } catch (error) {
      console.error("Setup completion error:", error);
      toast.error(`Failed to complete setup: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const renderStepContent = () => {
    switch (currentStep) {
      case SetupStep.SERVER_SETUP:
        return (
          <div className="space-y-6">
            <div className="flex items-center gap-2 text-blue-600">
              <Server className="h-5 w-5" />
              <span className="font-medium">
                Step 1: Connect to Identity Server
              </span>
            </div>

            <div className="space-y-4">
              <div>
                <Label htmlFor="server-url">Identity Server URL</Label>
                <Input
                  id="server-url"
                  value={serverUrl}
                  onChange={(e) => setServerUrl(e.target.value)}
                  placeholder={
                    networkConfig ? networkConfig.identity_server : "Loading..."
                  }
                  disabled={!networkConfig}
                  className="mt-2"
                />
                <p className="text-sm text-muted-foreground mt-1">
                  Enter the URL of your identity server. GitHub OAuth servers
                  provide enhanced verification.
                </p>
              </div>

              <Button
                onClick={handleServerSetup}
                disabled={isLoading || !networkConfig}
                className="w-full"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Connecting...
                  </>
                ) : (
                  "Connect to Server"
                )}
              </Button>
            </div>
          </div>
        );

      case SetupStep.GITHUB_AUTH:
        return (
          <div className="space-y-6">
            <div className="flex items-center gap-2 text-blue-600">
              <Github className="h-5 w-5" />
              <span className="font-medium">Step 2: GitHub Authentication</span>
            </div>

            {serverInfo && (
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <CheckCircle className="h-5 w-5 text-green-600" />
                    GitHub OAuth Server Detected
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-2">
                  <div className="flex items-center gap-2">
                    <Badge variant="outline" className="gap-1">
                      <Github className="h-3 w-3" />
                      GitHub OAuth
                    </Badge>
                    <code className="bg-muted px-1 py-0.5 rounded text-sm">
                      {serverInfo.server_id}
                    </code>
                  </div>
                </CardContent>
              </Card>
            )}

            <div className="space-y-4">
              <div>
                <Label htmlFor="fullname">Full Name</Label>
                <Input
                  id="fullname"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="Enter your full name"
                  className="mt-2"
                />
                <p className="text-sm text-muted-foreground mt-1">
                  This will be your display name in the identity POD. Your
                  GitHub username will be verified separately.
                </p>
              </div>

              <Button
                onClick={handleGitHubAuth}
                disabled={isLoading}
                className="w-full gap-2"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Preparing OAuth...
                  </>
                ) : (
                  <>
                    <Github className="h-4 w-4" />
                    Authenticate with GitHub
                  </>
                )}
              </Button>
            </div>
          </div>
        );

      case SetupStep.OAUTH_CALLBACK:
        return (
          <div className="space-y-6">
            <div className="flex items-center gap-2 text-blue-600">
              <ExternalLink className="h-5 w-5" />
              <span className="font-medium">Step 3: Complete GitHub OAuth</span>
            </div>

            <Card className="w-full">
              <CardHeader>
                <CardTitle>GitHub Authorization Required</CardTitle>
                <CardDescription>
                  Complete the OAuth flow in your browser, then return here with
                  the authorization code.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4 w-full">
                <div className="flex items-start gap-2 p-3 bg-muted rounded">
                  <Github className="h-4 w-4 mt-0.5 flex-shrink-0" />
                  <span className="text-sm font-mono break-all flex-1 min-w-0">
                    {authUrl}
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="flex-shrink-0"
                    onClick={() => oauthClient?.openAuthUrl(authUrl)}
                  >
                    <ExternalLink className="h-3 w-3" />
                  </Button>
                </div>
              </CardContent>
            </Card>

            <div className="space-y-4 w-full">
              <div className="w-full">
                <Label htmlFor="auth-code">Authorization Code</Label>
                <Input
                  id="auth-code"
                  value={authCode}
                  onChange={(e) => setAuthCode(e.target.value)}
                  placeholder="Paste the authorization code from GitHub"
                  className="mt-2 w-full"
                />
                <p className="text-sm text-muted-foreground mt-1">
                  After authorizing the app on GitHub, you'll receive an
                  authorization code. Paste it here.
                </p>
              </div>

              <Button
                onClick={handleOAuthCallback}
                disabled={isLoading}
                className="w-full"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Verifying...
                  </>
                ) : (
                  "Complete GitHub Verification"
                )}
              </Button>
            </div>
          </div>
        );

      case SetupStep.USERNAME_REGISTRATION:
        return (
          <div className="space-y-6">
            <div className="flex items-center gap-2 text-blue-600">
              <User className="h-5 w-5" />
              <span className="font-medium">Step 2: Register Username</span>
            </div>

            {serverInfo && (
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <CheckCircle className="h-5 w-5 text-green-600" />
                    Connected to Server
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-sm text-muted-foreground">
                    Server ID:{" "}
                    <code className="bg-muted px-1 py-0.5 rounded">
                      {serverInfo.server_id}
                    </code>
                  </p>
                </CardContent>
              </Card>
            )}

            <div className="space-y-4">
              <div>
                <Label htmlFor="username">Username</Label>
                <Input
                  autoComplete="off"
                  id="username"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="Enter your username"
                  className="mt-2"
                />
                <p className="text-sm text-muted-foreground mt-1">
                  Choose a unique username that will be associated with your
                  identity.
                </p>
              </div>

              <Button
                onClick={handleRegularUsernameRegistration}
                disabled={isLoading}
                className="w-full"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Registering...
                  </>
                ) : (
                  "Register Username"
                )}
              </Button>
            </div>
          </div>
        );

      case SetupStep.SETUP_COMPLETE:
        return (
          <div className="space-y-6">
            <div className="flex items-center gap-2 text-green-600">
              <Shield className="h-5 w-5" />
              <span className="font-medium">Setup Complete</span>
            </div>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <CheckCircle className="h-5 w-5 text-green-600" />
                  Identity Verified
                </CardTitle>
                <CardDescription>
                  Your identity has been successfully{" "}
                  {serverInfo?.is_github_server
                    ? "verified with GitHub and "
                    : ""}
                  registered with the identity server.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div>
                  <p className="text-sm font-medium">Full Name:</p>
                  <code className="bg-muted px-2 py-1 rounded">{username}</code>
                </div>

                {identityInfo?.github_username && (
                  <div>
                    <p className="text-sm font-medium">GitHub Username:</p>
                    <div className="flex items-center gap-2">
                      <Github className="h-4 w-4" />
                      <code className="bg-muted px-2 py-1 rounded">
                        {identityInfo.github_username}
                      </code>
                      {identityInfo.github_public_keys && (
                        <Badge variant="outline" className="text-xs">
                          {identityInfo.github_public_keys.length} SSH keys
                        </Badge>
                      )}
                    </div>
                  </div>
                )}

                <div>
                  <p className="text-sm font-medium">Identity Server:</p>
                  <code className="bg-muted px-2 py-1 rounded">
                    {serverInfo?.server_id}
                  </code>
                </div>

                <Separator />

                <p className="text-sm text-muted-foreground">
                  Your identity POD has been stored securely. You can now use
                  the application with your
                  {serverInfo?.is_github_server
                    ? " GitHub-verified"
                    : " registered"}{" "}
                  identity.
                </p>
              </CardContent>
            </Card>

            <Button
              onClick={handleCompleteSetup}
              disabled={isLoading}
              className="w-full"
            >
              {isLoading ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Completing Setup...
                </>
              ) : (
                "Complete Setup"
              )}
            </Button>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <Dialog open={open} onOpenChange={() => {}}>
      <DialogContent
        className="sm:max-w-[500px] max-w-[90vw] max-h-[90vh] overflow-y-auto"
        onInteractOutside={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>
            {serverInfo?.is_github_server
              ? "GitHub Identity Setup"
              : "Identity Setup Required"}
          </DialogTitle>
          <DialogDescription>
            Complete the identity setup to use the application.
            {serverInfo?.is_github_server
              ? " This server uses GitHub OAuth for enhanced verification."
              : " This process will create your identity POD and register it with an identity server."}
          </DialogDescription>
        </DialogHeader>

        <div className="py-4 max-w-full overflow-hidden">
          {renderStepContent()}
        </div>
      </DialogContent>
    </Dialog>
  );
}
