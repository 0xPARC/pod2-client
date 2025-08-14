import { DocumentMetadata } from "./documentApi";

// Interface for threaded reply structure
export interface ThreadedReply extends DocumentMetadata {
  children: ThreadedReply[];
  depth: number;
}

// Build threaded reply tree from flat list
export const buildReplyTree = (
  replies: DocumentMetadata[]
): ThreadedReply[] => {
  const replyMap = new Map<number, ThreadedReply>();
  const rootReplies: ThreadedReply[] = [];

  // First pass: create all reply objects
  replies.forEach((reply) => {
    const threadedReply: ThreadedReply = {
      ...reply,
      children: [],
      depth: 0
    };
    replyMap.set(reply.id!, threadedReply);
  });

  // Second pass: build parent-child relationships
  replies.forEach((reply) => {
    const threadedReply = replyMap.get(reply.id!)!;

    if (reply.reply_to?.document_id) {
      // This is a reply to another document
      const parentReply = replyMap.get(reply.reply_to.document_id);
      if (parentReply) {
        // It's a reply to another reply
        threadedReply.depth = parentReply.depth + 1;
        parentReply.children.push(threadedReply);
      } else {
        // It's a reply to the original document (not in replies list)
        rootReplies.push(threadedReply);
      }
    } else {
      // Top-level reply
      rootReplies.push(threadedReply);
    }
  });

  return rootReplies;
};
