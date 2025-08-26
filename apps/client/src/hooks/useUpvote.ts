import {
  AlreadyUpvotedError,
  upvoteDocument,
  UpvoteFailedError
} from "@/lib/documentApi";
import { useCallback, useState } from "react";
import { toast } from "sonner";

export function useUpvote(documentId: number, initialCount: number) {
  const [count, setCount] = useState(initialCount);
  const [error, setError] = useState<string | null>(null);
  const [alreadyUpvoted, setAlreadyUpvoted] = useState(false);
  const [isPending, setIsPending] = useState(false);

  const upvote = useCallback(async () => {
    if (isPending) {
      return;
    }

    setIsPending(true);
    const previousCount = count;
    setCount((prev) => prev + 1);

    const result = await upvoteDocument(documentId);

    if (result.ok) {
      setCount(result.value);
    } else {
      setCount(previousCount);
      result
        .match()
        .when(AlreadyUpvotedError, () => setAlreadyUpvoted(true))
        .when(UpvoteFailedError, (error) => {
          setError(error.message);
        })
        .run();
    }
    setIsPending(false);
    return result.getOrThrow();
  }, [documentId, count, isPending]);

  return { count, upvote, isPending, error, alreadyUpvoted };
}

export function useUpvoteWithToast(documentId: number, initialCount: number) {
  const { count, upvote, isPending, error } = useUpvote(
    documentId,
    initialCount
  );

  const upvoteWithToast = useCallback(async () => {
    const upvotePromise = upvote();

    toast.promise(upvotePromise, {
      loading: "Generating upvote POD...",
      success: "Upvote successful",
      error: (error) => {
        if (error instanceof AlreadyUpvotedError) {
          return "You have already upvoted this document";
        }
        return error.message;
      }
    });
  }, [upvote]);

  return { count, upvote: upvoteWithToast, isPending, error };
}
