import { createFileRoute } from "@tanstack/react-router";
import { EditorView } from "@/components/editor/EditorView";

export const Route = createFileRoute("/editor")({
  staticData: { breadcrumb: () => "POD Editor" },
  component: EditorView
});
