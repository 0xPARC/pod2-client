import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";
import { PodViewer } from "@/components/pod-manager/PodViewer";

export const Route = createFileRoute("/pods/")({
  staticData: { breadcrumb: () => "My PODs" },
  validateSearch: z.object({ space: z.string().optional() }),
  component: PodViewer
});
