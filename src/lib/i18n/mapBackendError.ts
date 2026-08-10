import i18n from "@/lib/i18n";

/**
 * Map a backend `Result::Err` string to a localized UI message when a stable
 * mapping exists; otherwise return the original text (see leftovers tracker).
 */
export function mapBackendError(message: string | null | undefined): string {
  const trimmed = (message ?? "").trim();
  if (!trimmed) {
    return i18n.t("errors:generic");
  }
  return trimmed;
}
