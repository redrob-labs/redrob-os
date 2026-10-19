# Brand sources

Verbatim copies of the Redrob Group Design System's shipped files (`redrob-design-system/assets/`)
and the `@redrob-labs/ui` token export. `SHA256SUMS` pins them; `tools/branding/make_assets.py`
verifies the pins and derives everything under `image/branding/` from these and nothing else.

| Files | From | Used for |
|---|---|---|
| `redrob-logo-primary-white.png` | `Logos/` -- gradient symbol, white wordmark | Plymouth splash, Anaconda sidebar (dark grounds) |
| `redrob-logo-primary.png` | `Logos/` -- gradient symbol, black wordmark | light grounds (first-login pages) |
| `redrob-logo-primary-solid-mono-white.png` | `Logos/` -- flat white | anything on a gradient (`10-logo.md`) |
| `redrob-symbol-solid-white.png` | `Symbol/` -- flat white symbol, 320 px | `system-logo-white.png` |
| `symbol-white-{32,64,192,512}.png` | `Favicons/` -- symbol alone on transparent | dark grounds that are not Redrob Blue |
| `favicon-{16,32,48,64}x*.png`, `icon-{192,512}.png`, `favicon.svg` | `Favicons/` -- white symbol on a Redrob Blue tile, drawn per size | the `redrob-os` hicolor icon |
| `tokens.json` | `@redrob-labs/ui` 1.7.0, `dist/styles/tokens.json` | every colour in the overlay |

Rules that bind here (`10-logo.md`, `Logos/README.md`, `Favicons/README.md`, `20-color.md`):

- Never redrawn, recoloured, re-proportioned, given a shadow, border or second radius.
- Never enlarged. A shipped size is used as is; any other size is the next larger file
  resized down (`make_assets.py` refuses to upscale).
- The lockup variant is picked by ground, never tinted to fit one. On a gradient, the
  solid mono white file.
- The tile is `blue-6` `#2B52FF`, never another colour; no dark-mode variant.
- Redrob OS is group-tier material: Redrob grounds and the Redrob lockup, no product colour.

To update: copy the new export over the old file, regenerate `SHA256SUMS`
(`sha256sum * | grep -v SHA256SUMS | sort -k2 > SHA256SUMS`), rerun `make_assets.py`, commit all three.
