# petpet-rs

![Rust](https://img.shields.io/badge/Rust-1.93%2B-orange?style=for-the-badge&logo=rust&logoColor=white&labelColor=1A222E&color=242B36)
![GitHub License](https://img.shields.io/github/license/UWblNSKPRIME/petpet-rs?style=for-the-badge&labelColor=1A222E&color=242B36)
![GitHub repo size](https://img.shields.io/github/repo-size/UWblNSKPRIME/petpet-rs?style=for-the-badge&logo=github&logoColor=white&labelColor=1A222E&color=242B36&cacheSeconds=0)
![Cloudflare Workers](https://img.shields.io/badge/Cloudflare_Workers-Free_Tier-F38020?style=for-the-badge&logo=cloudflare&logoColor=white&labelColor=1A222E&color=242B36)

A web service for generating `128x128` Animated WebP (`24-bit + Alpha`) petpet animations based on a Discord user's avatar, written in pure Rust.

When a user updates their avatar in Discord, the service automatically detects the new avatar hash and replaces the old cached file.

## Usage

### Discord avatar

<kbd>GET</kbd> `/discord/{user_id}` or `/ds/{user_id}`

The response is a `128x128` Animated WebP (`24-bit RGB + 8-bit Alpha`). You can also append `.webp` to the `{user_id}` path segment, e.g. `/discord/{user_id}.webp` or `/ds/{user_id}.webp`.

#### Path parameters

| Name        | Type      | Description           |
|-------------|-----------|-----------------------|
| `{user_id}` | Snowflake | The Discord user's ID |

#### Query parameters

| Name       | Default | Type             | Description                                        |
|------------|---------|------------------|----------------------------------------------------|
| `delay`    | `3`     | Unsigned Integer | Animation speed. Bigger is slower                  |
| `no-cache` | `false` | Boolean          | Whether to disable caching (Cache-Control headers) |

### Custom upload

<kbd>POST</kbd> `/custom` or `/c` (WebP)

<kbd>POST</kbd> `/custom/webp` or `/c/webp` (WebP)

Send a `multipart/form-data` request with a file field named `image` containing a PNG, JPEG or WebP.

The upload is limited to 5MB and a maximum of 1 MP (you can reconfigure it).

## Getting started

### Cloudflare Workers (Free Tier)

You can deploy `petpet-rs` directly to Cloudflare Workers (Free Tier) using [`wrangler.toml`](wrangler.toml):

```bash
npx wrangler login
npx wrangler deploy
```

*(Optional)* Set your Discord `BOT_TOKEN` secret (if omitted, `petpet-rs` automatically uses a public Discord user lookup fallback):

```bash
npx wrangler secret put BOT_TOKEN
```

### Standalone Server / Docker

* Build from source: `cargo build --release`
* Docker: `docker build -t petpet-rs . && docker run -p 3000:3000 petpet-rs`

> [!NOTE]
> The Discord bot sends preview requests from the `us-east1` GCP region (South Carolina, US). To reduce network latency, choose server locations close to it. If using a Cloudflare proxy or Cloudflare Workers, the nearest Cloudflare location is ATL (Atlanta, US).

## Configuration

Currently, config parameters can be specified either in the `config.yaml` file or via environment variables.

Outbound HTTP proxy settings are handled by standard environment variables: `HTTP_PROXY`, `HTTPS_PROXY`, and `NO_PROXY`.

Look at the [configuration reference](config.sample.yaml) with comments (including environment variable names).

## PetPet in other languages

* **Go**: [wavy-cat/petpet-go](https://github.com/wavy-cat/petpet-go)
* **Python**: [nakidai/petthecord](https://github.com/nakidai/petthecord)
* **Rust**: [mitsuki-kagamin/petpet-api](https://github.com/mitsuki-kagamin/petpet-api)
* **C**: [nakidai/cptc](https://github.com/nakidai/cptc)
