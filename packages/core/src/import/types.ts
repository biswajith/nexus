import type { NexusCollection, NexusEnvironment } from '../types/index.js';

export interface ImportResult {
  collections: NexusCollection[];
  environments: NexusEnvironment[];
  warnings: string[];
}

export type ImportFormat = 'postman-collection-v2.1' | 'postman-environment' | 'openapi-3' | 'swagger-2' | 'curl' | 'nexus';

export function detectImportFormat(data: string): ImportFormat {
  const trimmed = data.trim();

  // Try JSON
  if (trimmed.startsWith('{')) {
    try {
      const parsed = JSON.parse(trimmed);

      // Postman Collection v2.1
      if (parsed.info?.schema?.includes('collection.json') || parsed.info?._postman_id) {
        return 'postman-collection-v2.1';
      }

      // Postman Environment
      if (parsed._postman_variable_scope === 'environment' || (parsed.name && parsed.values && Array.isArray(parsed.values))) {
        return 'postman-environment';
      }

      // OpenAPI 3.x
      if (parsed.openapi && typeof parsed.openapi === 'string' && parsed.openapi.startsWith('3')) {
        return 'openapi-3';
      }

      // Swagger 2.0
      if (parsed.swagger && parsed.swagger === '2.0') {
        return 'swagger-2';
      }

      // Nexus native
      if (parsed.id && parsed.items && parsed.version) {
        return 'nexus';
      }
    } catch {
      // Not valid JSON, check for YAML
    }
  }

  // YAML (OpenAPI/Swagger often in YAML)
  if (trimmed.startsWith('openapi:') || trimmed.includes('openapi: ')) {
    return 'openapi-3';
  }
  if (trimmed.startsWith('swagger:') || trimmed.includes('swagger: ')) {
    return 'swagger-2';
  }

  // cURL
  if (trimmed.startsWith('curl ') || trimmed.startsWith('curl\t')) {
    return 'curl';
  }

  return 'nexus';
}
