import React, { useState, useEffect, type ReactNode } from 'react';
import type { SiteConfig } from './types/config';
import type { AlbumConfig } from './types/album-config';
import type { MemoConfig } from './types/memo-config';

export type RuntimeSiteConfig = SiteConfig;

/**
 * Error details for configuration loading failures
 */
export interface ConfigError {
  type: 'fetch' | 'parse' | 'validation';
  message: string;
  statusCode?: number;
  missingField?: string;
}

/**
 * Loading state for the RuntimeConfigLoader
 */
export interface LoadingState {
  status: 'loading' | 'error' | 'ready';
  error?: ConfigError;
}

/**
 * Props for the RuntimeConfigLoader component
 */
export interface RuntimeConfigLoaderProps {
  /** Path to the site configuration file (default: "/config.json") */
  configPath?: string;
  /** Path to the album configuration file (default: "/album.config.json") */
  albumConfigPath?: string;
  /** Path to the memo configuration file (default: "/memo.config.json") */
  memoConfigPath?: string;
  /** Render function that receives the loaded configurations */
  children: (siteConfig: RuntimeSiteConfig, albumConfig: AlbumConfig, memoConfig: MemoConfig) => ReactNode;
}

/** Required fields that must be present in config.json */
const REQUIRED_CONFIG_FIELDS: (keyof SiteConfig)[] = ['title', 'description', 'logo', 'favicon'];

/**
 * Validates that all required fields are present in the site configuration
 */
function validateSiteConfig(config: unknown): { valid: true; config: RuntimeSiteConfig } | { valid: false; missingField: string } {
  if (typeof config !== 'object' || config === null) {
    return { valid: false, missingField: 'config object' };
  }
  
  const configObj = config as Record<string, unknown>;
  
  for (const field of REQUIRED_CONFIG_FIELDS) {
    if (configObj[field] === undefined || configObj[field] === null || configObj[field] === '') {
      return { valid: false, missingField: field };
    }
  }
  
  return { valid: true, config: configObj as unknown as RuntimeSiteConfig };
}

/**
 * Validates that the album configuration has the required structure
 */
function validateAlbumConfig(config: unknown): { valid: true; config: AlbumConfig } | { valid: false; missingField: string } {
  if (typeof config !== 'object' || config === null) {
    return { valid: false, missingField: 'album config object' };
  }
  
  const configObj = config as Record<string, unknown>;
  
  if (typeof configObj.enabled !== 'boolean') {
    return { valid: false, missingField: 'enabled' };
  }
  
  if (!Array.isArray(configObj.albums)) {
    return { valid: false, missingField: 'albums' };
  }
  
  return { valid: true, config: configObj as unknown as AlbumConfig };
}

const DEFAULT_MEMO_CONFIG: MemoConfig = { enabled: false, provider: 'ech0', serverUrl: '' };

/**
 * Validates that the memo configuration has the required structure
 */
function validateMemoConfig(config: unknown): { valid: true; config: MemoConfig } | { valid: false; missingField: string } {
  if (typeof config !== 'object' || config === null) {
    return { valid: false, missingField: 'memo config object' };
  }
  const configObj = config as Record<string, unknown>;
  if (typeof configObj.enabled !== 'boolean') {
    return { valid: false, missingField: 'enabled' };
  }
  if (typeof configObj.provider !== 'string') {
    return { valid: false, missingField: 'provider' };
  }
  if (typeof configObj.serverUrl !== 'string') {
    return { valid: false, missingField: 'serverUrl' };
  }
  return { valid: true, config: configObj as unknown as MemoConfig };
}

/**
 * Fetches and parses a JSON configuration file
 */
async function fetchConfig<T>(
  path: string,
  validate: (config: unknown) => { valid: true; config: T } | { valid: false; missingField: string }
): Promise<{ success: true; data: T } | { success: false; error: ConfigError }> {
  try {
    const response = await fetch(path);
    
    if (!response.ok) {
      return {
        success: false,
        error: {
          type: 'fetch',
          message: `Failed to load ${path}: ${response.status} ${response.statusText}`,
          statusCode: response.status,
        },
      };
    }
    
    let data: unknown;
    try {
      data = await response.json();
    } catch (parseError) {
      return {
        success: false,
        error: {
          type: 'parse',
          message: `Failed to parse ${path}: ${parseError instanceof Error ? parseError.message : 'Invalid JSON'}`,
        },
      };
    }
    
    const validation = validate(data);
    if (!validation.valid) {
      return {
        success: false,
        error: {
          type: 'validation',
          message: `Missing required field: ${validation.missingField}`,
          missingField: validation.missingField,
        },
      };
    }
    
    return { success: true, data: validation.config };
  } catch (networkError) {
    return {
      success: false,
      error: {
        type: 'fetch',
        message: `Failed to load ${path}: ${networkError instanceof Error ? networkError.message : 'Network error'}`,
      },
    };
  }
}

/**
 * Error display component
 */
const ErrorDisplay: React.FC<{ error: ConfigError }> = ({ error }) => (
  <div className="flex items-center justify-center min-h-screen bg-gray-50 dark:bg-gray-900">
    <div className="max-w-md p-6 bg-white dark:bg-gray-800 rounded-lg shadow-lg">
      <div className="flex items-center mb-4">
        <svg className="w-6 h-6 text-red-500 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Configuration Error</h2>
      </div>
      <p className="text-gray-600 dark:text-gray-400 mb-2">{error.message}</p>
      {error.statusCode && (
        <p className="text-sm text-gray-500 dark:text-gray-500">HTTP Status: {error.statusCode}</p>
      )}
      {error.missingField && (
        <p className="text-sm text-gray-500 dark:text-gray-500">Missing field: <code className="bg-gray-100 dark:bg-gray-700 px-1 rounded">{error.missingField}</code></p>
      )}
      <p className="mt-4 text-sm text-gray-500 dark:text-gray-500">
        Please check your configuration files and try again.
      </p>
    </div>
  </div>
);

/**
 * RuntimeConfigLoader - Loads configuration files at runtime via fetch()
 * 
 * This component fetches `/config.json` and `/album.config.json` in parallel,
 * displays a loading indicator while fetching, and shows error messages for
 * HTTP errors, JSON parse errors, or missing required fields.
 * 
 * Supports `basePath` configuration for subdirectory deployment.
 * 
 * @example
 * ```tsx
 * <RuntimeConfigLoader>
 *   {(siteConfig, albumConfig) => (
 *     <SpageApp siteConfig={siteConfig} albumConfig={albumConfig} />
 *   )}
 * </RuntimeConfigLoader>
 * ```
 */
export const RuntimeConfigLoader: React.FC<RuntimeConfigLoaderProps> = ({
  configPath = '/config.json',
  albumConfigPath = '/album.config.json',
  memoConfigPath = '/memo.config.json',
  children,
}) => {
  const [state, setState] = useState<LoadingState>({ status: 'loading' });
  const [configs, setConfigs] = useState<{ siteConfig: RuntimeSiteConfig; albumConfig: AlbumConfig; memoConfig: MemoConfig } | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadConfigs() {
      setState({ status: 'loading' });

      // Fetch configuration files in parallel (memo config is optional — 404 yields default)
      const [siteResult, albumResult, memoResult] = await Promise.all([
        fetchConfig<RuntimeSiteConfig>(configPath, validateSiteConfig),
        fetchConfig<AlbumConfig>(albumConfigPath, validateAlbumConfig),
        fetchConfig<MemoConfig>(memoConfigPath, validateMemoConfig),
      ]);

      if (cancelled) return;

      // Check for errors in site config
      if (!siteResult.success) {
        setState({ status: 'error', error: siteResult.error });
        return;
      }

      // Check for errors in album config
      if (!albumResult.success) {
        setState({ status: 'error', error: albumResult.error });
        return;
      }

      // Memo config is optional — use default if fetch/parse/validation fails
      const memoConfig = memoResult.success ? memoResult.data : DEFAULT_MEMO_CONFIG;

      // All configs loaded successfully
      setConfigs({
        siteConfig: siteResult.data,
        albumConfig: albumResult.data,
        memoConfig,
      });
      setState({ status: 'ready' });
    }

    loadConfigs();

    return () => {
      cancelled = true;
    };
  }, [configPath, albumConfigPath, memoConfigPath]);

  // During loading, return null — the HTML skeleton in #root remains visible
  if (state.status === 'loading') {
    return null;
  }

  // Show error message
  if (state.status === 'error' && state.error) {
    return <ErrorDisplay error={state.error} />;
  }

  // Render children with loaded configs
  if (state.status === 'ready' && configs) {
    return <>{children(configs.siteConfig, configs.albumConfig, configs.memoConfig)}</>;
  }

  // Fallback (should not reach here)
  return null;
};

export default RuntimeConfigLoader;
