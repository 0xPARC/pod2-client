import { useState } from "react";
import { toast } from "sonner";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Separator } from "./ui/separator";
import { CheckCircle, Loader2, Server, User, Shield } from "lucide-react";
import { useAppStore } from "../lib/store";

interface IdentitySetupModalProps {
  open: boolean;
  onComplete: () => void;
}

enum SetupStep {
  SERVER_SETUP = "server_setup",
  USERNAME_REGISTRATION = "username_registration",
  SETUP_COMPLETE = "setup_complete"
}

export function IdentitySetupModal({ open, onComplete }: IdentitySetupModalProps) {
  const { loadPrivateKeyInfo } = useAppStore();
  const [currentStep, setCurrentStep] = useState<SetupStep>(SetupStep.SERVER_SETUP);
  const [serverUrl, setServerUrl] = useState("http://localhost:3001");
  const [username, setUsername] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [serverInfo, setServerInfo] = useState<{ server_id: string; public_key: any } | null>(null);

  const handleServerSetup = async () => {
    if (!serverUrl.trim()) {
      toast.error("Please enter a server URL");
      return;
    }

    setIsLoading(true);
    try {
      // Call the Tauri command to setup identity server
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke("setup_identity_server", { serverUrl });
      
      const serverResult = result as { server_id: string; public_key: any };
      setServerInfo(serverResult);
      setCurrentStep(SetupStep.USERNAME_REGISTRATION);
      toast.success(`Connected to identity server: ${serverResult.server_id}`);
    } catch (error) {
      console.error("Server setup error:", error);
      toast.error(`Failed to connect to identity server: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleUsernameRegistration = async () => {
    if (!username.trim()) {
      toast.error("Please enter a username");
      return;
    }

    setIsLoading(true);
    try {
      // Call the Tauri command to register username
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
      // Call the Tauri command to complete setup
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("complete_identity_setup");
      
      // Refresh the private key info in the store
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
              <span className="font-medium">Step 1: Connect to Identity Server</span>
            </div>
            
            <div className="space-y-4">
              <div>
                <Label htmlFor="server-url">Identity Server URL</Label>
                <Input
                  id="server-url"
                  value={serverUrl}
                  onChange={(e) => setServerUrl(e.target.value)}
                  placeholder="http://localhost:3001"
                  className="mt-2"
                />
                <p className="text-sm text-muted-foreground mt-1">
                  Enter the URL of your identity server. This server will verify your identity and issue cryptographic certificates.
                </p>
              </div>
              
              <Button 
                onClick={handleServerSetup} 
                disabled={isLoading}
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
                    Server ID: <code className="bg-muted px-1 py-0.5 rounded">{serverInfo.server_id}</code>
                  </p>
                </CardContent>
              </Card>
            )}
            
            <div className="space-y-4">
              <div>
                <Label htmlFor="username">Username</Label>
                <Input
                  id="username"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="Enter your username"
                  className="mt-2"
                />
                <p className="text-sm text-muted-foreground mt-1">
                  Choose a unique username that will be associated with your identity.
                </p>
              </div>
              
              <Button 
                onClick={handleUsernameRegistration} 
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
              <span className="font-medium">Step 3: Setup Complete</span>
            </div>
            
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <CheckCircle className="h-5 w-5 text-green-600" />
                  Identity Verified
                </CardTitle>
                <CardDescription>
                  Your identity has been successfully verified and registered.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div>
                  <p className="text-sm font-medium">Username:</p>
                  <code className="bg-muted px-2 py-1 rounded">{username}</code>
                </div>
                
                <div>
                  <p className="text-sm font-medium">Identity Server:</p>
                  <code className="bg-muted px-2 py-1 rounded">{serverInfo?.server_id}</code>
                </div>
                
                <Separator />
                
                <p className="text-sm text-muted-foreground">
                  Your identity POD has been stored securely and marked as mandatory. 
                  You can now use the application with your verified identity.
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
      <DialogContent className="sm:max-w-[500px]" onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>Identity Setup Required</DialogTitle>
          <DialogDescription>
            Complete the mandatory identity setup to use the application.
            This process will create your cryptographic identity and register it with an identity server.
          </DialogDescription>
        </DialogHeader>
        
        <div className="py-4">
          {renderStepContent()}
        </div>
      </DialogContent>
    </Dialog>
  );
}