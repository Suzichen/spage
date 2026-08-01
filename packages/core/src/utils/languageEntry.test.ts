import { describe, expect, it } from 'vitest';
import {
  normalizeSupportedLanguage,
  resolveDefaultLanguage,
  resolveInitialLanguage,
  resolveLanguagePath,
  resolvePostSlug,
  type SupportedLanguage,
} from './languageEntry';

describe('language entry URLs', () => {
  it('normalizes supported languages and initial selection priority', () => {
    expect(normalizeSupportedLanguage('ZH-cn')).toBe('zh-CN');
    expect(normalizeSupportedLanguage('fr')).toBeNull();
    expect(resolveDefaultLanguage('fr')).toBe('en');
    expect(resolveInitialLanguage('/lang/ja', 'zh-CN', 'en')).toBe('ja');
    expect(resolveInitialLanguage('/', 'ja', 'en')).toBe('ja');
    expect(resolveInitialLanguage('/', null, 'zh-CN')).toBe('zh-CN');
    expect(resolvePostSlug('/post/hello%20world/lang/ja')).toBe('hello world');
  });

  it('keeps only non-default language paths with matching routes and translations', () => {
    const cases: [string, SupportedLanguage, SupportedLanguage, string[], string][] = [
      ['/', 'ja', 'en', [], '/lang/ja'],
      ['/lang/ja', 'zh-CN', 'en', [], '/lang/zh-CN'],
      ['/page/2/lang/ja', 'en', 'en', [], '/page/2'],
      ['/post/hello/lang/ja', 'zh-CN', 'en', ['ja', 'zh-CN'], '/post/hello/lang/zh-CN'],
      ['/post/hello/lang/ja', 'zh-CN', 'en', ['ja'], '/post/hello'],
      ['/post/hello', 'en', 'zh-CN', ['en'], '/post/hello/lang/en'],
      ['/lang/en', 'ja', 'ja', [], '/'],
      ['/archives', 'ja', 'en', [], '/archives'],
    ];

    for (const [pathname, language, defaultLanguage, availableLanguages, expected] of cases) {
      expect(resolveLanguagePath({
        pathname,
        language,
        defaultLanguage,
        availableLanguages,
      })).toBe(expected);
    }
  });
});
