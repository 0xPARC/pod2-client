import { QueryClient } from "@tanstack/react-query";
import { fetchDocument, fetchDocumentReplyTree } from "./documentApi";

export const queryClient = new QueryClient();

export const documentsQueryKey = ["documents"] as const;
const documentQueryKey = (id: number) => ["document", id] as const;
export const replyTreeQueryKey = (id: number) =>
  ["document", id, "replyTree"] as const;

export const documentQueryOptions = (id: number) => ({
  queryKey: documentQueryKey(id),
  queryFn: () => fetchDocument(id)
});

export const replyTreeQueryOptions = (id: number) => ({
  queryKey: replyTreeQueryKey(id),
  queryFn: () => fetchDocumentReplyTree(id)
});
