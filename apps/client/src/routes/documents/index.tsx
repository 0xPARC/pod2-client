import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { DocumentsView } from "@/components/documents/DocumentsView";
import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";

function DocumentsPage() {
  const navigate = useNavigate();

  const handleNewDocument = () => {
    navigate({ to: "/documents/publish" });
  };

  return (
    <>
      <DocumentsTopBar title="Documents" onNewDocument={handleNewDocument} />
      <DocumentsView />
    </>
  );
}

export const Route = createFileRoute("/documents/")({
  staticData: { breadcrumb: () => "Documents" },
  component: DocumentsPage
});
