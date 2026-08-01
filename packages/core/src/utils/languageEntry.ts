export const SUPPORTED_LANGUAGES = ['en', 'zh-CN', 'ja'] as const;

export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export function normalizeSupportedLanguage(language: string | undefined): SupportedLanguage | null {
  return language
    ? SUPPORTED_LANGUAGES.find((candidate) => candidate.toLowerCase() === language.toLowerCase()) ?? null
    : null;
}

export function resolveLanguageEntryDestination(pathname: string): string {
  const destination = pathname.replace(/\/lang\/[^/]+\/?$/, '');
  return destination || '/';
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
  return normalizeSupportedLanguage(pathname.match(/\/lang\/([^/]+)\/?$/)?.[1])
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
