INSERT INTO player (lfs_username, display_name) VALUES ('worker-test', 'Worker Test');
INSERT INTO era (slug, title, version_requirement, installation_id)
VALUES ('2026-09-23', 'Worker test', '>=0.7A', '0.7');
INSERT INTO vehicle (id, kind, name, normalized_name, license)
VALUES ('XFG', 'standard', 'XF GTI', 'xf gti', 'demo') ON CONFLICT DO NOTHING;
INSERT INTO track (id, name, license, reverse, open_configuration, location)
VALUES ('BL1', 'Blackwood GP', 'demo', false, false, 'BL') ON CONFLICT DO NOTHING;
INSERT INTO hotlap (
    player_id, era_id, track, vehicle, raw_vehicle_name, lap_time_ms,
    split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    original_filename, spr_object_key, source, fingerprint, steering,
    abs_enabled, player_flags, game_version, state
)
SELECT player.id, era.id, 'BL1', 'XFG', 'XF GTI', 60000,
    0, 0, 0, 0, 'test.spr', 'missing.spr', 'upload', repeat('a', 64),
    'wheel', false, 0, '0.7A', 'pending'
FROM player, era WHERE player.lfs_username = 'worker-test' AND era.slug = '2026-09-23';
