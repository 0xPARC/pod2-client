import { Author } from "@/lib/documentApi";

export function AuthorName({ author }: { author: Author }) {
  switch (author.author_type) {
    case "user":
      return <span>{author.username}</span>;
    case "github":
      return <span>{author.github_username}</span>;
    default:
      author satisfies never;
  }
}
