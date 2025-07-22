import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Badge } from "./ui/badge";
import { KeyRound, Github, Clock, CheckCircle, XCircle } from "lucide-react";

interface RsaIntroPodData {
  type: string;
  github_username: string;
  message: string;
  namespace: string;
  pod_id: string;
  created_at: string;
  verification_status: boolean;
  serialized_pod?: any;
}

interface RsaIntroPodCardProps {
  rsaIntroPod: RsaIntroPodData;
}

export default function RsaIntroPodCard({ rsaIntroPod }: RsaIntroPodCardProps) {
  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <KeyRound className="h-5 w-5 text-purple-600" />
            <CardTitle className="flex items-center gap-2">
              RSA Introduction POD
              {rsaIntroPod.verification_status ? (
                <Badge variant="outline" className="text-green-600 border-green-600">
                  <CheckCircle className="h-3 w-3 mr-1" />
                  Verified
                </Badge>
              ) : (
                <Badge variant="outline" className="text-red-600 border-red-600">
                  <XCircle className="h-3 w-3 mr-1" />
                  Unverified
                </Badge>
              )}
            </CardTitle>
          </div>
          <CardDescription>
            Cryptographic proof linking SSH public key to GitHub account
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="text-sm font-medium text-muted-foreground">
                GitHub Username
              </label>
              <div className="flex items-center gap-2 mt-1">
                <Github className="h-4 w-4 text-gray-600" />
                <code className="text-sm bg-muted px-2 py-1 rounded">
                  {rsaIntroPod.github_username}
                </code>
              </div>
            </div>
            
            <div>
              <label className="text-sm font-medium text-muted-foreground">
                Created
              </label>
              <div className="flex items-center gap-2 mt-1">
                <Clock className="h-4 w-4 text-gray-600" />
                <span className="text-sm">
                  {new Date(rsaIntroPod.created_at).toLocaleString()}
                </span>
              </div>
            </div>
          </div>

          <div>
            <label className="text-sm font-medium text-muted-foreground">
              POD ID
            </label>
            <code className="text-xs bg-muted px-2 py-1 rounded block mt-1 break-all font-mono">
              {rsaIntroPod.pod_id}
            </code>
          </div>

          <div>
            <label className="text-sm font-medium text-muted-foreground">
              Signed Message
            </label>
            <code className="text-sm bg-muted px-3 py-2 rounded block mt-1 font-mono">
              {rsaIntroPod.message}
            </code>
          </div>

          <div>
            <label className="text-sm font-medium text-muted-foreground">
              Namespace
            </label>
            <code className="text-sm bg-muted px-2 py-1 rounded inline-block mt-1">
              {rsaIntroPod.namespace}
            </code>
          </div>

          <div className="pt-2 border-t border-border">
            <p className="text-xs text-muted-foreground">
              This RSA introduction POD provides cryptographic proof that you control 
              the SSH private key corresponding to the public key associated with your 
              GitHub account <strong>{rsaIntroPod.github_username}</strong>.
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}