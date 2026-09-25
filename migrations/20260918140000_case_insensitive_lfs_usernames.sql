CREATE UNIQUE INDEX player_lfs_username_lower_uidx
    ON player (LOWER(lfs_username));
