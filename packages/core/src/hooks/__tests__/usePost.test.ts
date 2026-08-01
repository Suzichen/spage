import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { usePost } from '../usePost';

// Mock react-i18next
const mockChangeLanguage = vi.fn();
let mockResolvedLanguage = 'en';
let mockBasePath = '';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: {
      get resolvedLanguage() {
        return mockResolvedLanguage;
      },
      changeLanguage: mockChangeLanguage,
    },
    t: (key: string) => key,
  }),
}));

// Mock useSiteConfig
vi.mock('../../context', () => ({
  useSiteConfig: () => ({
    title: 'Test Blog',
    description: 'Test',
    logo: '/logo.png',
    favicon: '/favicon.ico',
    language: 'en',
    basePath: mockBasePath,
  }),
}));

// Mock usePosts
const mockPosts = vi.fn();

vi.mock('@/hooks/usePosts', () => ({
  usePosts: () => mockPosts(),
}));

// Helper to create mock fetch responses
function createTextResponse(text: string, ok = true, status = 200): Response {
  return {
    ok,
    status,
    statusText: ok ? 'OK' : 'Not Found',
    text: () => Promise.resolve(text),
    json: () => Promise.resolve(JSON.parse(text)),
    headers: new Headers(),
    redirected: false,
    type: 'basic',
    url: '',
    clone: function () {
      return this;
    },
    body: null,
    bodyUsed: false,
    arrayBuffer: () => Promise.resolve(new ArrayBuffer(0)),
    blob: () => Promise.resolve(new Blob()),
    formData: () => Promise.resolve(new FormData()),
    bytes: () => Promise.resolve(new Uint8Array()),
  } as Response;
}

describe('usePost - Language-aware loading', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockResolvedLanguage = 'en';
    mockBasePath = '';
  });

  /**
   * Validates: Requirement 4.1
   * WHEN user visits a post detail page, THE Frontend SHALL prefer loading
   * the Localized_File matching Current_Language.
   */
  describe('Load localized file (Requirement 4.1)', () => {
    it('fetches localized file when currentLang is in availableLanguages', async () => {
      mockPosts.mockReturnValue({
        posts: [
          {
            slug: 'hello-world',
            title: 'Hello World',
            date: '2025-01-15T10:30:00',
            tags: [],
            categories: [],
            summary: 'A post',
            availableLanguages: ['zh-CN', 'en'],
          },
        ],
        loading: false,
        error: null,
      });

      mockResolvedLanguage = 'zh-CN';

      const mockContent = '---\ntitle: 你好世界\n---\n# 你好世界内容';
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse(mockContent)
      );

      const { result } = renderHook(() => usePost('hello-world'));

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      expect(global.fetch).toHaveBeenCalledWith(
        '/posts/hello-world.zh-CN.md',
        { cache: 'no-cache' }
      );
      expect(result.current.content).toBe('# 你好世界内容');
      expect(result.current.error).toBeNull();
    });

    it('fetches default file when currentLang is NOT in availableLanguages', async () => {
      mockPosts.mockReturnValue({
        posts: [
          {
            slug: 'hello-world',
            title: 'Hello World',
            date: '2025-01-15T10:30:00',
            tags: [],
            categories: [],
            summary: 'A post',
            availableLanguages: ['zh-CN'],
          },
        ],
        loading: false,
        error: null,
      });

      mockResolvedLanguage = 'en';

      const mockContent = '---\ntitle: Hello World\n---\n# Hello World Content';
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse(mockContent)
      );

      const { result } = renderHook(() => usePost('hello-world'));

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      expect(global.fetch).toHaveBeenCalledWith('/posts/hello-world.md', {
        cache: 'no-cache',
      });
      expect(result.current.content).toBe('# Hello World Content');
      expect(result.current.error).toBeNull();
    });

    it('prefixes post files with the configured base path', async () => {
      mockBasePath = '/blog/';
      mockPosts.mockReturnValue({
        posts: [{
          slug: 'hello-world',
          title: 'Hello World',
          date: '2025-01-15T10:30:00',
          tags: [],
          categories: [],
          summary: 'A post',
          availableLanguages: ['ja'],
        }],
        loading: false,
        error: null,
      });
      mockResolvedLanguage = 'ja';
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse('# Japanese content')
      );

      const { result } = renderHook(() => usePost('hello-world'));

      await waitFor(() => expect(result.current.loading).toBe(false));
      expect(global.fetch).toHaveBeenCalledWith('/blog/posts/hello-world.ja.md', {
        cache: 'no-cache',
      });
    });
  });

  /**
   * Validates: Requirement 4.2
   * IF the localized file does not exist (404), THEN THE Frontend SHALL
   * fall back to loading the Default_File.
   */
  describe('Fallback to default file (Requirement 4.2)', () => {
    it('falls back to default file when localized file returns 404', async () => {
      mockPosts.mockReturnValue({
        posts: [
          {
            slug: 'about',
            title: 'About',
            date: '2025-01-10T00:00:00',
            tags: [],
            categories: [],
            summary: 'About page',
            availableLanguages: ['ja'],
          },
        ],
        loading: false,
        error: null,
      });

      mockResolvedLanguage = 'ja';

      // First fetch (localized) returns 404
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse('', false, 404)
      );
      // Second fetch (default) returns content
      const defaultContent = '---\ntitle: About\n---\n# About this blog';
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse(defaultContent)
      );

      const { result } = renderHook(() => usePost('about'));

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // First call should be the localized URL
      expect(global.fetch).toHaveBeenNthCalledWith(1, '/posts/about.ja.md', {
        cache: 'no-cache',
      });
      // Second call should be the default URL
      expect(global.fetch).toHaveBeenNthCalledWith(2, '/posts/about.md', {
        cache: 'no-cache',
      });
      expect(result.current.content).toBe('# About this blog');
      expect(result.current.error).toBeNull();
    });
  });

  /**
   * Validates: Requirement 4.3
   * IF Default_File also does not exist (404), THEN THE Frontend SHALL
   * display an error.
   */
  describe('Both files 404 - error state (Requirement 4.3)', () => {
    it('sets error when both localized and default files return 404', async () => {
      mockPosts.mockReturnValue({
        posts: [
          {
            slug: 'missing-post',
            title: 'Missing',
            date: '2025-01-01T00:00:00',
            tags: [],
            categories: [],
            summary: 'Missing post',
            availableLanguages: ['zh-CN'],
          },
        ],
        loading: false,
        error: null,
      });

      mockResolvedLanguage = 'zh-CN';

      // Both fetches return 404
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse('', false, 404)
      );
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse('', false, 404)
      );

      const { result } = renderHook(() => usePost('missing-post'));

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      expect(result.current.error).not.toBeNull();
      expect(result.current.error?.message).toContain('Failed to load post');
      expect(result.current.content).toBe('');
    });
  });

  /**
   * Validates: Requirement 4.4
   * WHEN Current_Language changes, THE Frontend SHALL automatically
   * reload the post content.
   */
  describe('Language change reload (Requirement 4.4)', () => {
    it('re-fetches content when language changes', async () => {
      mockPosts.mockReturnValue({
        posts: [
          {
            slug: 'hello-world',
            title: 'Hello World',
            date: '2025-01-15T10:30:00',
            tags: [],
            categories: [],
            summary: 'A post',
            availableLanguages: ['zh-CN', 'en'],
          },
        ],
        loading: false,
        error: null,
      });

      mockResolvedLanguage = 'en';

      const enContent = '---\ntitle: Hello\n---\n# English content';
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse(enContent)
      );

      const { result, rerender } = renderHook(() => usePost('hello-world'));

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      expect(global.fetch).toHaveBeenCalledWith('/posts/hello-world.en.md', {
        cache: 'no-cache',
      });
      expect(result.current.content).toBe('# English content');

      // Change language to zh-CN
      const zhContent = '---\ntitle: 你好\n---\n# 中文内容';
      (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
        createTextResponse(zhContent)
      );

      await act(async () => {
        mockResolvedLanguage = 'zh-CN';
        rerender();
      });

      await waitFor(() => {
        expect(result.current.content).toBe('# 中文内容');
      });

      expect(global.fetch).toHaveBeenCalledWith(
        '/posts/hello-world.zh-CN.md',
        { cache: 'no-cache' }
      );
    });
  });
});
