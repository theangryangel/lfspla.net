# Eras and rankings

An era defines a range of LFS versions, an LFS installation, and rankings.
A track and vehicle pair is eligible only when at least one ranking in that
era selects it.

Era definitions are versioned YAML documents under `assets/eras/`, but PostgreSQL
is the source of truth at runtime.

Think of the eras files as desired state, or seeding/fixture data.

## Applying definitions

```sh
lfsplanet era apply assets/eras/*.yaml
lfsplanet era list
lfsplanet era export 2007-12-21 > era.yaml
lfsplanet era apply era.yaml --yes
lfsplanet era open 2007-12-21
lfsplanet era close 2007-12-21
lfsplanet era delete 2007-12-21 --yes
```

## Completion badges

A ranking can award a completion badge in addition to its usual ranking badge:

```yaml
badge:
  label: NUTTER
  qualification:
    type: ranked
    limit: 3
  completion:
    label: GABOR
```

The top three keep their Nutter badges. Every player with a published result on
all charts selected by that era's Nutter ranking also receives Gabor, regardless
of position or benchmark handicap. Players can hold both badges. The frontend
uses the Gabor artwork for Nutter completion awards.

Apply the updated era definitions to activate this configuration; applying an
updated definition also rebuilds its stored player badges.
