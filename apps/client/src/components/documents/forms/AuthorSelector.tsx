import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from "@/components/ui/select";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Author } from "../../../lib/documentApi";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "../../ui/popover";

type Status = "idle" | "validating" | "valid" | "error";

interface AuthorSelectorProps {
  value: Author[];
  onChange: (authors: Author[]) => void;
  defaultUser?: string | null;
  label?: string;
}

function authorKey(a: Author) {
  return a.author_type === "github"
    ? `github:${a.github_username}`
    : `user:${a.username}`;
}

function displayName(a: Author) {
  return a.author_type === "github" ? a.github_username : a.username;
}

export function AuthorSelector({
  value,
  onChange,
  defaultUser,
  label = "Authors"
}: AuthorSelectorProps) {
  const [open, setOpen] = useState(false);
  const [type, setType] = useState<"user" | "github">("user");
  const [username, setUsername] = useState("");
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string>("");
  const [githubUserId, setGithubUserId] = useState<string>("");

  // Prefill default user if none
  useEffect(() => {
    if (defaultUser && value.length === 0) {
      onChange([{ author_type: "user", username: defaultUser }]);
    }
  }, [defaultUser, value.length, onChange]);

  // Debounced GitHub validation
  const debounceRef = useRef<number | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    if (type !== "github") {
      setStatus(username ? "valid" : "idle");
      setError("");
      setGithubUserId("");
      return;
    }

    const candidate = username.trim();
    if (!candidate) {
      setStatus("idle");
      setError("");
      setGithubUserId("");
      return;
    }

    // Basic pattern check first
    if (!/^[a-zA-Z0-9-]{1,39}$/.test(candidate)) {
      setStatus("error");
      setError("Invalid GitHub username format");
      setGithubUserId("");
      return;
    }

    setStatus("validating");
    setError("");

    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(async () => {
      if (abortRef.current) abortRef.current.abort();
      const ctrl = new AbortController();
      abortRef.current = ctrl;

      try {
        const resp = await fetch(`https://api.github.com/users/${candidate}`, {
          signal: ctrl.signal
        });
        if (resp.status === 200) {
          const data = await resp.json();
          setGithubUserId(String(data.id ?? ""));
          setStatus("valid");
          setError("");
        } else if (resp.status === 404) {
          setStatus("error");
          setError("GitHub user not found");
          setGithubUserId("");
        } else if (resp.status === 403) {
          setStatus("error");
          setError("GitHub API rate limited — try later");
          setGithubUserId("");
        } else {
          setStatus("error");
          setError(`GitHub API error (${resp.status})`);
          setGithubUserId("");
        }
      } catch (e) {
        if ((e as any)?.name === "AbortError") return;
        setStatus("error");
        setError("Network error validating GitHub username");
        setGithubUserId("");
      }
    }, 600);

    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
      if (abortRef.current) abortRef.current.abort();
    };
  }, [type, username]);

  const canAdd = useMemo(() => {
    if (!username.trim()) return false;
    if (type === "user") return true;
    return status === "valid" && !!githubUserId;
  }, [type, username, status, githubUserId]);

  const addAuthor = () => {
    const uname = username.trim();
    if (!uname) return;
    let newAuthor: Author;
    if (type === "user") {
      newAuthor = { author_type: "user", username: uname };
    } else {
      if (!githubUserId) return;
      newAuthor = {
        author_type: "github",
        github_username: uname,
        github_userid: githubUserId
      };
    }
    const key = authorKey(newAuthor);
    const exists = value.some((a) => authorKey(a) === key);
    if (exists) {
      setError("Author already added");
      setStatus("error");
      return;
    }
    onChange([...value, newAuthor]);
    setUsername("");
    setGithubUserId("");
    setStatus("idle");
    setError("");
    setOpen(false);
  };

  const removeAuthor = (index: number) => {
    const next = [...value];
    next.splice(index, 1);
    onChange(next);
  };

  return (
    <div className="flex items-center gap-4">
      <div className="flex items-center gap-2">
        <div className="text-sm text-muted-foreground">{label}</div>
        <div className="flex flex-wrap gap-2">
          {value.map((a, i) => (
            <Badge key={authorKey(a)} variant="secondary" className="text-xs">
              {a.author_type}: {displayName(a)}
              <button
                className="ml-2 text-muted-foreground hover:text-foreground"
                onClick={() => removeAuthor(i)}
                aria-label={`Remove ${displayName(a)}`}
              >
                ×
              </button>
            </Badge>
          ))}
        </div>
      </div>

      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button variant="outline" size="sm">
            +
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-[450px]">
          <div className="flex items-center gap-2 overflow-hidden">
            <div className="flex-shrink-0">
              <Select
                value={type}
                onValueChange={(value) => setType(value as "user" | "github")}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select author type" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="user">User</SelectItem>
                  <SelectItem value="github">GitHub</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <Input
              placeholder={type === "github" ? "octocat" : "username"}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="w-48"
            />

            <Button size="sm" onClick={addAuthor} disabled={!canAdd}>
              Add
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setOpen(false);
                setStatus("idle");
                setError("");
                setUsername("");
                setGithubUserId("");
              }}
            >
              Cancel
            </Button>
          </div>
          {type === "github" && status !== "idle" && (
            <div className="text-xs mt-4">
              {status === "validating" && (
                <span className="text-muted-foreground">Validating…</span>
              )}
              {status === "valid" && (
                <span className="text-green-600">✓ Found on GitHub</span>
              )}
              {status === "error" && (
                <span className="text-destructive">{error}</span>
              )}
            </div>
          )}
        </PopoverContent>
      </Popover>
    </div>
  );
}
