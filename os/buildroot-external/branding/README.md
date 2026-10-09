# Branding assets baked into the image

| File | Source | Use |
|---|---|---|
| `boot-logo.png` | `redrob-design-system/assets/Symbol/redrob-symbol-solid-white.png` (flat white, the variant the spec assigns to dark grounds), resized to 72 px on a 96x96 black ground, nothing recoloured | `BR2_LINUX_KERNEL_CUSTOM_LOGO_PATH`: Buildroot converts it to the kernel's 224-colour ppm; fbcon shows it top-left at boot (`CONFIG_LOGO`) |

Regenerate: `magick <source> -resize 72x72 -background black -gravity center -extent 96x96 -alpha remove -alpha off boot-logo.png`.
