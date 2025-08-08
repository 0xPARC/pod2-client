import { ListIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cn } from "../../lib/utils";

interface TocHeading {
  id: string;
  text: string;
  level: number;
}

interface TableOfContentsProps {
  containerRef: React.RefObject<HTMLDivElement | null>;
  scrollContainerRef?: React.RefObject<HTMLDivElement | null>;
  className?: string;
}

export function TableOfContents({
  containerRef,
  scrollContainerRef,
  className
}: TableOfContentsProps) {
  const [headings, setHeadings] = useState<TocHeading[]>([]);
  const [activeId, setActiveId] = useState<string>("");
  const observersRef = useRef<Map<string, IntersectionObserver>>(new Map());

  // Extract headings and set up individual intersection observers
  useEffect(() => {
    const scrollContainer = document.getElementById("documents-app-content");
    if (!scrollContainer) return;

    const timeoutId = setTimeout(() => {
      // Find headings
      const headingElements = scrollContainer.querySelectorAll(
        "h1, h2, h3, h4, h5, h6"
      );

      const extractedHeadings: TocHeading[] = [];
      headingElements.forEach((heading) => {
        const level = parseInt(heading.tagName.charAt(1));
        const text = heading.textContent?.trim() || "";
        const id = heading.id;

        if (id) {
          extractedHeadings.push({ id, text, level });
        }
      });

      setHeadings(extractedHeadings);

      // Clear any existing observers
      observersRef.current.forEach((observer) => observer.disconnect());
      observersRef.current.clear();

      // Create one observer per heading using viewport
      extractedHeadings.forEach((heading) => {
        const element = scrollContainer.querySelector(
          `#${heading.id}`
        ) as HTMLElement;
        if (!element) return;

        const observer = new IntersectionObserver(
          (entries) => {
            const entry = entries[0];
            if (entry.isIntersecting) {
              setActiveId(heading.id);
            }
          },
          {
            root: null, // Use viewport
            rootMargin: "0px 0px -80% 0px", // Top 20% of viewport
            threshold: 0.1
          }
        );

        observer.observe(element);
        observersRef.current.set(heading.id, observer);
      });
    }, 100);

    return () => {
      clearTimeout(timeoutId);
      observersRef.current.forEach((observer) => observer.disconnect());
      observersRef.current.clear();
    };
  }, [containerRef, scrollContainerRef]);

  const handleHeadingClick = (id: string) => {
    const scrollContainer = document.getElementById("documents-app-content");
    if (!scrollContainer) return;

    const element = scrollContainer.querySelector(`#${id}`) as HTMLElement;

    if (element) {
      const containerRect = scrollContainer.getBoundingClientRect();
      const elementRect = element.getBoundingClientRect();
      const relativeTop = elementRect.top - containerRect.top;
      const scrollTop = scrollContainer.scrollTop + relativeTop - 20; // 20px offset from top

      scrollContainer.scrollTo({
        top: scrollTop,
        behavior: "smooth"
      });
    }
  };

  if (headings.length === 0) {
    return (
      <div className={cn("p-4 text-center text-muted-foreground", className)}>
        <ListIcon className="h-8 w-8 mx-auto mb-2 opacity-50" />
        <p className="text-sm">No headings found</p>
      </div>
    );
  }

  return (
    <nav
      className={cn("p-4", className)}
      role="navigation"
      aria-label="Table of contents"
    >
      <h2 className="flex items-center gap-2 text-sm font-semibold text-foreground mb-4">
        <ListIcon className="h-4 w-4" />
        Table of Contents
      </h2>

      <ul className="space-y-1">
        {headings.map((heading) => {
          const isActive = heading.id === activeId;
          const indentLevel = Math.max(0, heading.level - 1); // h1 = 0 indent, h2 = 1 indent, etc.

          return (
            <li key={heading.id}>
              <button
                onClick={() => handleHeadingClick(heading.id)}
                className={cn(
                  "w-full text-left text-sm py-1 px-2 rounded transition-colors hover:bg-muted/50",
                  "border-l-2 border-transparent hover:border-muted",
                  isActive && "bg-muted border-primary text-primary font-medium"
                )}
                style={{
                  paddingLeft: `${0.5 + indentLevel * 0.75}rem`
                }}
                title={heading.text}
              >
                <span className="block truncate">{heading.text}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
