import { ExternalLinkIcon, LoaderIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "../../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../../ui/card";
import { Input } from "../../ui/input";
import { Label } from "../../ui/label";

interface LinkEditorProps {
  url: string;
  onUrlChange: (url: string) => void;
  className?: string;
}

interface LinkPreview {
  title?: string;
  description?: string;
  image?: string;
  siteName?: string;
}

export function LinkEditor({ url, onUrlChange, className }: LinkEditorProps) {
  const [preview, setPreview] = useState<LinkPreview | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Validate URL format
  const isValidUrl = useCallback((urlString: string): boolean => {
    try {
      new URL(urlString);
      return true;
    } catch {
      return false;
    }
  }, []);

  // Mock function to fetch link metadata - in a real app, this would call an API
  const fetchLinkPreview = useCallback(
    async (urlString: string): Promise<LinkPreview | null> => {
      // This is a mock implementation
      // In a real app, you'd call your backend or a service like Open Graph API

      if (!isValidUrl(urlString)) {
        throw new Error("Invalid URL format");
      }

      // Mock data based on URL patterns
      const url = new URL(urlString);
      const hostname = url.hostname.toLowerCase();

      if (hostname.includes("github.com")) {
        return {
          title: "GitHub Repository",
          description: "A code repository hosted on GitHub",
          siteName: "GitHub",
          image:
            "https://github.githubassets.com/images/modules/logos_page/GitHub-Mark.png"
        };
      } else if (
        hostname.includes("youtube.com") ||
        hostname.includes("youtu.be")
      ) {
        return {
          title: "YouTube Video",
          description: "A video hosted on YouTube",
          siteName: "YouTube",
          image: "https://www.youtube.com/img/desktop/yt_1200.png"
        };
      } else if (
        hostname.includes("twitter.com") ||
        hostname.includes("x.com")
      ) {
        return {
          title: "Post on X",
          description: "A post on X (formerly Twitter)",
          siteName: "X"
        };
      } else {
        return {
          title: `Content from ${hostname}`,
          description: `A webpage hosted on ${hostname}`,
          siteName: hostname
        };
      }
    },
    [isValidUrl]
  );

  // Fetch preview when URL changes
  useEffect(() => {
    if (!url.trim()) {
      setPreview(null);
      setError(null);
      return;
    }

    const timeoutId = setTimeout(async () => {
      setIsLoading(true);
      setError(null);

      try {
        const previewData = await fetchLinkPreview(url);
        setPreview(previewData);
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Failed to fetch link preview"
        );
        setPreview(null);
      } finally {
        setIsLoading(false);
      }
    }, 500); // Debounce API calls

    return () => clearTimeout(timeoutId);
  }, [url, fetchLinkPreview]);

  const handleOpenLink = () => {
    if (isValidUrl(url)) {
      window.open(url, "_blank", "noopener,noreferrer");
    }
  };

  return (
    <div className={`space-y-4 ${className}`}>
      {/* URL Input */}
      <div className="space-y-2">
        <Label htmlFor="url-input">URL</Label>
        <div className="flex gap-2">
          <Input
            id="url-input"
            type="url"
            placeholder="https://example.com"
            value={url}
            onChange={(e) => onUrlChange(e.target.value)}
            className={error ? "border-destructive" : ""}
          />
          {isValidUrl(url) && (
            <Button
              variant="outline"
              size="sm"
              onClick={handleOpenLink}
              title="Open link in new tab"
            >
              <ExternalLinkIcon className="w-4 h-4" />
            </Button>
          )}
        </div>
        {error && <p className="text-sm text-destructive">{error}</p>}
      </div>

      {/* Loading state */}
      {isLoading && (
        <div className="flex items-center gap-2 text-muted-foreground">
          <LoaderIcon className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading preview...</span>
        </div>
      )}

      {/* Link Preview */}
      {preview && !isLoading && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="flex items-start justify-between">
              <div>
                <h3 className="text-lg font-semibold">{preview.title}</h3>
                {preview.siteName && (
                  <p className="text-sm text-muted-foreground mt-1">
                    {preview.siteName}
                  </p>
                )}
              </div>
              {preview.image && (
                <img
                  src={preview.image}
                  alt="Link preview"
                  className="w-16 h-16 object-cover rounded ml-4 flex-shrink-0"
                  onError={(e) => {
                    e.currentTarget.style.display = "none";
                  }}
                />
              )}
            </CardTitle>
          </CardHeader>
          {preview.description && (
            <CardContent className="pt-0">
              <p className="text-sm text-muted-foreground">
                {preview.description}
              </p>
            </CardContent>
          )}
        </Card>
      )}

      {/* Help text */}
      <div className="text-sm text-muted-foreground">
        <p>
          Enter a URL to share a link. The preview will be generated
          automatically.
        </p>
      </div>
    </div>
  );
}
