// Format date string for display
export const formatDate = (dateString?: string) => {
  if (!dateString) return "Unknown";
  return new Date(dateString).toLocaleDateString(undefined, {
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
};

// Format date string for compact display (used in replies)
export const formatDateCompact = (dateString?: string) => {
  if (!dateString) return "Unknown";
  return new Date(dateString).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit"
  });
};
