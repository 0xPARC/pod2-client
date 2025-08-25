import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";
import { PublishPage } from "@/components/documents/PublishPage";
import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";

const searchSchema = z.object({
  draftId: z.string().uuid().optional(),
  contentType: z.enum(["document", "link", "file"]).optional(),
  replyTo: z.string().optional(),
  title: z.string().optional()
});

function getPublishTitle(search: z.infer<typeof searchSchema>) {
  if (search?.draftId) return "Edit Draft";
  if (search?.contentType === "link") return "New Link";
  if (search?.contentType === "file") return "New File";
  return "New Document";
}

export const Route = createFileRoute("/documents/publish")({
  staticData: {
    breadcrumb: ({ search }: any) => {
      return getPublishTitle(search as z.infer<typeof searchSchema>);
    }
  },
  validateSearch: searchSchema,
  component: function Publish() {
    const search = Route.useSearch();
    const title = getPublishTitle(search);

    return (
      <>
        <DocumentsTopBar title={title} />
        <PublishPage
          editingDraftId={search.draftId}
          contentType={search.contentType ?? "document"}
          replyTo={search.replyTo}
        />
      </>
    );
  }
});
