import type { MainPod, ValueRef } from "@pod2/pod2js";
import { ClipboardCopy, FileCheck2, Plus } from "lucide-react";
import React from "react";
import { toast } from "sonner";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from "./ui/card";
import ValueRenderer from "./ValueRenderer";
import { useAppStore } from "../lib/store";
import { importPod } from "../lib/rpc";

interface MainPodCardProps {
  mainPod: MainPod;
  podId?: string;
  label?: string | null;
  // onClick?: () => void; // For future "open in tab" functionality
}

// function isAnchoredKey(arg: AnchoredKey | Value): arg is AnchoredKey {
//   return typeof arg === 'object' && 'key' in arg;
// }

function ViewStatementArg({ arg }: { arg: ValueRef }) {
  if (arg.type === "Key") {
    return (
      <span>
        {arg.value.podId}["{arg.value.key}"]
      </span>
    );
  }
  return <ValueRenderer value={arg.value} />;
}

const MainPodCard: React.FC<MainPodCardProps> = ({ mainPod, podId, label }) => {
  const statementCount = mainPod.publicStatements?.length || 0;

  // Get all PODs to check if this one already exists
  const { appState } = useAppStore();
  const allPods = [
    ...appState.pod_lists.signed_pods,
    ...appState.pod_lists.main_pods
  ];

  // Check if a POD with this ID already exists
  const podExists = podId ? allPods.some((pod) => pod.id === podId) : false;

  const handleExport = async () => {
    try {
      const jsonString = JSON.stringify(mainPod, null, 2);
      await navigator.clipboard.writeText(jsonString);
      toast.success("POD payload copied to clipboard.");
    } catch (err) {
      console.error("Failed to copy to clipboard:", err);
      toast.error("Failed to copy POD payload to clipboard.");
    }
  };

  const handleAdd = async () => {
    try {
      await importPod(mainPod, label || undefined);
      toast.success("POD added to collection successfully.");
    } catch (err) {
      console.error("Failed to add POD:", err);
      toast.error("Failed to add POD to collection.");
    }
  };

  // Placeholder icon, can be replaced with an SVG or icon library component

  return (
    <Card className="w-full mx-auto">
      <CardHeader>
        <div className="flex justify-between items-center">
          <CardTitle className="flex items-center">
            <span className="mr-2">
              <FileCheck2 className="w-4 h-4 text-sky-500 dark:text-sky-400" />
            </span>{" "}
            Main POD Details
          </CardTitle>
          <div className="flex items-center gap-2">
            {!podExists && podId && (
              <Button variant="outline" size="sm" onClick={handleAdd}>
                <Plus className="w-4 h-4 mr-2" />
                Add
              </Button>
            )}
            <Button variant="outline" size="sm" onClick={handleExport}>
              <ClipboardCopy className="w-4 h-4 mr-2" />
              Export
            </Button>
          </div>
        </div>
        <CardDescription className="mt-2">
          {podId && (
            <div>
              <span className="font-semibold">ID:</span> {podId}
            </div>
          )}
          {label && (
            <div>
              <span className="font-semibold">Label:</span> {label}
            </div>
          )}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <h4 className="font-semibold mb-2 text-sm text-muted-foreground">
          Statements:
        </h4>
        {statementCount > 0 ? (
          <div className="border rounded-md">
            {mainPod.publicStatements?.map((statement, index, arr) => (
              <div
                key={index}
                className={`flex p-2 text-sm ${index < arr.length - 1 ? "border-b" : ""}`}
              >
                <div className="w-1/3 font-medium text-gray-700 dark:text-gray-300 break-all pr-2">
                  {statement.predicate}
                </div>
                <div className="w-2/3 text-gray-900 dark:text-gray-100 break-all">
                  {statement.predicate !== "Custom" &&
                    statement.predicate !== "None" &&
                    statement.args.map((arg, index) => (
                      <div key={index} className="flex items-center">
                        <ViewStatementArg key={index} arg={arg} />
                      </div>
                    ))}
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

export default MainPodCard;
