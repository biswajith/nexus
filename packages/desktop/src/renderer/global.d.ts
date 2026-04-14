import type { NexusApi } from '../../preload/index.js';

declare global {
  interface Window {
    nexus: NexusApi;
  }
}
