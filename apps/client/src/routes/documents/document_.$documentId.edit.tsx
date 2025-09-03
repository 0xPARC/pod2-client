import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";
import { PublishPage } from "@/components/documents/PublishPage";
import { detectContentType } from "@/lib/contentUtils";
import { fetchDocument } from "@/lib/documentApi";
import { createFileRoute, useNavigate } from "@tanstack/react-router";

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
          contentType={loaderData.contentType}
          editingDocument={editingDocument}
        />
      </>
    );
  }
});
