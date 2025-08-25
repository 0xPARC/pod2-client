import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { fetchDocument } from "@/lib/documentApi";
import { DocumentDetailView } from "@/components/documents/DocumentDetailView";
import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";

export const Route = createFileRoute("/documents/document/$documentId")({
  staticData: {
    breadcrumb: ({ loaderData }: any) => loaderData?.title ?? "Document"
  },
  loader: async ({ params }) => {
    const id = Number(params.documentId);
    if (!Number.isFinite(id)) throw new Error("Invalid document id");
    try {
      const doc = await fetchDocument(id);
      return { id, title: doc.metadata.title };
    } catch {
      return { id, title: `Document #${id}` };
    }
  },
  component: function DocumentDetail() {
    const { documentId } = Route.useParams();
    const loaderData = Route.useLoaderData();
    const navigate = useNavigate();

    const handleNewDocument = () => {
      navigate({ to: "/documents/publish" });
    };

    return (
      <>
        <DocumentsTopBar
          title={loaderData.title}
          prefix="Document:"
          onNewDocument={handleNewDocument}
        />
        <DocumentDetailView documentId={Number(documentId)} />
      </>
    );
  }
});
