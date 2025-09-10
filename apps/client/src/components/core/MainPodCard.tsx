import { prettyPrintCustomPredicates } from "@/lib/features/authoring/rpc";
import type {
  CustomPredicateRef,
  MainPod,
  Value,
  ValueRef
} from "@pod2/pod2js";
import { ClipboardCopy, Plus } from "lucide-react";
import React, { useEffect, useState } from "react";
import { toast } from "sonner";
import { importPod } from "../../lib/rpc";
import { useAppStore } from "../../lib/store";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import ValueRenderer from "./ValueRenderer";
import { ValueTable } from "./ValueTable";

interface MainPodCardProps {
  mainPod: MainPod;
  podId?: string;
  label?: string | null;
}

function ViewStatementArg({ arg }: { arg: ValueRef }) {
  if (arg.type === "Key") {
    return (
      <span>
        {arg.value.root}["{arg.value.key}"]
      </span>
    );
  }
  return <ValueRenderer value={arg.value} />;
}

function ViewCustomPredicate({
  predicate,
  args
}: {
  predicate: CustomPredicateRef;
  args: Value[];
}): React.ReactNode {
  const index = predicate.index;
  const batch = predicate.batch;
  const predicateName = batch.predicates[index].name;

  return (
    <div>
      <span className="font-medium mb-2 block">
        Predicate: <span className="font-mono">{predicateName}</span>
      </span>
      <span className="font-medium mb-2 block">Arguments:</span>
      <ValueTable values={args} />
    </div>
  );
}

const MainPodCard: React.FC<MainPodCardProps> = ({ mainPod, podId, label }) => {
  const statementCount = mainPod.publicStatements?.length || 0;
  const [prettyPrintedPredicates, setPrettyPrintedPredicates] =
    useState<string>("");

  useEffect(() => {
    const fetchPrettyPrintedPredicates = async () => {
      const prettyPrintedPredicates =
        await prettyPrintCustomPredicates(mainPod);
      console.log("prettyPrintedPredicates", prettyPrintedPredicates);
      setPrettyPrintedPredicates(prettyPrintedPredicates);
    };
    fetchPrettyPrintedPredicates();
  }, [mainPod]);

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

  return (
    <Card className="w-full mx-auto">
      <CardHeader>
        <div className="flex justify-between items-center">
          <CardTitle className="flex items-center">
            <span
              className="font-mono text-sm cursor-pointer"
              onClick={() => navigator.clipboard.writeText(mainPod.stsHash)}
            >
              ID: {mainPod.stsHash.slice(0, 32)}&hellip;
            </span>
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
      </CardHeader>
      <CardContent>
        <h4 className="font-semibold mb-2 text-sm text-muted-foreground">
          Statements:
        </h4>
        {statementCount > 0 ? (
          <div className="border rounded-md mb-4">
            {mainPod.publicStatements?.map((statement, index, arr) => (
              <div
                key={index}
                className={`flex p-2 text-sm ${index < arr.length - 1 ? "border-b" : ""}`}
              >
                <div className="w-1/4 font-medium text-gray-700 dark:text-gray-300 break-all pr-2">
                  {statement.predicate}
                </div>
                <div className="w-3/4 text-gray-900 dark:text-gray-100 break-all flex flex-col gap-0.5">
                  {statement.predicate !== "Custom" &&
                    statement.predicate !== "None" &&
                    statement.predicate !== "Intro" &&
                    statement.args.map((arg, index) => (
                      <div key={index} className="flex items-center">
                        <ViewStatementArg key={index} arg={arg} />
                      </div>
                    ))}
                  {statement.predicate === "Custom" && (
                    <div className="text-gray-900 dark:text-gray-100 break-all">
                      <ViewCustomPredicate
                        predicate={statement.args[0]}
                        args={statement.args[1]}
                      />
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground italic">
            No entries in this POD.
          </p>
        )}
        {prettyPrintedPredicates && (
          <>
            <h4 className="font-semibold mb-2 text-sm text-muted-foreground">
              Custom Predicates:
            </h4>
            <div className="border rounded-md">
              <pre className="text-sm text-muted-foreground p-4">
                {prettyPrintedPredicates}
              </pre>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
};

export default MainPodCard;
