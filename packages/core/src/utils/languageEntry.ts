export const SUPPORTED_LANGUAGES = ['en', 'zh-CN', 'ja'] as const;
const LANGUAGE_ENTRY_PATTERN = /\/lang\/([^/]+)\/?$/;

export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export interface LanguageEntry {
  language: SupportedLanguage | null;
  destination: string;
}

export function normalizeSupportedLanguage(language: string | undefined): SupportedLanguage | null {
  return language
    ? SUPPORTED_LANGUAGES.find((candidate) => candidate.toLowerCase() === language.toLowerCase()) ?? null
    : null;
}

export function resolveLanguageEntryDestination(pathname: string): string {
  const destination = pathname.replace(LANGUAGE_ENTRY_PATTERN, '');
  return destination || '/';
}

export function parseLanguageEntry(pathname: string): LanguageEntry | null {
  const match = pathname.match(LANGUAGE_ENTRY_PATTERN);
  return match
    ? {
        language: normalizeSupportedLanguage(match[1]),
        destination: resolveLanguageEntryDestination(pathname),
      }
    : null;
}

export function resolvePostSlug(pathname: string): string | null {
  const match = resolveLanguageEntryDestination(pathname).match(/^\/post\/([^/]+)\/?$/);
  if (!match) return null;

  try {
    return decodeURIComponent(match[1]);
  } catch {
    return match[1];
  }
}

export function resolveDefaultLanguage(language: string | undefined): SupportedLanguage {
  return normalizeSupportedLanguage(language) ?? 'en';
}

export function resolveInitialLanguage(
  pathname: string,
  storedLanguage: string | null,
  defaultLanguage: SupportedLanguage,
): SupportedLanguage {
  return parseLanguageEntry(pathname)?.language
    ?? normalizeSupportedLanguage(storedLanguage ?? undefined)
    ?? defaultLanguage;
}

interface ResolveLanguagePathOptions {
  pathname: string;
  language: SupportedLanguage;
  defaultLanguage: SupportedLanguage;
  availableLanguages?: readonly string[];
}

export function resolveLanguagePath({
  pathname,
  language,
  defaultLanguage,
  availableLanguages = [],
}: ResolveLanguagePathOptions): string {
  const cleanPath = resolveLanguageEntryDestination(pathname);

  // The configured default language owns the canonical path. Non-default
  // language paths are limited to list pages and posts with that translation;
  // all other routes fall back to their language-neutral canonical URL.
  if (language === defaultLanguage) return cleanPath;
  if (cleanPath === '/' || /^\/page\/\d+\/?$/.test(cleanPath)) {
    return cleanPath === '/'
      ? `/lang/${language}`
      : `${cleanPath.replace(/\/$/, '')}/lang/${language}`;
  }
  if (/^\/post\/[^/]+\/?$/.test(cleanPath) && availableLanguages.includes(language)) {
    return `${cleanPath.replace(/\/$/, '')}/lang/${language}`;
  }

  return cleanPath;
}
