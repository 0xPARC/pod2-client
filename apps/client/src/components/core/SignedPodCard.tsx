import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { SignedPod } from "@pod2/pod2js";
import { ClipboardCopy } from "lucide-react";
import React from "react";
import { toast } from "sonner";
import { Button } from "../ui/button";
import ValueRenderer from "./ValueRenderer";

interface SignedPodCardProps {
  signedPod: SignedPod;
}

const SignedPodCard: React.FC<SignedPodCardProps> = ({ signedPod }) => {
  const handleExport = async () => {
    try {
      const jsonString = JSON.stringify(signedPod, null, 2);
      await navigator.clipboard.writeText(jsonString);
      toast.success("POD payload copied to clipboard.");
    } catch (err) {
      console.error("Failed to copy to clipboard:", err);
      toast.error("Failed to copy POD payload to clipboard.");
    }
  };

  return (
    <Card className="w-full mx-auto">
      <CardHeader>
        <div className="flex justify-between items-center">
          <CardTitle className="flex items-center">
            <span
              className="font-mono text-sm cursor-pointer"
              onClick={() => navigator.clipboard.writeText(signedPod.id)}
            >
              ID: {signedPod.id.slice(0, 32)}&hellip;
            </span>
          </CardTitle>
          <Button variant="outline" size="sm" onClick={handleExport}>
            <ClipboardCopy className="w-4 h-4 mr-2" />
            Export
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <h4 className="font-semibold mb-2 text-sm text-muted-foreground">
          Entries:
        </h4>

        {Object.keys(signedPod.entries).length > 0 ? (
          <div className="border rounded-md">
            {Object.entries(signedPod.entries)
              .sort((a, b) => a[0].localeCompare(b[0]))
              .map(([key, value], index, arr) => (
                <div
                  key={key}
                  className={`flex p-2 text-sm ${index < arr.length - 1 ? "border-b" : ""}`}
                >
                  <div className="w-1/3 font-medium text-gray-700 dark:text-gray-300 break-all pr-2">
                    {key}
                  </div>
                  <div className="w-2/3 text-gray-900 dark:text-gray-100 break-all">
                    <ValueRenderer value={value} />
                  </div>
                </div>
              ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground italic">
            No entries in this POD.
          </p>
        )}
        {/* Display more SignedPod specific details here, e.g., proof */}
      </CardContent>
    </Card>
  );
};

export default SignedPodCard;
