import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";
import { DraftsView } from "@/components/documents/DraftsView";
import { createFileRoute, useNavigate } from "@tanstack/react-router";

function DraftsPage() {
  const navigate = useNavigate();

  const handleNewDocument = () => {
    navigate({ to: "/documents/publish" });
  };

  const handleEditDraft = (draftId: string) => {
    navigate({ to: "/documents/publish", search: { draftId } });
  };

  return (
    <>
      <DocumentsTopBar title="Drafts" onNewDocument={handleNewDocument} />
      <DraftsView onEditDraft={handleEditDraft} />
    </>
  );
}

export const Route = createFileRoute("/documents/drafts")({
  staticData: { breadcrumb: () => "Drafts" },
  component: DraftsPage
});
