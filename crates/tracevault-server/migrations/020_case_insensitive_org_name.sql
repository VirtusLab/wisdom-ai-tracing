-- Make org slug uniqueness case-insensitive so that lookups can match
-- regardless of how the GitHub org name was capitalized in the URL
-- (e.g. github.com/VirtusLab vs the stored slug "virtuslab").
ALTER TABLE orgs DROP CONSTRAINT IF EXISTS orgs_name_key;
CREATE UNIQUE INDEX IF NOT EXISTS orgs_name_lower_key ON orgs (LOWER(name));
