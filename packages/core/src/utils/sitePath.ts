export function resolveSitePath(path: string, basePath?: string): string {
  if (/^[a-z][a-z\d+.-]*:/i.test(path) || path.startsWith('//') || path.startsWith('#')) {
    return path;
  }

  const base = basePath?.trim().replace(/^\/*|\/*$/g, '') ?? '';
  const localPath = path.startsWith('/') ? path : `/${path}`;
  if (!base) return localPath;

  const prefix = `/${base}`;
  return localPath === prefix || localPath.startsWith(`${prefix}/`)
    ? localPath
    : `${prefix}${localPath}`;
}
