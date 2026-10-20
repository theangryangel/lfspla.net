# Eras and rankings

An era defines a range of LFS versions, an LFS installation, and rankings.
A track and vehicle pair is eligible only when at least one ranking in that
era selects it.

Era definitions are versioned YAML documents under `assets/eras/`, but PostgreSQL
is the source of truth at runtime.

Think of the eras files as desired state, or seeding/fixture data.

## Applying definitions

```sh
lfsplanet seed assets/eras/*.yaml --dry-run
lfsplanet seed assets/eras/*.yaml
lfsplanet era list
lfsplanet era export 2007-12-21 > era.yaml
lfsplanet seed era.yaml --yes
lfsplanet era open 2007-12-21
lfsplanet era close 2007-12-21
lfsplanet era delete 2007-12-21 --yes
```

Era documents declare `kind: Era`. See [catalogue seeding](catalogue.md) for
input formats, validation, and dry runs. `era export` emits tagged YAML suitable
for `seed`.

Eras omitted from the input are left untouched. Rankings and combinations inside
a supplied era retain their replacement rules. Changing an existing era requires
`--yes` and may delete laps outside its new combinations, as reported in the
change summary. Referenced LFS installations and canonical tracks and vehicles
must already exist; location definitions with nested tracks and vehicle definitions can be seeded
in the same batch.

## Completion badges

A ranking can award a completion badge in addition to its usual ranking badge:

```yaml
badge:
  id: nutter
  qualification:
    type: ranked
    limit: 3
  completion:
    id: gabor
```

The top three keep their Nutter badges. Every player with a published result on
all charts selected by that era's Nutter ranking also receives Gabor, regardless
of position or benchmark handicap. Players can hold both badges. The `id` fields reference [catalogue badges](catalogue.md#badges); artwork and names
come from those definitions. Seed the badge definitions before applying an era.

Apply the updated era definitions to activate this configuration; applying an
updated definition also rebuilds its stored player badges.
