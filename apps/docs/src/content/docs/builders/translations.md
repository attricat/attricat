---
title: Translate labels
description: Show blueprint names, attribute names, and view labels in each user's language with the workspace lexicon.
---

Blueprint labels are plain text by default and display exactly as written. To translate a label, wrap it in double braces. The text inside the braces is looked up in the workspace **lexicon**, a set of translations that lives outside blueprints:

```toml
name = "{{Product}}"

[[attributes]]
code = "price"
name = "{{Price}}"
value_type = "number"

[[views.detail.tabs]]
label = "{{Overview}}"
```

A user whose interface is set to Polish sees the Polish entry for `Product`. If there is none, they see the English entry, and if that is missing too, the text inside the braces. An untranslated label therefore still reads correctly in English.

Because the lexicon is separate from blueprints, you can fix a translation or add a language without publishing a new blueprint revision. Translations follow the user's interface language. They do not depend on the selected [context](/guides/contexts/), which chooses catalog values, not labels.

## Which labels are translated

| Label | Where |
| --- | --- |
| Blueprint name | `name` |
| Attribute name | `[[attributes]] name` on an inline attribute |
| Reusable attribute name | `name` in its definition |
| Tab and accordion section label | `label` in `tabs` and `accordion` blocks |
| Table column heading | `label` in `[views.table] columns` |
| Incoming relationship button and dialog title | `label` in `incoming_relationship_list` |

Other text, such as `heading` and `text` blocks, display separators, and catalog values, is always shown as written.

## Reference syntax

| Write | Meaning |
| --- | --- |
| `{{Product}}` | Translate `Product`. |
| `{{Order\|purchase}}` | Translate `Order` in the `purchase` sense. The part after `\|` is a context and is never displayed. |
| `SKU {{Price}}` | Literal text around a reference is kept as is. |
| `\{{` | A literal `{{`. |
| `Brand™` | No braces: shown as written, never translated. |

- **Keys are English text.** The text inside the braces is both the lookup key and the English fallback. Leading, trailing, and repeated spaces are ignored, but case matters: `Product` and `product` are separate keys.
- **Shared terms are translated once.** Every blueprint that uses `{{Price}}` shows the same translation.
- **Use a context for homonyms and grammar.** `{{Order|purchase}}` and `{{Order|sorting}}` are separate entries, as are words that need a different grammatical gender in another language. A reference with a context never falls back to the entry without one.
- **Wrap the whole phrase.** Write `"{{Products in this category}}"`, not `"{{Products}} in this category"`. Word order, case, and gender differ between languages, so translators need the full phrase.

Saving a blueprint fails if a translated label contains a malformed reference: an unclosed `{{`, empty braces, an empty context, more than one `|`, or a brace inside a reference.

## Manage translations

Translations are workspace data. Anyone who can read the catalog loads them; changing them requires `blueprints.write`.

```sh
acli lexicon set --key Product --language pl --text Produkt
acli lexicon set --key Order --context purchase --language pl --text Zamówienie
acli lexicon list --language pl
acli lexicon delete --key Order --context purchase --language pl
```

To work on a whole language at once, export it, edit the file, and import it:

```sh
acli lexicon export --language pl > pl.json
acli lexicon import --file pl.json            # add and update entries
acli lexicon import --file pl.json --replace  # also delete entries missing from the file
```

Import files can also be written in TOML (use a `.toml` extension):

```toml
format_version = 1
language = "pl"

[[entries]]
key = "Product"
text = "Produkt"

[[entries]]
key = "Order"
context = "purchase"
text = "Zamówienie"
```

Languages are BCP 47 tags such as `pl`, `de`, or `pt-BR`. See the [CLI reference](/reference/cli/#translations) for every option. The web app currently offers English and Polish as interface languages.

## Change English wording

An English (`en`) entry replaces the displayed English text without changing the key, so existing translations stay attached:

```sh
acli lexicon set --key Product --language en --text Item
```

Use this to rename a label or fix a typo. Change the key in the blueprint only when the meaning changes, and then translate the new key.

## Plural forms

Labels without a number, such as tab labels and column headings, use separate keys for singular and plural wording: `{{Product}}` and `{{Products}}`.

Where the app shows a number of entities, such as the result total in the Explorer, it can use the blueprint name in the correct form for that number: "1 Produkt", "3 Produkty", "5 Produktów". For this, give the blueprint name's entry one form per plural category of the language:

```sh
acli lexicon set --key Product --language en --plural-category one --text Product
acli lexicon set --key Product --language en --plural-category other --text Products
acli lexicon set --key Product --language pl --plural-category one --text Produkt
acli lexicon set --key Product --language pl --plural-category few --text Produkty
acli lexicon set --key Product --language pl --plural-category many --text Produktów
acli lexicon set --key Product --language pl --plural-category other --text Produktu
```

| Language | Plural categories |
| --- | --- |
| English, German, Dutch, Swedish | `one`, `other` |
| French, Spanish, Italian, Portuguese | `one`, `many`, `other` |
| Polish, Czech, Slovak, Ukrainian, Russian | `one`, `few`, `many`, `other` |
| Japanese, Chinese, Korean | `other` |

An entry without a plural category uses `other`. A form contains only the noun; the app places the number. When an entry has plural forms, labels without a number use its `one` form, so write the forms with the capitalization your labels use. Until a blueprint name has plural forms, the app keeps its generic wording, such as "5 results".

## Find missing and unused translations

```sh
acli lexicon report
acli lexicon report --language pl --language de
```

The report lists, per language:

- **Untranslated** references used by catalog labels that have no entry in that language. English is never listed here because the key is the English text.
- **Missing plural categories** for blueprint names (which the app shows with counts) and for any entry that already has plural forms. For English, this is where missing `one` and `other` forms appear.

It also lists **orphaned** entries whose key and context no longer appear in any blueprint revision or reusable attribute. Review them before deleting: older revisions that entities still use count as in use.

## Translations from solution packs

A [solution pack](/builders/solution-packs/) can ship translations. Applying the pack adds its entries to the lexicon. Entries the workspace has written, before or after installation, always take precedence: a pack never overwrites them, and editing an entry the pack supplied makes it a workspace entry. A newer release of the pack can update the entries it supplied that nobody has edited.
