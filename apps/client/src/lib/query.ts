import { QueryClient } from "@tanstack/react-query";
import { Document, fetchDocument, fetchDocumentReplyTree } from "./documentApi";

export const queryClient = new QueryClient();

export const documentsQueryKey = ["documents"] as const;
const documentQueryKey = (id: number) => ["document", id] as const;
const replyTreeQueryKey = (id: number) =>
  ["document", id, "replyTree"] as const;

export const documentQueryOptions = (id: number) => ({
  queryKey: documentQueryKey(id),
  queryFn: () => fetchDocument(id),
  initialData: () => {
    const list = queryClient.getQueryData<Document[]>(documentsQueryKey);
    console.log(
      "list",
      list?.map((d) => d.metadata.id)
    );
    return list?.find((d) => d.metadata.id === id);
  }
});

export const replyTreeQueryOptions = (id: number) => ({
  queryKey: replyTreeQueryKey(id),
  queryFn: () => fetchDocumentReplyTree(id)
});
