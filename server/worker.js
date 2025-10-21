// Cloudflare Workers entry point
// This file loads the WASM module and forwards fetch requests to it

import { fetch as wasmFetch } from './server.js';

export default {
  async fetch(request, env, ctx) {
    // Call the WASM fetch function
    return await wasmFetch(request, env, ctx);
  }
};
