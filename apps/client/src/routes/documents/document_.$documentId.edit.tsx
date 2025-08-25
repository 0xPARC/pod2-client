import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { fetchDocument } from "@/lib/documentApi";
import { detectContentType } from "@/lib/contentUtils";
import { PublishPage } from "@/components/documents/PublishPage";
import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";

export const Route = createFileRoute("/documents/document_/$documentId/edit")({
  staticData: {
    breadcrumb: ({ loaderData }: any) =>
      `Edit: ${loaderData?.title ?? "Document"}`
  },
  loader: async ({ params }) => {
    const id = Number(params.documentId);
    if (!Number.isFinite(id)) throw new Error("Invalid document id");
    try {
      const document = await fetchDocument(id);
      const contentType = detectContentType(document);
      return {
        id,
        title: document.metadata.title,
        document,
        contentType
      };
    } catch (error) {
      throw new Error(`Failed to load document ${id} for editing: ${error}`);
    }
  },
  component: function DocumentEdit() {
    const loaderData = Route.useLoaderData();
    const navigate = useNavigate();

    const handleNewDocument = () => {
      navigate({ to: "/documents/publish" });
    };

    const handleEditSuccess = (newDocumentId: number) => {
      console.log("Document edited successfully with ID:", newDocumentId);
      // Navigate back to the document view (could be same ID or new revision)
      navigate({
        to: "/documents/document/$documentId",
        params: { documentId: newDocumentId.toString() }
      });
    };

    // Prepare editing document data from loader
    const editingDocument = {
      documentId: loaderData.document.metadata.id!,
      postId: loaderData.document.metadata.post_id,
      title: loaderData.document.metadata.title || "",
      content: loaderData.document.content,
      tags: loaderData.document.metadata.tags,
      authors: loaderData.document.metadata.authors,
      replyTo: loaderData.document.metadata.reply_to
        ? `${loaderData.document.metadata.reply_to.post_id}:${loaderData.document.metadata.reply_to.document_id}`
        : null
    };

    return (
      <>
        <DocumentsTopBar
          title={`Edit: ${loaderData.title}`}
          onNewDocument={handleNewDocument}
        />
        <PublishPage
          onPublishSuccess={handleEditSuccess}
          contentType={loaderData.contentType}
          editingDocument={editingDocument}
        />
      </>
    );
  }
});
