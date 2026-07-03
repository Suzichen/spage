import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  useParams: () => ({ slug: 'test-post' }),
  useLocation: () => ({ hash: '', key: 'default' }),
  useNavigationType: () => 'PUSH',
  useNavigate: () => vi.fn(),
  Link: ({ children, to }: { children: React.ReactNode; to: string }) =>
    React.createElement('a', { href: to }, children),
}));

// Mock react-i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'post.noLocalizedVersion': 'This article is not available in your language.',
        'common.loading': 'Loading...',
        'common.postNotFound': 'Post not found',
        'common.prevPost': 'Previous Post',
        'common.nextPost': 'Next Post',
        'common.tags': 'Tags',
      };
      return translations[key] ?? key;
    },
    i18n: { resolvedLanguage: 'en' },
  }),
}));

// Mock usePost hook - control isFallback value
const mockUsePost = vi.fn();
vi.mock('@/hooks/usePost', () => ({
  usePost: (...args: unknown[]) => mockUsePost(...args),
}));

// Mock usePosts hook - PostDetail uses it for relatedPosts
vi.mock('@/hooks/usePosts', () => ({
  usePosts: () => ({ posts: [], loading: false, error: null }),
}));

// Mock Prism to avoid side effects
vi.mock('prismjs', () => ({
  default: { highlightAll: vi.fn() },
}));
vi.mock('prismjs/themes/prism.css', () => ({}));

// Mock react-markdown
vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) =>
    React.createElement('div', { 'data-testid': 'markdown' }, children),
}));

// Mock remark/rehype plugins
vi.mock('remark-gfm', () => ({ default: () => {} }));
vi.mock('rehype-slug', () => ({ default: () => {} }));

// Mock TableOfContents
vi.mock('@/components/TableOfContents', () => ({
  default: () => React.createElement('div', { 'data-testid': 'toc' }),
}));

// Mock date-fns
vi.mock('date-fns', () => ({
  format: () => 'January 01, 2025',
}));

import PostDetail from '../PostDetail';

describe('PostDetail - Language fallback notice bar', () => {
  /**
   * Validates: Requirement 4.5
   * When currentLang === siteDefaultLanguage, the notice bar should NOT be shown
   * regardless of whether the post has a localized version.
   */
  describe('currentLang === siteDefaultLanguage', () => {
    it('does not show notice bar when user language matches site default', () => {
      mockUsePost.mockReturnValue({
        post: {
          slug: 'test-post',
          title: 'Test Post',
          date: '2025-01-01T00:00:00',
          tags: ['test'],
          categories: [],
          summary: 'A test post',
          availableLanguages: [],
        },
        content: '# Test content',
        loading: false,
        error: null,
        isFallback: false, // currentLang === siteDefaultLanguage → isFallback is false
        prevPost: undefined,
        nextPost: undefined,
      });

      render(React.createElement(PostDetail));

      expect(screen.queryByText('This article is not available in your language.')).not.toBeInTheDocument();
    });

    it('does not show notice bar even when post has localized versions but user is on default lang', () => {
      mockUsePost.mockReturnValue({
        post: {
          slug: 'test-post',
          title: 'Test Post',
          date: '2025-01-01T00:00:00',
          tags: ['test'],
          categories: [],
          summary: 'A test post',
          availableLanguages: ['zh-CN', 'ja'],
        },
        content: '# Test content',
        loading: false,
        error: null,
        isFallback: false, // currentLang === siteDefaultLanguage → isFallback is false
        prevPost: undefined,
        nextPost: undefined,
      });

      render(React.createElement(PostDetail));

      expect(screen.queryByText('This article is not available in your language.')).not.toBeInTheDocument();
    });
  });

  /**
   * Validates: Requirement 4.5
   * When currentLang !== siteDefaultLanguage AND the post has a localized version
   * for that language (loaded successfully), the notice bar should NOT be shown.
   */
  describe('currentLang !== siteDefaultLanguage, post has localized version', () => {
    it('does not show notice bar when localized version is available and loaded', () => {
      mockUsePost.mockReturnValue({
        post: {
          slug: 'test-post',
          title: 'テスト記事',
          date: '2025-01-01T00:00:00',
          tags: ['test'],
          categories: [],
          summary: 'テスト',
          availableLanguages: ['ja'],
        },
        content: '# テスト内容',
        loading: false,
        error: null,
        isFallback: false, // localized version loaded successfully → isFallback is false
        prevPost: undefined,
        nextPost: undefined,
      });

      render(React.createElement(PostDetail));

      expect(screen.queryByText('This article is not available in your language.')).not.toBeInTheDocument();
    });
  });

  /**
   * Validates: Requirement 4.5
   * When currentLang !== siteDefaultLanguage AND the post does NOT have a localized
   * version for that language, the notice bar SHOULD be shown.
   */
  describe('currentLang !== siteDefaultLanguage, post has NO localized version', () => {
    it('shows notice bar when no localized version exists for user language', () => {
      mockUsePost.mockReturnValue({
        post: {
          slug: 'test-post',
          title: 'Test Post',
          date: '2025-01-01T00:00:00',
          tags: ['test'],
          categories: [],
          summary: 'A test post',
          availableLanguages: [],
        },
        content: '# Test content',
        loading: false,
        error: null,
        isFallback: true, // no localized version AND not default lang → isFallback is true
        prevPost: undefined,
        nextPost: undefined,
      });

      render(React.createElement(PostDetail));

      expect(screen.getByText('This article is not available in your language.')).toBeInTheDocument();
    });
  });
});
