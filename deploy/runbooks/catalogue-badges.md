# Catalogue badge rollout

Run this release in the normal maintenance window with the web application and
record processors stopped. Use the newly built binary and the production config
path for all commands below. Keep the usual database backup for the release.

1. Apply migrations with `lfsplanet migrate`. The first migration adds `badge`,
   `badge_variant`, and `badge_award`, converts stored ranking badge labels into
   slug references, and queues all eras for refresh. The separate
   `20261019160000_remove_legacy_badges.sql` migration drops the old projection.
   A host that already applied `20261019150000_catalogue_badges.sql` applies only
   the cleanup and compatibility backfill on its next migration run; the first file's checksum is unchanged.
2. Copy the badge catalogue definitions and artwork, then run:

   ```sh
   lfsplanet seed assets/badges/*.yaml --dry-run
   lfsplanet seed assets/badges/*.yaml
   ```

   Adjust paths to the deployed catalogue directory. Catalogue deployment now
   synchronizes and seeds badges before applying eras. Built-in identities exist
   after migration, but need seeding for their artwork. Operator-created ranking
   badges need standard/gold/silver/bronze variants.
   The compatibility migration creates text-only presentation for existing custom
   rankings, so they do not block seeding. Add catalogue artwork as desired;
   migrated completion references use `gabor` for Nutter, otherwise
   `<ranking-slug>-completion`.

3. Run `lfsplanet hotlap rebadge --all`. There is no need to reclassify hotlaps for
   this change. If a separate era repair is needed, run
   `lfsplanet hotlap fix-eras` first and rebadge afterwards.
4. Confirm the refresh queue is empty and inspect award totals:

   ```sql
   SELECT count(*) FROM era_badge_refresh;
   SELECT era.slug, award.badge_id, award.variant, count(*)
   FROM badge_award award JOIN era ON era.id = award.era_id
   GROUP BY era.slug, award.badge_id, award.variant
   ORDER BY era.slug, award.badge_id, award.variant;
   ```

5. Deploy the matching frontend and restart the application/processors. Check a
   driver profile and chart/ranking result with podium badges, including artwork
   and tooltip descriptions, before ending maintenance.

A failed rebuild rolls back its automatic award replacement and leaves its
refresh request pending. Correct missing definitions or other reported errors,
then rerun rebadge. Manual grants are preserved by both rebadge and fix-eras.

The old JSONB awards are derived data and are not copied. Player accounts,
hotlaps, personal bests and chart identities are retained. The old application
cannot read the new ranking configuration or awards; rolling back the binary
alone is not supported after this schema change. Restore the pre-release database
backup if a full rollback is required.
