# 理财产品分类和多语言栏目配置

## Goal

Allow admins to configure Earn product category columns, including multilingual category names, and assign Earn products to those configured categories instead of the current hard-coded category labels.

## What I Already Know

* Earn products already store a `category` code on `earn_products`.
* The admin Earn product create/edit SideSheet currently uses hard-coded category labels and descriptions.
* Earn product introductions already use country-driven default locales, and that pattern should be reused for category multilingual names.
* Admin resource pages are configured through `resourceConfigs.tsx`, with row actions and create SideSheets in `ResourceCreateActions.tsx`.

## Requirements

* Add an `earn_product_categories` table with category `code`, multilingual `name_json`, `sort_order`, `status`, and timestamps.
* Seed the existing built-in category codes and backfill any existing product category codes into the new category table.
* Add admin APIs to list, create, view, update, and enable/disable category columns.
* Product create/update should assign products by category code and validate the category exists and is active.
* Earn product list/detail responses should include the configured category display name and multilingual JSON so admin and PC can display configured category names.
* Add a backend admin page for category columns under the Earn navigation.
* The product create/edit form should load active category options from the new category API.
* Category create/edit SideSheets should support multilingual category names by country, using each country's default locale.

## Acceptance Criteria

* [ ] Admin can add a category column with multilingual names.
* [ ] Admin can edit category multilingual names, sort order, and status.
* [ ] Admin can disable/enable a category column.
* [ ] Earn product create/edit uses a searchable configured category dropdown.
* [ ] Earn product tables display configured category names instead of only hard-coded labels.
* [ ] Unknown or disabled category codes cannot be assigned to new/updated products.
* [ ] Existing fixed_term/flexible/structured/staking products keep working after migration.

## Definition of Done

* Backend route tests cover category CRUD/status and product category assignment.
* Admin resource tests cover category page and configured category dropdown in Earn product forms.
* `cargo fmt`, focused backend tests, web typecheck, focused web tests, and `git diff --check` pass.
* `docs/superpowers/PROGRESS.md` is updated.

## Out of Scope

* Deleting categories.
* Reworking the PC Finance page layout into category tabs in this task.
* Migrating `earn_products.category` to a numeric foreign key.

## Technical Notes

* Keep `earn_products.category` as a stable code for backward compatibility.
* Do not add a database foreign key on `earn_products.category` because historical rows may contain custom category codes.
* Category multilingual JSON shape:

```json
{
  "version": 1,
  "default_locale": "zh-CN",
  "items": [
    { "locale": "zh-CN", "country": "CN", "title": "定期" }
  ]
}
```
