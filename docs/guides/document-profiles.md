---
title: Document profiles
description: Resolve package-defined profiles and validate explicit invoice prerequisites using the CLI, Rust library or MCP.
order: 8
---

The profiles API reads a versioned catalog supplied by a template package;
there is no hardcoded country selection in the compiler. The coordinated business-templates
package supplies `profiles/catalog.json` and its schemas/fixtures. Basic compilation remains
independent of the optional `zugferd` feature, which enables these invoice profile commands.

```sh
typstgen list-document-profiles --catalog path/to/profiles/catalog.json
typstgen resolve-document-profile --catalog path/to/profiles/catalog.json selection.json
typstgen validate-document --catalog path/to/profiles/catalog.json request.json
```

Selection is a JSON object with `profile_id`, `profile_version`, `language`, `format_locale`,
`currency`, `layout`, `output_profile` and optional `allow_draft`. A request contains `selection`,
`data` (business-templates invoice JSON), `company` and independently established `facts`.
Unknown identities, unsupported choices, abstract profiles, rule conflicts and cycles are blocked.
Required fields merge by union; replacing an inherited rule requires `replace_rules` explicitly.
Language never changes the selected rule jurisdiction. Catalog shape is checked by the package
JSON Schema workflow; typstgen reports JSON Schema validation as `not_run`.

The library's `typstgen::profiles` module exposes the same three functions over
`serde_json::Value`. MCP exposes `list_document_profiles`, `resolve_document_profile`,
`validate_document`, each accepting a workspace-relative `catalog` path and a JSON `selection`
or `request` where needed. Catalog reads respect the existing workspace path confinement.
The existing `check` tool still checks compilation, while `preview`/`compile` reuse the normal
rendering path. No second server, automatic file generation or transmission is involved.

Validation returns concrete field paths, rule IDs and catalog source records, a canonical model
and an established-template rendering view (`data`/`company`) only when business checks pass.
Both CII and that view originate from the existing exact-decimal invoice model. Dates are ISO,
amounts/rates/quantities are decimal strings, and zero-rate categories remain distinct.
The persisted profile selection and caller evidence are part of the reproducible input.

`resolved`/`blocked` describe selection; `ready_for_preview`/`blocked` describe validation.
Individual check states cover schema, business rules, arithmetic, EN16931, output profile,
PDF/A and transmission. External checks remain `not_run` until actually executed; neither a
normalized invoice nor a compiled PDF is a full legal validation or delivery receipt.
The existing `--zugferd` path uses CII EN16931; use exactly the same canonical data as the PDF,
then validate the hybrid artifact externally. Profile resolution does not promise an
unimplemented XRechnung, Peppol or national transmission adapter.

The cross-repository synthetic integration check is
`typst-business-templates/scripts/check-profiles.py --typstgen /path/to/typstgen`, optionally
with Mustang and veraPDF. Its generated report records external versions, hashes, notices and
skipped stages. See the package's invoice-profile guide for the supported bounded cases.

## User-owned invoice configuration

For configurable tax rates and presentation without a national catalog, use:

```sh
typstgen prepare-invoice --config invoice-config.json invoice.json > resolved.json
```

The configuration declares version `1`, `presentation` (language, format_locale, currency,
layout), named `taxes` with category/rate/reason, a `default_tax`, optional `required_fields`,
`notes` and `display_fields`. Each invoice item chooses a `tax` key or uses the default.
The input is `{company, data, facts?}`; totals may be omitted and are calculated with the
existing exact-decimal invoice model. Supplied totals and explicit currencies are checked.
The returned rendering view supplies the same canonical values as CII export.

The Rust function is `typstgen::invoice_config::prepare_invoice(config, request)`;
MCP exposes `prepare_invoice` with a workspace-relative `config` path and a `request` object.
A blocked report is unsuccessful at the CLI. Configuration provenance is returned, with
`checks.jurisdiction = "not_assessed"`: supplied rates do not imply reviewed national rules.
Amounts use four-place input precision and round half away from zero to two decimal places;
no currency conversion, gross-price calculation or document allowances are added.

[Configuration schema, editable default and complete example](https://github.com/casoon/typst-business-templates/blob/main/docs/guides/invoice-configuration.md)
live in the template package repository.

Configurations also accept `tax_model: "sales-tax"` with named additive components instead
of VAT categories. Each component has an ID, label and percentage; its taxable basis and
rounded amount are calculated separately. These invoices return PDF rendering data and
`en16931: not_applicable`; the CII adapter rejects sales-tax output. VAT remains the default.
US presentation supports `en-US`, `paper: "us-letter"`, USD dollar prefixes, US date formatting
and optional state/routing/account fields. See the package's configuration guide and its
`examples/invoice-languages/` gallery for all seven languages and the additional US variant.
