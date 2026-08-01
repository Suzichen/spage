import { describe, expect, it } from 'vitest';
import { resolveSitePath } from './sitePath';

describe('resolveSitePath', () => {
  it('prefixes local resources with a normalized base path', () => {
    expect(resolveSitePath('/generated/manifest.json', '/blog/')).toBe('/blog/generated/manifest.json');
    expect(resolveSitePath('posts/hello.md', 'blog')).toBe('/blog/posts/hello.md');
    expect(resolveSitePath('/blog/logo.svg', '/blog')).toBe('/blog/logo.svg');
    expect(resolveSitePath('/logo.svg', '/')).toBe('/logo.svg');
  });

  it('leaves external and special URLs unchanged', () => {
    expect(resolveSitePath('https://cdn.example/logo.png', '/blog')).toBe('https://cdn.example/logo.png');
    expect(resolveSitePath('//cdn.example/logo.png', '/blog')).toBe('//cdn.example/logo.png');
    expect(resolveSitePath('data:image/png;base64,abc', '/blog')).toBe('data:image/png;base64,abc');
  });
});
