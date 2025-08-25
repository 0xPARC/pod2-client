import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";
import { DocumentsView } from "@/components/documents/DocumentsView";
import { fetchDocuments } from "@/lib/documentApi";
import { documentsQueryKey } from "@/lib/query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";

function DocumentsPage() {
  const navigate = useNavigate();
  const handleNewDocument = () => navigate({ to: "/documents/publish" });

  return (
    <>
      <DocumentsTopBar title="Documents" onNewDocument={handleNewDocument} />
      <DocumentsView />
    </>
  );
}

export const Route = createFileRoute("/documents/")({
  staticData: { breadcrumb: () => "Documents" },
  loader: async ({ context: { queryClient } }) => {
    await queryClient.ensureQueryData({
      queryKey: documentsQueryKey,
      queryFn: fetchDocuments
    });
  },
  component: DocumentsPage
});
