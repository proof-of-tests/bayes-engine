use worker::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    // All static assets (HTML, CSS, WASM) are served automatically by CloudFlare
    // from the assets directory configured in wrangler.toml.
    //
    // This Worker only handles dynamic API routes.
    // For now, we don't have any API routes, so we just pass through to assets.

    // Fetch from assets (binding name is "ASSETS" by default)
    env.assets("ASSETS")?.fetch_request(req).await
}
