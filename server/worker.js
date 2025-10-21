// Cloudflare Workers entry point
// This file loads the WASM module and forwards fetch requests to it

import init, { fetch as wasmFetch } from './server.js';
import wasm from './server_bg.wasm';

let initialized = false;

export default {
  async fetch(request, env, ctx) {
    // Initialize WASM on first request
    if (!initialized) {
      await init(wasm);
      initialized = true;
    }

    // Call the WASM fetch function
    return await wasmFetch(request, env, ctx);
  }
};
