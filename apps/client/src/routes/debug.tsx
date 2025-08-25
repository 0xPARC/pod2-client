import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { DebugView } from "@/components/settings/DebugView";
import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";

function DebugPage() {
  const navigate = useNavigate();

  const handleNewDocument = () => {
    navigate({ to: "/documents/publish" });
  };

  return (
    <>
      <DocumentsTopBar title="Debug" onNewDocument={handleNewDocument} />
      <DebugView />
    </>
  );
}

export const Route = createFileRoute("/debug")({
  staticData: { breadcrumb: () => "Debug" },
  component: DebugPage
});
