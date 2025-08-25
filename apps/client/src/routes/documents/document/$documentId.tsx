import { DocumentDetailView } from "@/components/documents/DocumentDetailView";
import { DocumentsTopBar } from "@/components/documents/DocumentsTopBar";
import { Card, CardContent } from "@/components/ui/card";
import { getCurrentUsername } from "@/lib/documentApi";
import { documentQueryOptions, replyTreeQueryOptions } from "@/lib/query";
import type { RouterContext } from "@/lib/router";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { AlertCircleIcon } from "lucide-react";

export const Route = createFileRoute("/documents/document/$documentId")({
  staticData: {
    breadcrumb: ({ loaderData }: any) => loaderData?.title ?? "Document"
  },
  loader: async ({ params, context }) => {
    const { queryClient } = context as RouterContext;
    const id = Number(params.documentId);
    if (!Number.isFinite(id)) throw new Error("Invalid document id");

    const doc = await queryClient.ensureQueryData(documentQueryOptions(id));
    const replyTree = queryClient.ensureQueryData(replyTreeQueryOptions(id));
    return { document: doc, replyTree, username: getCurrentUsername() };
  },
  component: function DocumentDetail() {
    const loaderData = Route.useLoaderData();
    const navigate = useNavigate();

    const handleNewDocument = () => {
      navigate({ to: "/documents/publish" });
    };

    return (
      <>
        <DocumentsTopBar
          title={loaderData.document.metadata.title}
          prefix="Document:"
          onNewDocument={handleNewDocument}
        />
        <DocumentDetailView />
      </>
    );
  },
  errorComponent: ({ error }) => {
    return (
      <div className="p-6 min-h-screen w-full">
        <div className="w-full">
          <Card className="border-destructive">
            <CardContent className="pt-6">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircleIcon className="h-5 w-5" />
                <span>{error.message || "Document not found"}</span>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }
});
