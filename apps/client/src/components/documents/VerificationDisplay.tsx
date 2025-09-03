import { AlertCircleIcon } from "lucide-react";
import { Card, CardContent } from "../ui/card";

interface VerificationDisplayProps {
  verificationError: string | null;
}

export function VerificationDisplay({
  verificationError
}: VerificationDisplayProps) {
  if (!verificationError) return null;

  return (
    <Card className="mb-6 border-destructive">
      <CardContent className="pt-6">
        <div className="flex items-center gap-2 text-destructive">
          <AlertCircleIcon className="h-4 w-4" />
          <span className="font-medium">
            Verification failed: {verificationError}
          </span>
        </div>
      </CardContent>
    </Card>
  );
}
