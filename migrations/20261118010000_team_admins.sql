-- Keep existing owners' access, including teams without an owner membership.
INSERT INTO team_member (team_id, player_id, role)
SELECT id, owner_id, 'admin' FROM team
ON CONFLICT (team_id, player_id) DO UPDATE SET role = 'admin';

ALTER TABLE team DROP COLUMN owner_id;
