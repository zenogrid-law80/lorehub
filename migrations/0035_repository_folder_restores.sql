-- Folder restores keep the original Lore identity and are not new server repositories.
ALTER TABLE repository_restores ADD COLUMN target_path TEXT;
