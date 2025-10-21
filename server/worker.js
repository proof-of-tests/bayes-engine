// Cloudflare Workers entry point
// This file loads the WASM module and forwards fetch requests to it

import * as wasm from './server.js';

export default {
  async fetch(request, env, ctx) {
    // The WASM module exports a fetch function that we can call directly
    return await wasm.fetch(request, env, ctx);
  }
};
