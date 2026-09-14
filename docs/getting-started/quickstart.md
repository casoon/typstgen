---
title: Quickstart
description: Compile the example letter from the repository, then set up the same layout in your own project.
order: 2
---

## Compile the example

The repository contains the documents from the [showcase](../../../showcase/) together with their
config:

```sh
git clone https://github.com/casoon/typstgen
cd typstgen/examples
typstgen compile letter.typ --config typstgen.toml
```

typstgen prints the path of the PDF it wrote:

```text
letter.pdf
```

`letter.typ` imports `letterhead.typ`, which is not in the same folder but in `templates/`.
typstgen finds it because `typstgen.toml` names that folder:

```toml
template_paths = ["templates"]
use_system_fonts = false
```

## Your own project

1. Put shared templates into a folder, for example `templates/`.
2. Add a `typstgen.toml` next to where you run typstgen:

   ```toml
   template_paths = ["templates"]
   ```

3. Import templates by file name and compile:

   ```typst
   #import "letterhead.typ": letterhead
   ```

   ```sh
   typstgen compile letter.typ
   ```

Without `--output`, the PDF gets the input's name with a `.pdf` extension, in the same folder.
`./typstgen.toml` is picked up automatically; use `--config` for a file elsewhere.

Paths in `typstgen.toml` are relative to the working directory, and at least one template path must
exist, even for documents without imports. The [templates guide](../../guides/templates/) explains
the lookup in detail.
