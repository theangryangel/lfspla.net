# Contributing to lfspla.net

Thanks for taking an interest in lfspla.net. Small, clear changes are easiest
to review and maintain.

## Getting started

Follow [local development](docs/development.md) to set up the application.
The repository uses `prek` for formatting and checks:

```sh
prek install --prepare-hooks
prek run --all-files
```

Run the checks that cover your change before opening a pull request.

PostgreSQL regression tests are ignored by the ordinary test command. Run them
against a disposable PostgreSQL server with a role allowed to create databases:

```sh
DATABASE_URL=postgres://user:password@localhost/test cargo test --locked -p lfsplanet database_tests -- --ignored
```

SQLx creates and migrates a separate database for each test. CI runs these tests
against its own PostgreSQL service.

## Pull requests

Keep each pull request focused on one change. Explain what changed and why.
Link any issue it fixes. Include test steps, and add screenshots for visible
frontend changes.

Update the documentation when behaviour, setup, deployment, or user-facing
features change.

Add database changes as a new migration. Do not edit a migration that may have
already run elsewhere. Era files in `assets/eras/` are inputs: changing one
does not change a running site until it is applied.

Track outlines in `assets/tracks/` are generated and kept in Git. Regenerate
and commit them when the generator or an LFS track path file changes. Seed
the corresponding `kind: Location` documents in `assets/tracks/*.yaml` to publish
their nested track outlines as attachments. See [catalogue seeding](docs/catalogue.md). Vehicle
artwork in `assets/vehicles/` is added by hand; see the README there.
Neither is covered by the MIT License; see [NOTICE](NOTICE.md).

## Discussion and conduct

Be aware of the person behind the code. Be clear and kind when asking for
changes or giving feedback.

AI-assisted work is welcome. Say when you used it, understand the result, and
take responsibility for the change and any review replies.

## Bugs, features, and ranking rules

Use the [issue tracker](https://github.com/theangryangel/lfspla.net/issues)
for bugs, feature ideas, and ranking or era discussions. Bug reports should
include steps to reproduce the problem, expected and actual results, and logs
with secrets removed.
