import {
  AlertCircleIcon,
  ArrowUpDownIcon,
  ChevronDownIcon,
  FileTextIcon,
  FilterIcon,
  PlusIcon,
  RefreshCwIcon,
  SearchIcon,
  XIcon
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { DocumentMetadata, fetchDocuments } from "../../lib/documentApi";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent } from "../ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from "../ui/dropdown-menu";
import { Input } from "../ui/input";
import { DocumentDetailView } from "./DocumentDetailView";
import { PublishPage } from "./PublishPage";

export function DocumentsView() {
  const [documents, setDocuments] = useState<DocumentMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedDocumentId, setSelectedDocumentId] = useState<number | null>(
    null
  );
  const [showPublishForm, setShowPublishForm] = useState(false);
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [sortBy, setSortBy] = useState<"recent" | "upvotes">("recent");
  const [searchQuery, setSearchQuery] = useState<string>("");

  const loadDocuments = async () => {
    try {
      setLoading(true);
      setError(null);
      const docs = await fetchDocuments();
      setDocuments(docs);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load documents");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadDocuments();
  }, []);

  // Extract all unique tags from documents
  const availableTags = useMemo(() => {
    const tagSet = new Set<string>();
    documents.forEach((doc) => {
      doc.tags.forEach((tag) => tagSet.add(tag));
    });
    return Array.from(tagSet).sort();
  }, [documents]);

  // Filter, search, and sort documents
  const filteredAndSortedDocuments = useMemo(() => {
    let filtered = documents;

    // Filter by search query (case insensitive search in titles)
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter((doc) =>
        doc.title.toLowerCase().includes(query)
      );
    }

    // Filter by tag
    if (selectedTag) {
      filtered = filtered.filter((doc) => doc.tags.includes(selectedTag));
    }

    // Sort
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === "recent") {
        // Sort by created_at in reverse chronological order (most recent first)
        const dateA = new Date(a.created_at || 0).getTime();
        const dateB = new Date(b.created_at || 0).getTime();
        return dateB - dateA;
      } else if (sortBy === "upvotes") {
        // Sort by upvote count (highest first)
        return b.upvote_count - a.upvote_count;
      }
      return 0;
    });

    return sorted;
  }, [documents, searchQuery, selectedTag, sortBy]);

  const formatDate = (dateString?: string) => {
    if (!dateString) return "unknown time ago";

    // Server timestamps are UTC, current time is local - let JS handle the conversion
    const now = new Date();
    const date = new Date(dateString + (dateString.endsWith("Z") ? "" : "Z")); // Ensure UTC parsing
    const diffMs = now.getTime() - date.getTime();

    const diffMinutes = Math.floor(diffMs / (1000 * 60));
    const diffHours = Math.floor(diffMinutes / 60);
    const diffDays = Math.floor(diffHours / 24);
    const diffMonths = Math.floor(diffDays / 30);
    const diffYears = Math.floor(diffDays / 365);

    if (diffYears > 0) {
      return `${diffYears} year${diffYears > 1 ? "s" : ""} ago`;
    } else if (diffMonths > 0) {
      return `${diffMonths} month${diffMonths > 1 ? "s" : ""} ago`;
    } else if (diffDays > 0) {
      return `${diffDays} day${diffDays > 1 ? "s" : ""} ago`;
    } else if (diffHours > 0) {
      return `${diffHours} hour${diffHours > 1 ? "s" : ""} ago`;
    } else if (diffMinutes > 0) {
      return `${diffMinutes} minute${diffMinutes > 1 ? "s" : ""} ago`;
    } else {
      return "just now";
    }
  };

  // If publish form is shown, show the publish page
  if (showPublishForm) {
    return (
      <PublishPage
        onBack={() => setShowPublishForm(false)}
        onPublishSuccess={(documentId) => {
          console.log("Document published with ID:", documentId);
          setShowPublishForm(false);
          loadDocuments(); // Refresh the document list
        }}
      />
    );
  }

  // If a document is selected, show the detail view
  if (selectedDocumentId !== null) {
    return (
      <DocumentDetailView
        documentId={selectedDocumentId}
        onBack={() => setSelectedDocumentId(null)}
      />
    );
  }

  return (
    <div className="p-6 min-h-screen w-full overflow-y-auto">
      <div className="w-full">
        <div className="mb-6 flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold mb-2">Documents</h1>
            <p className="text-muted-foreground">
              Documents from the PodNet server with cryptographic verification.
            </p>
          </div>
          <div className="flex gap-2">
            <Button
              onClick={() => setShowPublishForm(true)}
              className="bg-primary hover:bg-primary/90"
            >
              <PlusIcon className="h-4 w-4 mr-2" />
              Publish Document
            </Button>
            <Button
              onClick={loadDocuments}
              disabled={loading}
              variant="outline"
            >
              <RefreshCwIcon
                className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`}
              />
              Refresh
            </Button>
          </div>
        </div>

        {/* Search */}
        <div className="mb-4">
          <div className="relative">
            <SearchIcon className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search document titles..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
            {searchQuery && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setSearchQuery("")}
                className="absolute right-1 top-1/2 transform -translate-y-1/2 h-6 w-6 p-0"
              >
                <XIcon className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>

        {/* Filters and Sorting */}
        <div className="mb-4 flex items-center gap-2 flex-wrap">
          {/* Sort Dropdown */}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="flex items-center gap-2">
                <ArrowUpDownIcon className="h-4 w-4" />
                Sort: {sortBy === "recent" ? "Most Recent" : "Most Upvoted"}
                <ChevronDownIcon className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-44">
              <DropdownMenuLabel>Sort By</DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => setSortBy("recent")}>
                <span>Most Recent</span>
                {sortBy === "recent" && (
                  <div className="ml-auto h-2 w-2 bg-primary rounded-full" />
                )}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => setSortBy("upvotes")}>
                <span>Most Upvoted</span>
                {sortBy === "upvotes" && (
                  <div className="ml-auto h-2 w-2 bg-primary rounded-full" />
                )}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          {/* Tag Filter */}
          {availableTags.length > 0 && (
            <>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" className="flex items-center gap-2">
                    <FilterIcon className="h-4 w-4" />
                    {selectedTag ? `Tag: ${selectedTag}` : "Filter by Tag"}
                    <ChevronDownIcon className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="w-48">
                  <DropdownMenuLabel>Filter by Tag</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={() => setSelectedTag(null)}>
                    <span>Show All Documents</span>
                    {!selectedTag && (
                      <div className="ml-auto h-2 w-2 bg-primary rounded-full" />
                    )}
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  {availableTags.map((tag) => (
                    <DropdownMenuItem
                      key={tag}
                      onClick={() => setSelectedTag(tag)}
                    >
                      <span>{tag}</span>
                      {selectedTag === tag && (
                        <div className="ml-auto h-2 w-2 bg-primary rounded-full" />
                      )}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
              {selectedTag && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setSelectedTag(null)}
                  className="h-8 px-2"
                >
                  <XIcon className="h-4 w-4" />
                </Button>
              )}
            </>
          )}

          {(selectedTag || searchQuery.trim()) && (
            <Badge variant="secondary" className="ml-2">
              {filteredAndSortedDocuments.length} document
              {filteredAndSortedDocuments.length !== 1 ? "s" : ""}
              {searchQuery.trim() && selectedTag
                ? ` matching "${searchQuery}" with tag "${selectedTag}"`
                : searchQuery.trim()
                  ? ` matching "${searchQuery}"`
                  : ` with tag "${selectedTag}"`}
            </Badge>
          )}
        </div>

        {error && (
          <Card className="mb-6 border-destructive">
            <CardContent className="pt-6">
              <div className="flex items-center gap-2 text-destructive">
                <AlertCircleIcon className="h-5 w-5" />
                <span>{error}</span>
              </div>
            </CardContent>
          </Card>
        )}

        {loading ? (
          <Card>
            <CardContent className="pt-6">
              <div className="flex items-center justify-center py-8">
                <RefreshCwIcon className="h-6 w-6 animate-spin mr-2" />
                Loading documents...
              </div>
            </CardContent>
          </Card>
        ) : filteredAndSortedDocuments.length === 0 ? (
          <Card>
            <CardContent className="pt-6">
              <div className="text-center py-8">
                <FileTextIcon className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
                <p className="text-muted-foreground">
                  {searchQuery.trim() && selectedTag
                    ? `No documents found matching "${searchQuery}" with tag "${selectedTag}"`
                    : searchQuery.trim()
                      ? `No documents found matching "${searchQuery}"`
                      : selectedTag
                        ? `No documents found with tag "${selectedTag}"`
                        : "No documents found"}
                </p>
              </div>
            </CardContent>
          </Card>
        ) : (
          <div className="space-y-1">
            {filteredAndSortedDocuments.map((doc) => (
              <div
                key={doc.id}
                className="flex items-start gap-2 p-2 hover:bg-muted/50 cursor-pointer border-b border-border/50"
                onClick={() => setSelectedDocumentId(doc.id!)}
              >
                {/* Upvote section - Reddit-style */}
                <div className="flex flex-col items-center min-w-[60px] pt-1">
                  <div className="text-xs text-muted-foreground">▲</div>
                  <div className="text-sm font-bold text-orange-600">
                    {doc.upvote_count}
                  </div>
                  <div className="text-xs text-muted-foreground">▼</div>
                </div>

                {/* Content section */}
                <div className="flex-1 min-w-0">
                  <div className="flex items-start gap-2">
                    <div className="flex-1">
                      <h3 className="text-sm font-medium text-blue-600 hover:underline line-clamp-2 mb-1">
                        {doc.title}
                      </h3>

                      <div className="flex items-center gap-1 text-xs text-muted-foreground mb-1">
                        <span>submitted</span>
                        <span>{formatDate(doc.created_at)}</span>
                        <span>by</span>
                        <span className="text-blue-600">{doc.uploader_id}</span>
                        {doc.reply_to && (
                          <>
                            <span>•</span>
                            <span>reply to #{doc.reply_to}</span>
                          </>
                        )}
                      </div>

                      {/* Tags and authors in compact format */}
                      <div className="flex items-center gap-2 text-xs">
                        {doc.tags.length > 0 && (
                          <div className="flex gap-1">
                            {doc.tags.slice(0, 3).map((tag, index) => (
                              <span
                                key={index}
                                className="bg-muted text-muted-foreground px-1 py-0.5 rounded text-xs"
                              >
                                {tag}
                              </span>
                            ))}
                            {doc.tags.length > 3 && (
                              <span className="text-muted-foreground">
                                +{doc.tags.length - 3} more
                              </span>
                            )}
                          </div>
                        )}

                        {doc.authors.length > 0 && (
                          <div className="flex gap-1">
                            <span className="text-muted-foreground">
                              authors:
                            </span>
                            {doc.authors.slice(0, 2).map((author, index) => (
                              <span key={index} className="text-blue-600">
                                {author}
                                {index < Math.min(doc.authors.length, 2) - 1 &&
                                  ","}
                              </span>
                            ))}
                            {doc.authors.length > 2 && (
                              <span className="text-muted-foreground">
                                +{doc.authors.length - 2} more
                              </span>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                </div>

                {/* Right side info */}
                <div className="text-xs text-muted-foreground text-right">
                  <div>#{doc.id}</div>
                  <div>r{doc.revision}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
