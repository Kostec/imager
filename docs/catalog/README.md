# Example external catalog

Wukong Imager always has its built-in catalog (the Wukong Pi Zero, with no images). An external
catalog adds manufacturers, boards and images to it. Point the app at one in
**Settings > Downloads and storage > Catalog source**.

## From a URL (nginx in Docker)

```
docker compose up -d
```

Then enter `http://localhost:8080` in Catalog source, press **Check**, then **Save**. The app
fetches `http://localhost:8080/catalog.json`. A URL that already ends in `.json` is used as is.
Relative `url` values in the file resolve against the catalog's own address.

## From a folder

Enter the path of the `folder/` directory next to this file (or of its `catalog.json`). Images are
named with `file`, relative to the folder; `..` and absolute paths are refused.

## catalog.json, version 1

| Field | Notes |
|---|---|
| `version` | must be `1` |
| `vendors[]` | `slug`, `name` |
| `boards[]` | `slug`, `name`, `vendor` (a slug from `vendors` or the built-in `wukong`), optional `soc`, `architecture`, `summary`, `image` |
| `images[]` | `board`, `name`, optional `release`, `kernel`, `size`, `supports_settings`; exactly one of `url` (absolute, or relative to the catalog URL) and `file` (folder catalogs only) |
| `images[].sha256` | 64 hex characters, of the file as served (the compressed file for `.xz`). Without it the image is not verified and the check shows a warning |

When several manufacturers (or several boards) are on offer, the app shows the manufacturer and
board steps; with one it goes straight to the image. If the file is broken or the URL is down, the
app says where and why, and keeps working with the built-in catalog.

`images/demo.img` is a 59-byte text file, not a bootable image: it only lets you try the flow.
