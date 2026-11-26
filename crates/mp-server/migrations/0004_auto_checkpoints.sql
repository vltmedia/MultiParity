-- Automatic checkpoints (taken every few minutes while someone works) are off unless a project
-- turns them on: most art tools already autosave, and copying big files that often costs disk,
-- battery and upload.
ALTER TABLE projects ADD COLUMN auto_checkpoints BOOLEAN NOT NULL DEFAULT FALSE;
